//! Fail-closed execution boundary for Sumeragi v2 reducer effects.
//!
//! [`SerializedV2Runtime`] is the only owner of consensus state. This module
//! does not select leaders, count votes, form certificates, change views, or
//! decide blocks. It turns each [`AdapterEffect`] into explicit work at the
//! networking, signing, exact-body, deterministic-validation, application,
//! status, and evidence boundaries. View-specific consumers retain their exact
//! [`EventTag`], while immutable persistence and validation work can be rebound
//! after a certified view transition.
//!
//! The caller must explicitly select the exact-body signature policy: the
//! configured genesis authority at height one or the context's rotating leader
//! thereafter. The executor forwards that policy to the body store and still
//! routes full semantic block validation through the deterministic validator;
//! it never invents a second block-authorization rule.
//!
//! Exact-body fsync executes as a tagged asynchronous task, but its immutable
//! storage operation is separate from the current reducer consumer. Canonical
//! decoding and deterministic validation execute against an immutable durable
//! receipt; only the executor owns the current reducer consumer. Only
//! [`V2BodyStore`] can mint completion receipts, so networking code cannot
//! acknowledge durability or validity.
//!
//! # Worker integration contract
//!
//! 1. Open the adapter/runtime, then call [`V2EffectExecutor::open`]. Move the
//!    returned [`V2BodyStore`] to the storage/validation service thread. If
//!    recovery reported an interrupted canonical Kura tip, call
//!    [`V2EffectExecutor::verify_pending_kura_apply_replay`] before dispatching
//!    startup effects or opening ingress. Drain that local replay only through
//!    [`V2EffectExecutor::step_pending_tip_recovery`] while live clocks remain
//!    unarmed; the finalized runtime is then consumed. For a normal height,
//!    call [`V2EffectExecutor::arm_live_clocks`] exactly once after every
//!    constructor and startup effect and immediately before opening ingress.
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
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use super::v2_core::{
    EquivocationKind, EventTag, ExactBodyOwnerProjection, ExactBodyRetirementAccounting,
    MAX_EFFECTS_PER_STEP, TagProjection, exact_body_stage_is_owned, plan_exact_body_owner_binding,
    plan_exact_body_owner_rebind, plan_exact_body_retirement_accounting,
};
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    block::{BlockHeader, CertifiedMergeLedgerReference, consensus_v2 as wire},
    merge::MergeLedgerEntry,
    peer::PeerId,
};

use super::{
    output_guard::ConsensusOutputGuard,
    v2::{AdapterEffect, AdapterError, SignRequest},
    v2_body_store::{
        BlockSignaturePolicy, BodyStoreCompletion, BodyValidationCompletion, DurableBodyReceipt,
        V2BodyStore, ValidatedBodyReceipt,
    },
    v2_chunks::{V2ChunkError, encode_payload},
    v2_recovery::PendingKuraApply,
    v2_runtime::{
        BodyAvailableReservation, DecisionProposalRetirement, EnqueueError, NetworkIngressError,
        RetiredBodyPipelineCompletions, RuntimeClockError, RuntimeQueueLaneSnapshot,
        RuntimeQueueSnapshot, RuntimeStep, SerializedV2Runtime,
    },
    v2_transport::{
        AuthenticatedCertifiedBodyRequest, AuthenticatedPayloadChunk,
        CertifiedBodyRequestRegistrationPlan, CertifiedBodyRequestRetirementPlan,
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

/// Exact height-one Nexus/AMX projection authenticated by replay of a durable
/// Decision, body frame, and deterministic validation marker.
///
/// The field is private so only [`V2EffectExecutor::verify_pending_kura_apply_replay`]
/// can mint this capability after completing the full pending-tip binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct VerifiedPendingGenesisNexusAmxContext {
    hash: Hash,
}

impl VerifiedPendingGenesisNexusAmxContext {
    /// Return the exact projection bound into the replayed height-context id.
    pub(crate) const fn hash(self) -> Hash {
        self.hash
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
    /// Construct ordinary chunk-reconstruction work for service-boundary unit tests.
    #[cfg(test)]
    pub(crate) fn ordinary_for_test(
        id: u64,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Self {
        Self {
            id: EffectWorkId(id),
            tag,
            round: manifest.round,
            subject: manifest.subject,
            manifest: Some(manifest),
            sources: Vec::new(),
            certified_request: None,
        }
    }

    /// Construct certified or hybrid work for service-boundary unit tests.
    #[cfg(test)]
    pub(crate) fn certified_for_test(
        id: u64,
        tag: EventTag,
        manifest: Option<wire::PayloadManifest>,
        sources: Vec<PeerId>,
        certified_request: wire::CertifiedBodyRequest,
    ) -> Self {
        Self {
            id: EffectWorkId(id),
            tag,
            round: certified_request.round,
            subject: certified_request.subject,
            manifest,
            sources,
            certified_request: Some(certified_request),
        }
    }

    /// Work identifier used by chunk and reconstruction callbacks.
    pub(crate) const fn id(&self) -> EffectWorkId {
        self.id
    }

    /// Manifest known before reconstruction, if any.
    pub(crate) const fn manifest(&self) -> Option<&wire::PayloadManifest> {
        self.manifest.as_ref()
    }

    /// Whether a reconstructed manifest is the exact result requested by this work.
    pub(crate) fn matches_reconstructed_manifest(&self, manifest: &wire::PayloadManifest) -> bool {
        manifest.round == self.round
            && manifest.subject == self.subject
            && self
                .manifest
                .as_ref()
                .is_none_or(|expected| expected == manifest)
    }

    /// Whether two tasks name the same immutable executor-owned operation.
    pub(crate) fn has_same_identity(&self, other: &Self) -> bool {
        self.id == other.id
            && self.tag == other.tag
            && self.round == other.round
            && self.subject == other.subject
    }

    /// Whether this task is an exact monotonic acquisition upgrade of `previous`.
    ///
    /// A proposal manifest and the first authenticated certificate request may
    /// each arrive after acquisition starts. Neither authority may subsequently
    /// be removed or replaced.
    pub(crate) fn monotonically_extends(&self, previous: &Self) -> bool {
        if !self.has_same_identity(previous)
            || previous
                .manifest
                .as_ref()
                .is_some_and(|manifest| self.manifest.as_ref() != Some(manifest))
            || previous
                .certified_request
                .as_ref()
                .is_some_and(|request| self.certified_request.as_ref() != Some(request))
        {
            return false;
        }

        match (&previous.certified_request, &self.certified_request) {
            (Some(_), Some(_)) => self.sources == previous.sources,
            (None, Some(_)) => previous.sources.is_empty(),
            (None, None) => self.sources == previous.sources,
            (Some(_), None) => false,
        }
    }

    /// Rebind the consumer of unchanged acquisition work to a later reducer incarnation.
    ///
    /// A timeout certificate may advance the reducer while reconstruction of the exact
    /// post-install durable-lock body is live or already queued for completion. The immutable
    /// acquisition authority and work identifier stay fixed; only the completion tag may advance.
    pub(crate) fn rebind_consumer(&self, tag: EventTag) -> Option<Self> {
        if tag.height() != self.tag.height()
            || tag.view() <= self.tag.view()
            || tag.generation() <= self.tag.generation()
        {
            return None;
        }
        let mut rebound = self.clone();
        rebound.tag = tag;
        Some(rebound)
    }

    /// Whether `self` is the exact later-incarnation consumer binding of `previous`.
    pub(crate) fn rebinds_consumer_of(&self, previous: &Self) -> bool {
        self.tag.height() == previous.tag.height()
            && self.tag.view() > previous.tag.view()
            && self.tag.generation() > previous.tag.generation()
            && self.id == previous.id
            && self.round == previous.round
            && self.subject == previous.subject
            && self.manifest == previous.manifest
            && self.sources == previous.sources
            && self.certified_request == previous.certified_request
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
    /// Construct immutable persistence work for cross-module queue tests.
    #[cfg(test)]
    pub(crate) fn for_test(
        id: u64,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        canonical_wire: Vec<u8>,
    ) -> Self {
        Self {
            id: EffectWorkId::for_test(id),
            tag,
            manifest,
            canonical_wire: Arc::from(canonical_wire),
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

    /// Exact canonical manifest stored beside the body.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }

    /// Exact canonical `SignedBlockWire` bytes.
    pub(crate) fn canonical_wire(&self) -> &[u8] {
        &self.canonical_wire
    }
}

/// Immutable deterministic-validation work for one exact durable body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BodyValidationTask {
    id: EffectWorkId,
    durable_receipt: DurableBodyReceipt,
}

impl BodyValidationTask {
    /// Construct exact deterministic-validation work for body-store boundary tests.
    #[cfg(test)]
    pub(crate) const fn for_test(id: u64, durable_receipt: DurableBodyReceipt) -> Self {
        Self {
            id: EffectWorkId(id),
            durable_receipt,
        }
    }

    /// Stable work identifier reused by every retry.
    pub(crate) const fn id(&self) -> EffectWorkId {
        self.id
    }

    /// Exact proposal round.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        self.durable_receipt.round()
    }

    /// Exact proposal subject.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.durable_receipt.subject()
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

    /// Work identifier whose queue ownership ends when this completion is consumed.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        self.work_id
    }
}

/// Operational status of the effect boundary, excluding consensus state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectExecutorStatus {
    /// Exact height-context incarnation which owns every counter and queue.
    pub height_context_id: wire::HeightContextId,
    /// Height which owns every counter and queue.
    pub height: u64,
    /// Monotonic instant at which all queue ages in this snapshot were measured.
    pub captured_at: Instant,
    /// Whether an internal boundary failure permanently stopped execution.
    pub fail_closed: bool,
    /// First fatal diagnostic, retained until process restart.
    pub fatal_reason: Option<String>,
    /// Outstanding signing operations.
    pub pending_signatures: usize,
    /// Height-local locked-candidate acquisitions awaiting their current consumer.
    pub pending_candidate_loads: usize,
    /// Outstanding body reconstruction/fetch operations.
    pub pending_fetches: usize,
    /// Outstanding exact-body persistence operations.
    pub pending_stores: usize,
    /// Outstanding deterministic-validation operations.
    pub pending_validations: usize,
    /// Validation operations waiting for an exact merge sidecar.
    pub deferred_validation_merge_work: usize,
    /// Durable Apply operations waiting for an exact merge sidecar.
    pub deferred_application_merge_work: usize,
    /// All validation or application operations waiting for an exact merge sidecar.
    ///
    /// This aggregate is retained for bounded-ownership diagnostics; callers
    /// classifying the active stage must use the split counters above.
    pub deferred_merge_work: usize,
    /// Outstanding durable application operations.
    pub pending_applications: usize,
    /// Reconstructed bodies waiting for the reducer's StoreBody effect.
    pub ready_bodies: usize,
    /// Logical bytes owned by reconstructed and locked-body stages.
    ///
    /// A byte-identical immutable subject may also appear in the pending-store
    /// counter. Admission applies `max_ready_body_bytes` to the deterministic
    /// subject-keyed union, so these diagnostic stage counters may overlap.
    pub ready_body_bytes: u64,
    /// Logical bytes owned by pending store tasks; aliases can overlap with
    /// `ready_body_bytes` and are deduplicated for bounded admission.
    pub pending_store_bytes: u64,
    /// Runtime completions queued for serialized reducer delivery.
    pub queued_runtime_completions: usize,
    /// Completed I/O work retained by the bounded effect-worker handoff.
    pub effect_completion_queue: RuntimeQueueLaneSnapshot,
    /// Reducer effects retained until one bounded pending-work slot becomes available.
    ///
    /// This strict FIFO is attempted before runtime advancement and therefore
    /// never reports eligible scheduler-skip debt.
    pub effect_dispatch_queue: RuntimeQueueLaneSnapshot,
    /// Per-class serialized runtime ownership and fairness state.
    pub runtime_queues: RuntimeQueueSnapshot,
    /// View-aware no-progress threshold derived from the configured pacemaker.
    pub watchdog_threshold: Duration,
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
    /// Retire retained local proposal payloads for one superseded subject.
    ///
    /// Lock or Decision installation is the ownership boundary: a late
    /// completion may not restore chunks for a proposal the reducer can no
    /// longer broadcast.
    fn retire_outbound_payload_for_subject(
        &mut self,
        subject: wire::BlockSubject,
    ) -> Result<(), Self::Error>;
    /// Retire every height-local proposal payload after a durable Decision.
    ///
    /// Once one subject is decided, no proposal at the active height can be
    /// broadcast again. Keeping any chunk owner alive would let a late local
    /// completion resurrect terminal proposal work.
    fn retire_all_outbound_payloads(&mut self) -> Result<(), Self::Error>;
    /// Retire height-local candidate acquisition and prepared proposal work
    /// after a durable Decision.
    fn retire_candidate_work_after_decision(
        &mut self,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
    ) -> Result<(), Self::Error>;
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
    /// Move the completion consumer for one unchanged protected fetch to a later view.
    ///
    /// Implementations must preserve live acquisition state and any already queued terminal
    /// completion. This is an ownership transfer, not cancellation followed by new work.
    fn rebind_body_fetch(
        &mut self,
        previous: &BodyFetchTask,
        rebound: BodyFetchTask,
    ) -> Result<(), Self::Error>;
    /// Cancel exact reconstruction work, whether still live or already held by
    /// the bounded queued-reconstruction completion handoff.
    fn cancel_body_fetch(&mut self, task: &BodyFetchTask) -> Result<(), Self::Error>;
    /// Transfer one exact ordinary or hybrid reconstruction owner to the executor.
    ///
    /// Implementations must validate the complete task before mutation and
    /// then remove exactly one live or reconstructed service owner. Returning
    /// an error guarantees that the service owner is unchanged.
    fn complete_body_reconstruction_fetch(
        &mut self,
        task: &BodyFetchTask,
    ) -> Result<(), Self::Error>;
    /// Retire the exact service owner after a certified response wins acquisition.
    fn complete_certified_body_fetch(&mut self, task: &BodyFetchTask) -> Result<(), Self::Error>;
    /// Hand one structurally, cryptographically, and outer-peer authenticated
    /// chunk to the persistent chunk/reconstruction adapter.
    fn accept_authenticated_chunk(
        &mut self,
        task: &BodyFetchTask,
        chunk: AuthenticatedPayloadChunk,
    ) -> Result<AuthenticatedChunkDisposition, Self::Error>;
    /// Queue or retransmit exact-body persistence. Repeated task identifiers
    /// refer to the same immutable bytes.
    fn enqueue_body_store(&mut self, task: BodyStoreTask) -> Result<(), Self::Error>;
    /// Cancel body persistence made stale before a certified view transition.
    ///
    /// Returns `true` only when queued work was removed before execution. A
    /// `false` result means the immutable operation is already active or its
    /// completion is pending. The service keeps authenticating that physical
    /// owner until delivery, while the executor may logically retire its stale
    /// reducer consumer and account the eventual completion as stale durable
    /// catalogue input.
    fn cancel_body_store(&mut self, work_id: EffectWorkId) -> Result<bool, Self::Error>;
    /// Queue or retransmit deterministic validation of one exact durable body.
    fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error>;
    /// Cancel deterministic-validation work made stale by a certified view transition.
    fn cancel_body_validation(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error>;
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
        offender: PeerId,
        round: wire::ConsensusRound,
        kind: EquivocationKind,
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
    /// The exact completion was accepted, whether routed immediately or cached
    /// until a current reducer consumer attaches.
    Accepted,
    /// Validation remains pending until its exact certified merge sidecar is
    /// fetched, authenticated, and installed for a deterministic retry.
    Deferred,
    /// Authenticated remote data completed an acquisition but proved
    /// noncanonical, so that acquisition was retired without advancing the
    /// reducer.
    Rejected,
    /// The work identifier was already completed or belongs to an old owner.
    Stale,
}

/// Result of handing an authenticated chunk to the persistent reconstruction
/// service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthenticatedChunkDisposition {
    /// The chunk was retained or completed one canonical reconstruction.
    Accepted,
    /// The committed chunk set reconstructed invalid or noncanonical body data;
    /// the service retired that remote acquisition without a local failure.
    Rejected,
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
    BodyMismatch(&'static str),
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
            Self::BodyMismatch(reason) => {
                write!(
                    f,
                    "Sumeragi v2 reconstructed body does not match its fetch: {reason}"
                )
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingFetch {
    task: BodyFetchTask,
    request_hash: Option<HashOf<wire::CertifiedBodyRequest>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StorePurpose {
    Reducer,
    LocalProposal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StoreConsumer {
    Reducer { tag: EventTag },
    LocalProposal { tag: EventTag },
}

impl StoreConsumer {
    const fn new(tag: EventTag, purpose: StorePurpose) -> Self {
        match purpose {
            StorePurpose::Reducer => Self::Reducer { tag },
            StorePurpose::LocalProposal => Self::LocalProposal { tag },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingStore {
    task: BodyStoreTask,
    consumer: Option<StoreConsumer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValidationConsumer {
    Reducer {
        tag: EventTag,
    },
    LocalProposal {
        tag: EventTag,
        manifest: wire::PayloadManifest,
    },
}

impl ValidationConsumer {
    const fn tag(&self) -> EventTag {
        match self {
            Self::Reducer { tag } | Self::LocalProposal { tag, .. } => *tag,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingValidation {
    task: BodyValidationTask,
    consumer: Option<ValidationConsumer>,
}

#[derive(Clone, Debug)]
struct PendingApply {
    task: ApplyTask,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReadyBody {
    manifest: wire::PayloadManifest,
    bytes: Arc<[u8]>,
}

impl ReadyBody {
    fn derive(
        context: &wire::HeightContext,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Result<Self, V2ChunkError> {
        let bytes = bytes.into();
        let manifest = encode_payload(context, round, subject, bytes.as_ref())?
            .manifest()
            .clone();
        Ok(Self { manifest, bytes })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BodyPipelineOwner {
    tag: EventTag,
    manifest_hash: Option<HashOf<wire::PayloadManifest>>,
}

/// Preflighted exact-body owner update.
///
/// Planning performs every fallible identity check. The service/runtime
/// admission happens next; installing this value afterwards is an infallible
/// map replacement because the executor is the sole owner of these maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BodyPipelineOwnerBindingPlan {
    key: (wire::ConsensusRound, wire::BlockSubject),
    owner: BodyPipelineOwner,
    already_owned: bool,
}

#[derive(Clone, Copy, Debug)]
struct WorkIdPlan {
    id: EffectWorkId,
    next: u64,
}

#[derive(Clone, Debug)]
struct ReadyBodyReleasePlan {
    key: (wire::ConsensusRound, wire::BlockSubject),
    body: ReadyBody,
    remaining_ready_bytes: u64,
}

#[derive(Clone, Debug)]
struct ReadyBodyInstallPlan {
    key: (wire::ConsensusRound, wire::BlockSubject),
    body: ReadyBody,
    ready_body_bytes: u64,
    release: Option<ReadyBodyReleasePlan>,
}

#[derive(Clone, Debug)]
struct RetainedLockedBodyPlan {
    subject: wire::BlockSubject,
    bytes: Arc<[u8]>,
    install: bool,
    ready_body_bytes: u64,
}

#[derive(Clone, Debug)]
struct RetainedBodyUnionEntry {
    bytes: Arc<[u8]>,
    owners: usize,
    manifests: BTreeMap<wire::ConsensusRound, (wire::PayloadManifest, usize)>,
}

#[derive(Clone, Debug, Default)]
struct RetainedBodyUnion {
    entries: BTreeMap<wire::BlockSubject, RetainedBodyUnionEntry>,
}

impl RetainedBodyUnion {
    fn insert(
        &mut self,
        subject: wire::BlockSubject,
        bytes: Arc<[u8]>,
    ) -> Result<(), EffectExecutorError> {
        if Hash::new(bytes.as_ref()) != subject.payload_hash {
            return Err(EffectExecutorError::Contract(
                "retained canonical bytes differ from their subject payload hash".to_owned(),
            ));
        }
        if let Some(existing) = self.entries.get_mut(&subject) {
            if existing.bytes.as_ref() != bytes.as_ref() {
                return Err(EffectExecutorError::Contract(
                    "one canonical subject has conflicting retained bytes".to_owned(),
                ));
            }
            existing.owners = existing.owners.checked_add(1).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "retained canonical-body owner count overflowed".to_owned(),
                )
            })?;
            return Ok(());
        }
        self.entries.insert(
            subject,
            RetainedBodyUnionEntry {
                bytes,
                owners: 1,
                manifests: BTreeMap::new(),
            },
        );
        Ok(())
    }

    fn insert_manifest(
        &mut self,
        manifest: wire::PayloadManifest,
        bytes: Arc<[u8]>,
    ) -> Result<(), EffectExecutorError> {
        if let Some(entry) = self.entries.get(&manifest.subject)
            && let Some((existing, _)) = entry.manifests.get(&manifest.round)
            && existing != &manifest
        {
            return Err(EffectExecutorError::Contract(
                "one exact body round has conflicting retained manifests".to_owned(),
            ));
        }
        let subject = manifest.subject;
        let round = manifest.round;
        self.insert(subject, bytes)?;
        let entry = self.entries.get_mut(&subject).ok_or_else(|| {
            EffectExecutorError::Contract(
                "retained union insertion lost its exact subject".to_owned(),
            )
        })?;
        match entry.manifests.get_mut(&round) {
            Some((_, owners)) => {
                *owners = owners.checked_add(1).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "retained manifest owner count overflowed".to_owned(),
                    )
                })?;
            }
            None => {
                entry.manifests.insert(round, (manifest, 1));
            }
        }
        Ok(())
    }

    fn remove(
        &mut self,
        subject: wire::BlockSubject,
        bytes: &[u8],
    ) -> Result<(), EffectExecutorError> {
        let Some(existing) = self.entries.get_mut(&subject) else {
            return Err(EffectExecutorError::Contract(
                "planned canonical-body release has no deterministic union owner".to_owned(),
            ));
        };
        if existing.bytes.as_ref() != bytes {
            return Err(EffectExecutorError::Contract(
                "planned canonical-body release differs from deterministic union bytes".to_owned(),
            ));
        }
        if existing.owners > 1 {
            existing.owners -= 1;
        } else {
            self.entries.remove(&subject);
        }
        Ok(())
    }

    fn remove_manifest(
        &mut self,
        manifest: &wire::PayloadManifest,
        bytes: &[u8],
    ) -> Result<(), EffectExecutorError> {
        let Some(entry) = self.entries.get_mut(&manifest.subject) else {
            return Err(EffectExecutorError::Contract(
                "planned manifest release has no deterministic union owner".to_owned(),
            ));
        };
        if entry.bytes.as_ref() != bytes {
            return Err(EffectExecutorError::Contract(
                "planned manifest release differs from deterministic union bytes".to_owned(),
            ));
        }
        let Some((existing, manifest_owners)) = entry.manifests.get_mut(&manifest.round) else {
            return Err(EffectExecutorError::Contract(
                "planned manifest release has no exact round owner".to_owned(),
            ));
        };
        if existing != manifest {
            return Err(EffectExecutorError::Contract(
                "planned manifest release differs from exact retained evidence".to_owned(),
            ));
        }
        if *manifest_owners > 1 {
            *manifest_owners -= 1;
        } else {
            entry.manifests.remove(&manifest.round);
        }
        self.remove(manifest.subject, bytes)
    }

    fn total_bytes(&self) -> Result<u64, EffectExecutorError> {
        self.entries.values().try_fold(0u64, |total, entry| {
            let bytes = u64::try_from(entry.bytes.len()).map_err(|_| {
                EffectExecutorError::Contract(
                    "retained canonical-body byte count is not representable".to_owned(),
                )
            })?;
            total.checked_add(bytes).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "retained canonical-body union byte count overflowed".to_owned(),
                )
            })
        })
    }
}

#[derive(Clone, Debug)]
struct CertifiedFetchRequestPlan {
    work_id: EffectWorkId,
    request: wire::CertifiedBodyRequest,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    registration: CertifiedBodyRequestRegistrationPlan,
}

#[derive(Clone, Debug)]
struct CertifiedFetchRetirementPlan {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    retirement: CertifiedBodyRequestRetirementPlan,
}

#[derive(Clone, Debug)]
struct PendingFetchRetirementPlan {
    pending: PendingFetch,
    certified: Option<CertifiedFetchRetirementPlan>,
}

#[derive(Clone, Debug)]
enum StaleFetchTransitionPlan {
    Rebind {
        pending: PendingFetch,
        rebound: BodyFetchTask,
        owner: BodyPipelineOwner,
    },
    Retire(PendingFetchRetirementPlan),
}

/// Preflighted byte ownership retired by one certified-view cleanup.
///
/// The exact residual is computed before any cancellation or runtime queue
/// mutation. The executor installs it only after every fallible callback has
/// acknowledged the planned cleanup.
#[derive(Clone, Debug)]
struct CertifiedViewBodyCleanupPlan {
    stale_stores: Vec<EffectWorkId>,
    stale_ready: Vec<(wire::ConsensusRound, wire::BlockSubject)>,
    protected_ready_rebinds: Vec<CertifiedViewReadyRebindPlan>,
    accounting: ExactBodyRetirementAccounting,
}

#[derive(Clone, Debug)]
struct CertifiedViewReadyRebindPlan {
    key: (wire::ConsensusRound, wire::BlockSubject),
    previous_tag: EventTag,
    manifest: wire::PayloadManifest,
    owner: BodyPipelineOwner,
}

#[derive(Clone, Debug)]
enum FetchReadyCommitPlan {
    Reuse {
        release: Option<ReadyBodyReleasePlan>,
    },
    Install(ReadyBodyInstallPlan),
}

#[derive(Clone, Debug)]
struct FetchCompletionPlan {
    work_id: EffectWorkId,
    owner: BodyPipelineOwnerBindingPlan,
    ready: FetchReadyCommitPlan,
    certified_retirement: Option<CertifiedFetchRetirementPlan>,
    runtime_reservation: BodyAvailableReservation,
}

#[derive(Clone, Debug)]
enum ValidationAdmissionPlan {
    None,
    Service(BodyValidationTask),
    RuntimeSucceeded {
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    },
    RuntimeFailed {
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    },
    RuntimeLocalProposal {
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    },
}

#[derive(Clone, Debug)]
enum ValidationCommitPlan {
    None,
    Attach {
        id: EffectWorkId,
        consumer: ValidationConsumer,
    },
    Insert {
        work: WorkIdPlan,
        pending: PendingValidation,
    },
}

#[derive(Clone, Debug)]
struct ValidationStartPlan {
    admission: ValidationAdmissionPlan,
    commit: ValidationCommitPlan,
}

#[derive(Debug)]
struct FinalityCompletion {
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
}

/// One reducer-emitted causal suffix waiting for pending-work capacity.
///
/// The source reducer bounds every transition by [`MAX_EFFECTS_PER_STEP`], so
/// retaining the unconsumed suffix preserves exact FIFO order without creating
/// an independently growing adapter queue. This queue is intentionally
/// volatile: after process restart each progress item is reconstructed from
/// the source classified by [`RestartEffectSource`] rather than from this
/// adapter memory.
#[derive(Debug)]
struct RetainedEffectBatch {
    effects: VecDeque<AdapterEffect>,
    oldest_at: Instant,
}

/// Exhaustive source inventory of effects which may create pending work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingWorkProducer {
    Sign,
    Fetch,
    Store,
    Validate,
    Apply,
}

/// Restart authority for one reducer-emitted adapter effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestartEffectSource {
    /// Safety-WAL outbound intent, signed message, QC, or TC.
    DurableConsensusEvidence,
    /// Reducer Missing state plus proposal/QC/lock/Decision retransmission.
    BodyReconstruction,
    /// Checksummed exact-body store catalog.
    DurableBody,
    /// Durable Decision plus the exact fsynced validation marker.
    DurableDecision,
    /// Recovered view owns fresh process-local cleanup; old services no longer exist.
    RecoveredView,
    /// Non-progress diagnostic; losing it in a process crash cannot orphan work.
    DiagnosticOnly,
}

pub(crate) trait EffectRuntime {
    fn step_effects(&mut self, now: Instant) -> Result<RuntimeStep<AdapterEffect>, String>;
    fn step_recovery_effects(&mut self, now: Instant)
    -> Result<RuntimeStep<AdapterEffect>, String>;
    /// Return the exact durable Decision currently owned by the reducer.
    fn decided_body(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        String,
    >;
    fn enqueue_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError>;
    /// Reserve an exact body completion without exposing it to the reducer.
    fn reserve_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError>;
    /// Publish one previously reserved body completion infallibly.
    fn commit_body_available(&mut self, reservation: BodyAvailableReservation);
    /// Release one unpublished body-completion reservation infallibly.
    fn abort_body_available(&mut self, reservation: BodyAvailableReservation);
    /// Rebind one already queued exact-body completion to a later reducer incarnation.
    ///
    /// Returns `true` only when the runtime still owned that exact completion.
    fn rebind_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String>;
    /// Retire one queued exact-body completion whose reducer pipeline was superseded.
    ///
    /// Returns `true` only when the runtime removed that exact completion owner.
    fn retire_body_available(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String>;
    /// Retire every serialized completion stage for one exact superseded pipeline.
    fn retire_body_pipeline_completions(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<RetiredBodyPipelineCompletions, String>;
    /// Retire queued proposals which an installed lock makes definitively unsafe.
    fn retire_unsafe_proposals_for_lock(
        &mut self,
        locked_round: wire::ConsensusRound,
        locked_subject: wire::BlockSubject,
    ) -> Result<usize, String>;
    /// Retire queued authenticated and local proposal work after Decision.
    ///
    /// A unique current-tag completion is retained only when its full trusted
    /// evidence matches the durable Decision; stale exact work is retired for
    /// reconstruction and conflicts fail closed before mutation.
    fn retire_proposal_work_after_decision(
        &mut self,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> Result<DecisionProposalRetirement, String>;
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
    /// Atomically admit every deterministic validation rejection in one set.
    fn enqueue_validation_failures_atomically(
        &mut self,
        failures: &[(EventTag, wire::ConsensusRound, wire::BlockSubject)],
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
    fn remaining_completion_capacity(&self) -> usize;
    fn queue_snapshot(&self, now: Instant) -> RuntimeQueueSnapshot;
    fn watchdog_threshold(&self) -> Duration;
}

impl EffectRuntime for SerializedV2Runtime {
    fn step_effects(&mut self, now: Instant) -> Result<RuntimeStep<AdapterEffect>, String> {
        self.step(now).map_err(|error| error.to_string())
    }

    fn step_recovery_effects(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<AdapterEffect>, String> {
        self.step_recovery(now).map_err(|error| error.to_string())
    }

    fn decided_body(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        String,
    > {
        self.replayed_decision_key()
            .map_err(|error| error.to_string())
    }

    fn enqueue_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_body_available(self, tag, manifest)
    }

    fn reserve_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        SerializedV2Runtime::reserve_body_available(self, tag, manifest)
    }

    fn commit_body_available(&mut self, reservation: BodyAvailableReservation) {
        SerializedV2Runtime::commit_body_available(self, reservation);
    }

    fn abort_body_available(&mut self, reservation: BodyAvailableReservation) {
        SerializedV2Runtime::abort_body_available(self, reservation);
    }

    fn rebind_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        SerializedV2Runtime::rebind_body_available(self, previous, rebound, manifest)
            .map_err(|error| error.to_string())
    }

    fn retire_body_available(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        SerializedV2Runtime::retire_body_available(self, tag, manifest)
            .map_err(|error| error.to_string())
    }

    fn retire_body_pipeline_completions(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<RetiredBodyPipelineCompletions, String> {
        SerializedV2Runtime::retire_body_pipeline_completions(self, tag, round, subject)
    }

    fn retire_unsafe_proposals_for_lock(
        &mut self,
        locked_round: wire::ConsensusRound,
        locked_subject: wire::BlockSubject,
    ) -> Result<usize, String> {
        SerializedV2Runtime::retire_unsafe_proposals_for_lock(self, locked_round, locked_subject)
    }

    fn retire_proposal_work_after_decision(
        &mut self,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> Result<DecisionProposalRetirement, String> {
        SerializedV2Runtime::retire_proposal_work_after_decision(
            self,
            decision_round,
            decision_subject,
            decision_commitment,
        )
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

    fn enqueue_validation_failures_atomically(
        &mut self,
        failures: &[(EventTag, wire::ConsensusRound, wire::BlockSubject)],
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_validation_failures_atomically(self, failures)
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

    fn remaining_completion_capacity(&self) -> usize {
        SerializedV2Runtime::remaining_completion_capacity(self)
    }

    fn queue_snapshot(&self, now: Instant) -> RuntimeQueueSnapshot {
        SerializedV2Runtime::queue_snapshot(self, now)
    }

    fn watchdog_threshold(&self) -> Duration {
        SerializedV2Runtime::watchdog_threshold(self)
    }
}

/// One-owner executor which binds runtime effects to production adapters.
pub(crate) struct V2EffectExecutor<R = SerializedV2Runtime> {
    runtime: R,
    output_guard: Arc<ConsensusOutputGuard>,
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
    body_pipeline_owners: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), BodyPipelineOwner>,
    certified_work: BTreeMap<HashOf<wire::CertifiedBodyRequest>, EffectWorkId>,
    outstanding_requests: OutstandingCertifiedBodyRequests,
    ready_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ReadyBody>,
    protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    protected_decision: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    decision_body_drained: bool,
    retained_locked_body: Option<(wire::BlockSubject, Arc<[u8]>)>,
    ready_body_bytes: u64,
    pending_store_bytes: u64,
    durable_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    validated_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    rejected_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    finality_completion: Option<FinalityCompletion>,
    retained_effect_batch: Option<RetainedEffectBatch>,
    fatal_reason: Option<String>,
}

impl V2EffectExecutor<SerializedV2Runtime> {
    /// Open the exact-body store under an explicit signature-authority policy
    /// and take ownership of the serialized runtime.
    pub(crate) fn open(
        mut runtime: SerializedV2Runtime,
        body_store_root: impl AsRef<Path>,
        context: wire::HeightContext,
        requester: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        signature_policy: BlockSignaturePolicy,
        output_guard: Arc<ConsensusOutputGuard>,
        config: EffectQueueConfig,
    ) -> Result<(Self, V2BodyStore), EffectExecutorError> {
        let executor_output_guard = Arc::clone(&output_guard);
        let construction = output_guard.begin_fail_stop_operation().ok_or_else(|| {
            EffectExecutorError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            )
        })?;
        let body_store =
            V2BodyStore::open_with_policy(body_store_root, context.clone(), signature_policy)
                .map_err(|error| EffectExecutorError::BodyStore(error.to_string()))?;
        let recovered_bodies = body_store
            .recovery_catalog()
            .map_err(|error| EffectExecutorError::BodyStore(error.to_string()))?;
        let recovered_validations = body_store.validated_recovery_catalog();
        for (key, validated_receipt) in &recovered_validations {
            let Some((manifest, durable_receipt)) = recovered_bodies.get(key) else {
                return Err(EffectExecutorError::BodyStore(
                    "validated recovery marker has no exact durable body".to_owned(),
                ));
            };
            if validated_receipt.durable() != durable_receipt {
                return Err(EffectExecutorError::BodyStore(
                    "validated recovery marker differs from its durable body".to_owned(),
                ));
            }
            runtime
                .recover_validated_body(manifest, validated_receipt)
                .map_err(|error| EffectExecutorError::Runtime(error.to_string()))?;
        }
        let mut executor = Self::with_runtime_and_guard(
            runtime,
            recovered_bodies,
            context,
            requester,
            local_validator,
            executor_output_guard,
            config,
        )?;
        executor.validated_bodies = recovered_validations;
        construction.complete();
        Ok((executor, body_store))
    }

    /// Arm the runtime pacemaker after all height startup work has completed.
    pub(crate) fn arm_live_clocks(&mut self, now: Instant) -> Result<(), RuntimeClockError> {
        self.runtime.arm_live_clocks(now)
    }

    /// Prepare the reducer status installed only when this height's live
    /// activation boundary succeeds.
    pub(crate) fn successor_activation_status_snapshot(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        self.runtime.successor_activation_status_snapshot()
    }

    /// Bind an interrupted Kura tip to the exact reducer Decision and durable
    /// validation marker reconstructed before network ingress opens.
    ///
    /// This must be called immediately after [`Self::open`] whenever recovery
    /// returns a [`PendingKuraApply`]. A missing Decision, a different block,
    /// or absent exact body/validation durability fails closed before the
    /// startup effects can be dispatched. Exact height-one replay returns a
    /// capability binding the frozen Nexus/AMX projection for pre-apply lane
    /// work; other heights return `None`.
    pub(crate) fn verify_pending_kura_apply_replay(
        &self,
        expected: PendingKuraApply,
    ) -> Result<Option<VerifiedPendingGenesisNexusAmxContext>, EffectExecutorError> {
        self.ensure_open()?;
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
        if self.fatal_reason.is_some() || self.output_guard.restart_required() {
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

    /// Whether the fair-ingress head can enter its exact runtime FIFO prefix
    /// or coalesce with an exact queued authenticated envelope.
    pub(crate) fn can_admit_network_message(&self, message: &wire::ConsensusMessageV2) -> bool {
        self.fatal_reason.is_none()
            && !self.output_guard.restart_required()
            && self.retained_effect_batch.is_none()
            && self.runtime.can_admit_network_message(message)
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
        !self.output_guard.restart_required()
            && self.finality_completion.is_some()
            && self.retained_effect_batch.is_none()
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
    #[cfg(test)]
    pub(crate) fn with_runtime(
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
        Self::with_runtime_and_guard(
            runtime,
            recovered_bodies,
            context,
            requester,
            local_validator,
            ConsensusOutputGuard::isolated(),
            config,
        )
    }

    fn with_runtime_and_guard(
        runtime: R,
        recovered_bodies: BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            (wire::PayloadManifest, DurableBodyReceipt),
        >,
        context: wire::HeightContext,
        requester: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        output_guard: Arc<ConsensusOutputGuard>,
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
            output_guard,
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
            body_pipeline_owners: BTreeMap::new(),
            certified_work: BTreeMap::new(),
            outstanding_requests,
            ready_bodies: BTreeMap::new(),
            protected_lock: None,
            protected_decision: None,
            decision_body_drained: false,
            retained_locked_body: None,
            ready_body_bytes: 0,
            pending_store_bytes: 0,
            durable_bodies: BTreeMap::new(),
            validated_bodies: BTreeMap::new(),
            rejected_bodies: BTreeMap::new(),
            finality_completion: None,
            retained_effect_batch: None,
            fatal_reason: None,
        })
    }

    /// Whether a new local proposal can reserve its first exact-body work owner.
    ///
    /// The runner checks this before consuming a prepared candidate or
    /// registering outbound payload bytes. Reducer retransmission and local
    /// completions continue while admission is deferred. This capacity rule is
    /// runtime-independent so production and deterministic executor tests use
    /// the same admission boundary.
    pub(crate) fn can_admit_local_proposal(&self) -> bool {
        self.fatal_reason.is_none()
            && !self.output_guard.restart_required()
            && self.retained_effect_batch.is_none()
            && self.pending_work() < self.config.max_pending_work
    }

    /// Exact runtime FIFO capacity currently available to trusted completions.
    pub(crate) fn remaining_completion_capacity(&self) -> usize {
        self.runtime.remaining_completion_capacity()
    }

    /// Reconcile executor ownership with the reducer's exact monotonic PrepareQC lock.
    ///
    /// A higher lock can be installed without changing the reducer [`EventTag`]. Before the
    /// runner stages bytes for that lock, every round-bound owner of the superseded lock must be
    /// retired. The view-independent byte cache is retained only when the exact subject is
    /// unchanged. Publishing the replacement rank last also prevents a delayed observation of an
    /// older lock from reclaiming the cache after `A -> B` reconciliation.
    pub(crate) fn reconcile_locked_body_for_reproposal<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        lock: (wire::ConsensusRound, wire::BlockSubject),
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        // Validate the synchronous runner observation before cleanup starts.
        // Exact repetition remains idempotent. A same-round conflict or lower
        // reducer directive is an invariant violation, but rejecting it here
        // keeps the executor projection untouched so the runner can latch its
        // shared restart guard with the original diagnostic.
        self.preflight_observed_protected_lock(tag, lock)?;
        let changed = match self.reconcile_protected_lock(tag, Some(lock), services) {
            Ok(changed) => changed,
            Err(error) => return Err(self.close(error, services)),
        };
        if changed && let Err(error) = self.publish_status(services) {
            return Err(self.close(error, services));
        }
        Ok(())
    }

    fn preflight_observed_protected_lock(
        &self,
        tag: EventTag,
        replacement: (wire::ConsensusRound, wire::BlockSubject),
    ) -> Result<(), EffectExecutorError> {
        let (replacement_round, _) = replacement;
        if tag.height() != self.context.height
            || replacement_round.context_id != self.context.id()
            || replacement_round.height != self.context.height
            || replacement_round.view > tag.view()
        {
            return Err(EffectExecutorError::Contract(
                "protected lock is outside its frozen height or consumer view".to_owned(),
            ));
        }
        if let Some(current) = self.protected_lock
            && current != replacement
            && replacement_round.view <= current.0.view
        {
            return Err(EffectExecutorError::Contract(
                "protected lock replacement did not strictly increase PrepareQC round".to_owned(),
            ));
        }
        Ok(())
    }

    fn reconcile_protected_lock<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        replacement: Option<(wire::ConsensusRound, wire::BlockSubject)>,
        services: &mut S,
    ) -> Result<bool, EffectExecutorError> {
        let Some(replacement) = replacement else {
            if self.protected_lock.is_some() || self.retained_locked_body.is_some() {
                return Err(EffectExecutorError::Contract(
                    "a durable PrepareQC lock cannot be cleared by reconciliation".to_owned(),
                ));
            }
            return Ok(false);
        };
        // This method is also entered directly by locked-body reproposal,
        // outside EnterView's aggregate preflight. Check the complete owner
        // sets before even the exact-repetition fast path can return, so no
        // state-clearing entrypoint can silently preserve corrupt counters.
        self.preflight_exact_body_byte_accounting()?;
        let (replacement_round, replacement_subject) = replacement;
        if tag.height() != self.context.height
            || replacement_round.context_id != self.context.id()
            || replacement_round.height != self.context.height
            || replacement_round.view > tag.view()
        {
            return Err(EffectExecutorError::Contract(
                "protected lock is outside its frozen height or consumer view".to_owned(),
            ));
        }

        let superseded = match self.protected_lock {
            Some(current) if current == replacement => return Ok(false),
            Some(current) if replacement_round.view <= current.0.view => {
                return Err(EffectExecutorError::Contract(
                    "protected lock replacement did not strictly increase PrepareQC round"
                        .to_owned(),
                ));
            }
            current => current,
        };
        if let (Some((_, current_subject)), Some((retained_subject, _))) =
            (superseded, self.retained_locked_body.as_ref())
            && *retained_subject != current_subject
        {
            return Err(EffectExecutorError::Contract(
                "retained locked body differs from the published protected lock".to_owned(),
            ));
        }

        let retire_retained = self
            .retained_locked_body
            .as_ref()
            .is_some_and(|(subject, _)| *subject != replacement_subject);
        let key_is_superseded =
            |round: wire::ConsensusRound, subject: wire::BlockSubject| match superseded {
                Some((old_round, old_subject)) if old_subject == replacement_subject => {
                    subject == old_subject && round == old_round
                }
                Some((_, old_subject)) => {
                    subject == old_subject
                        || (subject != replacement_subject
                            && round.context_id == replacement_round.context_id
                            && round.height == replacement_round.height
                            && round.view <= replacement_round.view)
                }
                None => {
                    round.context_id == replacement_round.context_id
                        && round.height == replacement_round.height
                        && if subject == replacement_subject {
                            round.view < replacement_round.view
                        } else {
                            round.view <= replacement_round.view
                        }
                }
            };
        let mut superseded_keys = BTreeSet::new();
        for key in self
            .body_pipeline_owners
            .keys()
            .chain(self.ready_bodies.keys())
            .copied()
        {
            if key_is_superseded(key.0, key.1) {
                superseded_keys.insert(key);
            }
        }
        for pending in self.pending_fetches.values() {
            let key = (pending.task.round, pending.task.subject);
            if key_is_superseded(key.0, key.1) {
                superseded_keys.insert(key);
            }
        }
        for pending in self.pending_stores.values() {
            let key = (pending.task.manifest.round, pending.task.manifest.subject);
            if key_is_superseded(key.0, key.1) {
                superseded_keys.insert(key);
            }
        }
        for pending in self.pending_validations.values() {
            let key = (pending.task.round(), pending.task.subject());
            if key_is_superseded(key.0, key.1) {
                superseded_keys.insert(key);
            }
        }
        for pending in self.pending_applications.values() {
            let key = (pending.task.certificate.round, pending.task.subject);
            if key_is_superseded(key.0, key.1) {
                superseded_keys.insert(key);
            }
        }
        if self.pending_signatures.values().any(|pending| {
            pending
                .request
                .body_round()
                .zip(pending.request.subject())
                .is_some_and(|key| key_is_superseded(key.0, key.1))
        }) {
            return Err(EffectExecutorError::Contract(
                "lock installation overtook an outstanding durable signature intent".to_owned(),
            ));
        }
        if self.pending_applications.values().any(|pending| {
            superseded_keys.contains(&(pending.task.certificate.round, pending.task.subject))
        }) {
            return Err(EffectExecutorError::Contract(
                "a decided protected body cannot be superseded".to_owned(),
            ));
        }
        let retained_bytes = if retire_retained {
            self.retained_locked_body
                .as_ref()
                .map(|(_, bytes)| {
                    u64::try_from(bytes.len()).map_err(|_| {
                        EffectExecutorError::Contract(
                            "retained locked-body byte count is not representable".to_owned(),
                        )
                    })
                })
                .transpose()?
                .unwrap_or(0)
        } else {
            0
        };

        let ready_bytes =
            self.ready_bodies
                .iter()
                .try_fold(0u64, |total, ((_, subject), ready)| {
                    if !superseded_keys.contains(&(ready.manifest.round, *subject)) {
                        return Ok(total);
                    }
                    let bytes = u64::try_from(ready.bytes.len()).map_err(|_| {
                        EffectExecutorError::Contract(
                            "ready-body byte count is not representable".to_owned(),
                        )
                    })?;
                    total.checked_add(bytes).ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "superseded ready-body byte count overflowed".to_owned(),
                        )
                    })
                })?;
        let fetches = self
            .pending_fetches
            .values()
            .filter(|pending| superseded_keys.contains(&(pending.task.round, pending.task.subject)))
            .map(|pending| self.plan_pending_fetch_retirement(pending))
            .collect::<Result<Vec<_>, _>>()?;

        let stores = self
            .pending_stores
            .iter()
            .filter(|(_, pending)| {
                superseded_keys
                    .contains(&(pending.task.manifest.round, pending.task.manifest.subject))
            })
            .map(|(id, pending)| (*id, pending.task.canonical_wire.len()))
            .collect::<Vec<_>>();
        let retired_store_bytes = stores.iter().try_fold(0u64, |total, (_, bytes)| {
            let bytes = u64::try_from(*bytes).map_err(|_| {
                EffectExecutorError::Contract(
                    "pending-store byte count is not representable".to_owned(),
                )
            })?;
            total.checked_add(bytes).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "superseded pending-store byte count overflowed".to_owned(),
                )
            })
        })?;
        let accounting = plan_exact_body_retirement_accounting(
            self.ready_body_bytes,
            retained_bytes,
            ready_bytes,
            self.pending_store_bytes,
            retired_store_bytes,
        )
        .ok_or_else(|| {
            EffectExecutorError::Contract(
                "superseded body byte accounting underflow or leakage".to_owned(),
            )
        })?;

        let validations = self
            .pending_validations
            .iter()
            .filter(|(_, pending)| {
                superseded_keys.contains(&(pending.task.round(), pending.task.subject()))
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        let mut pipeline_owners = Vec::new();
        for key in &superseded_keys {
            if let Some(owner) = self.body_pipeline_owners.get(key).copied() {
                let ready = self.ready_bodies.get(key);
                if let Some(ready) = ready
                    && owner
                        .manifest_hash
                        .is_some_and(|hash| hash != HashOf::new(&ready.manifest))
                {
                    return Err(EffectExecutorError::Contract(
                        "superseded ready body differs from its pipeline ownership".to_owned(),
                    ));
                }
                pipeline_owners.push((*key, owner, ready.is_some()));
            }
        }
        for ((round, subject), owner, ready) in &pipeline_owners {
            let retired = self
                .runtime
                .retire_body_pipeline_completions(owner.tag, *round, *subject)
                .map_err(EffectExecutorError::Runtime)?;
            if *ready && !retired.body_available() {
                return Err(EffectExecutorError::Contract(
                    "superseded ready body has no queued reducer completion to retire".to_owned(),
                ));
            }
        }
        self.runtime
            .retire_unsafe_proposals_for_lock(replacement_round, replacement_subject)
            .map_err(EffectExecutorError::Runtime)?;

        let mut retired_outbound_subjects = superseded_keys
            .iter()
            .filter_map(|(_, subject)| (*subject != replacement_subject).then_some(*subject))
            .collect::<BTreeSet<_>>();
        if retire_retained && let Some((subject, _)) = self.retained_locked_body.as_ref() {
            retired_outbound_subjects.insert(*subject);
        }
        for subject in retired_outbound_subjects {
            services
                .retire_outbound_payload_for_subject(subject)
                .map_err(service_error)?;
        }
        for plan in &fetches {
            services
                .cancel_body_fetch(&plan.pending.task)
                .map_err(service_error)?;
        }
        for (id, _) in &stores {
            services.cancel_body_store(*id).map_err(service_error)?;
        }
        for id in &validations {
            if !self.deferred_merge_work.contains_key(id) {
                services
                    .cancel_body_validation(*id)
                    .map_err(service_error)?;
            }
        }

        for plan in fetches {
            self.commit_pending_fetch_retirement(plan);
        }
        for (id, _) in stores {
            self.pending_stores.remove(&id);
        }
        for id in validations {
            self.deferred_merge_work.remove(&id);
            self.pending_validations.remove(&id);
        }
        self.ready_bodies
            .retain(|key, _| !superseded_keys.contains(key));
        self.body_pipeline_owners
            .retain(|key, _| !superseded_keys.contains(key));
        if retire_retained {
            self.retained_locked_body = None;
        }
        self.ready_body_bytes = accounting.ready_after;
        self.pending_store_bytes = accounting.store_after;
        self.protected_lock = Some(replacement);
        Ok(true)
    }

    /// Retain the exact locked body for a follower's current-view proposal.
    ///
    /// Locked bytes are immutable across views, but their DA manifest,
    /// durable receipt, and validation receipt remain round-bound. The runner
    /// therefore retains one bounded, view-independent copy and stages it under
    /// each authenticated current-round proposal. The ordinary
    /// `BodyAvailable -> StoreBody -> ValidateBody` pipeline still mints fresh
    /// current-view evidence. Validation may promote the byte-identical body's
    /// earlier durable execution witness under the immutable height context;
    /// it never reuses the old round-bound marker itself. If acquisition already
    /// started, the trusted local bytes finish that exact fetch immediately and
    /// retire its network owner.
    pub(crate) fn retain_locked_body_for_reproposal<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
        canonical_wire: Vec<u8>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        if tag.height() != self.context.height {
            return Err(EffectExecutorError::Contract(
                "retained locked body belongs to a different height".to_owned(),
            ));
        }
        let round = wire::ConsensusRound {
            context_id: self.context.id(),
            height: tag.height(),
            view: tag.view(),
        };
        let ready = ReadyBody::derive(&self.context, round, subject, canonical_wire)
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        let key = (round, subject);

        let retention = self.plan_retained_locked_body(subject, Arc::clone(&ready.bytes))?;

        if let Some(existing) = self.ready_bodies.get(&key) {
            if existing.manifest != ready.manifest || existing.bytes != ready.bytes {
                return Err(EffectExecutorError::Contract(
                    "retained locked body conflicts with staged current-view bytes".to_owned(),
                ));
            }
            self.commit_retained_locked_body(retention);
            return Ok(());
        }
        if self.body_pipeline_owners.contains_key(&key)
            && !self
                .pending_fetches
                .values()
                .any(|pending| pending.task.round == round && pending.task.subject == subject)
        {
            self.commit_retained_locked_body(retention);
            return Ok(());
        }

        if let Some(task) = self.pending_fetches.values().find_map(|pending| {
            (pending.task.round == round && pending.task.subject == subject)
                .then(|| pending.task.clone())
        }) {
            if task.tag != tag || !task.matches_reconstructed_manifest(&ready.manifest) {
                self.commit_retained_locked_body(retention);
                return Ok(());
            }
            let plan = self
                .plan_fetch_completion(&task, ready, Some(&retention), services)
                .map_err(|error| match error {
                    EffectTransportError::Backpressure => EffectExecutorError::ReadyBodyCapacity,
                    EffectTransportError::FailClosed(reason) => {
                        EffectExecutorError::Service(reason)
                    }
                    error => EffectExecutorError::Contract(error.to_string()),
                })?;
            if let Err(error) = services.complete_body_reconstruction_fetch(&task) {
                self.abort_fetch_completion(plan);
                let error = self.fail_closed_transport(error, services);
                return Err(EffectExecutorError::Service(error.to_string()));
            }
            self.commit_retained_locked_body(retention);
            self.commit_fetch_completion(plan);
            return self.publish_status(services);
        }

        let ready_plan =
            self.plan_ready_body_install_with_retention(key, ready, None, Some(&retention))?;
        self.commit_retained_locked_body(retention);
        self.commit_ready_body_install(ready_plan);
        self.publish_status(services)
    }

    /// Consume startup or reducer effects in their exact emitted order.
    pub(crate) fn consume_effects<S: V2EffectServices>(
        &mut self,
        effects: Vec<AdapterEffect>,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        if let Err(error) = self.retain_effect_batch(effects) {
            return Err(self.close(error, services));
        }
        self.drain_retained_effect_batch(services)
            .map_err(|error| self.close(error, services))
    }

    /// Install one complete reducer transition before dispatching any prefix.
    ///
    /// Rejecting a second batch while debt exists is deliberate: only the
    /// serialized runtime can establish causal order, and it is not stepped
    /// again until this suffix drains. The bound is shared with the reducer's
    /// source-level transition contract.
    fn retain_effect_batch(
        &mut self,
        effects: Vec<AdapterEffect>,
    ) -> Result<(), EffectExecutorError> {
        if self.retained_effect_batch.is_some() {
            return Err(EffectExecutorError::Contract(
                "a second reducer effect batch overtook retained causal dispatch debt".to_owned(),
            ));
        }
        if effects.len() > MAX_EFFECTS_PER_STEP {
            return Err(EffectExecutorError::Contract(format!(
                "one reducer transition emitted {} effects above the source bound {MAX_EFFECTS_PER_STEP}",
                effects.len()
            )));
        }
        if effects.is_empty() {
            return Ok(());
        }
        debug_assert!(effects.iter().all(|effect| {
            Self::restart_effect_source(effect) != RestartEffectSource::DiagnosticOnly
                || Self::pending_work_producer(effect).is_none()
        }));
        self.retained_effect_batch = Some(RetainedEffectBatch {
            effects: effects.into(),
            oldest_at: Instant::now(),
        });
        Ok(())
    }

    /// Drain the retained causal suffix in exact FIFO order.
    ///
    /// Pending-work exhaustion is the sole retryable adapter error. Every
    /// other boundary failure remains fail-closed. Durable Decision ownership
    /// is reconciled before every attempt so a suffix retained across a local
    /// completion cannot resurrect work finality retired in the meantime.
    fn drain_retained_effect_batch<S: V2EffectServices>(
        &mut self,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        let decision = self.reconcile_runtime_decision(services)?;
        if let Some(decision) = decision
            && let Some(batch) = self.retained_effect_batch.as_mut()
        {
            // A single serialized runtime step may drain work that was queued before a
            // CommitQC and then install that Decision later in the same returned batch.
            // Decision reconciliation has already retired every competing owner, including
            // outbound proposal chunks. Retire the corresponding in-flight effects as well:
            // dispatching them would either resurrect terminal work or ask the transport for
            // chunks that finality has deliberately released.
            batch
                .effects
                .retain(|effect| Self::effect_survives_decision(effect, decision));
        }
        if self
            .retained_effect_batch
            .as_ref()
            .is_some_and(|batch| batch.effects.is_empty())
        {
            self.retained_effect_batch = None;
        }

        let mut consumed = 0usize;
        loop {
            let Some(effect) = self
                .retained_effect_batch
                .as_ref()
                .and_then(|batch| batch.effects.front())
                .cloned()
            else {
                break;
            };
            let pending_work_producer = Self::pending_work_producer(&effect);
            match self.consume_one(effect, services) {
                Ok(()) => {
                    let batch = self
                        .retained_effect_batch
                        .as_mut()
                        .expect("retained effect existed before successful dispatch");
                    batch.effects.pop_front();
                    consumed = consumed.checked_add(1).ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "retained effect dispatch count overflowed".to_owned(),
                        )
                    })?;
                    if batch.effects.is_empty() {
                        self.retained_effect_batch = None;
                        break;
                    }
                }
                Err(EffectExecutorError::PendingWorkCapacity { .. }) => {
                    debug_assert!(pending_work_producer.is_some());
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        self.publish_status(services)?;
        Ok(consumed)
    }

    fn pending_work_producer(effect: &AdapterEffect) -> Option<PendingWorkProducer> {
        match effect {
            AdapterEffect::Sign { .. } => Some(PendingWorkProducer::Sign),
            AdapterEffect::FetchBody { .. } => Some(PendingWorkProducer::Fetch),
            AdapterEffect::StoreBody { .. } => Some(PendingWorkProducer::Store),
            AdapterEffect::ValidateBody { .. } => Some(PendingWorkProducer::Validate),
            AdapterEffect::Apply { .. } => Some(PendingWorkProducer::Apply),
            AdapterEffect::Broadcast(_)
            | AdapterEffect::EnterView { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => None,
        }
    }

    fn restart_effect_source(effect: &AdapterEffect) -> RestartEffectSource {
        match effect {
            AdapterEffect::Sign { .. } | AdapterEffect::Broadcast(_) => {
                RestartEffectSource::DurableConsensusEvidence
            }
            AdapterEffect::FetchBody { .. } | AdapterEffect::StoreBody { .. } => {
                RestartEffectSource::BodyReconstruction
            }
            AdapterEffect::ValidateBody { .. } => RestartEffectSource::DurableBody,
            AdapterEffect::Apply { .. } => RestartEffectSource::DurableDecision,
            AdapterEffect::EnterView { .. } => RestartEffectSource::RecoveredView,
            AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => {
                RestartEffectSource::DiagnosticOnly
            }
        }
    }

    /// Return whether an already-emitted effect remains owned after durable finality.
    ///
    /// The exact CommitQC may still need propagation, and the exact decided body must finish its
    /// local recovery/application pipeline. Diagnostic reports do not create consensus work.
    /// Every other effect belongs to a pre-Decision transition and is terminally stale.
    fn effect_survives_decision(
        effect: &AdapterEffect,
        decision: (
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        ),
    ) -> bool {
        let (decision_round, decision_subject, decision_commitment) = decision;
        match effect {
            AdapterEffect::Broadcast(message) => matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate)
                    if certificate.phase == wire::GlobalPhase::Commit
                        && certificate.round == decision_round
                        && certificate.subject == decision_subject
                        && certificate.execution_commitment == decision_commitment
            ),
            AdapterEffect::FetchBody { round, subject, .. }
            | AdapterEffect::StoreBody { round, subject, .. }
            | AdapterEffect::ValidateBody { round, subject, .. } => {
                (*round, *subject) == (decision_round, decision_subject)
            }
            AdapterEffect::Apply {
                subject,
                certificate,
                ..
            } => {
                *subject == decision_subject
                    && certificate.phase == wire::GlobalPhase::Commit
                    && certificate.round == decision_round
                    && certificate.subject == decision_subject
                    && certificate.execution_commitment == decision_commitment
            }
            AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => true,
            AdapterEffect::Sign { .. } | AdapterEffect::EnterView { .. } => false,
        }
    }

    /// Consume only the local exact-body/application pipeline permitted while recovering an
    /// interrupted canonical Kura tip.
    ///
    /// Recovery begins with a cryptographically authenticated Decision plus an exact durable body
    /// and validation marker. It must therefore never sign, broadcast, fetch from peers, enter a
    /// view, or report network-derived evidence. The reducer still replays its ordinary
    /// `FetchBody -> StoreBody -> ValidateBody -> Apply` state transitions, but every step is
    /// required to resolve from the already reopened local catalogs before its effect is
    /// dispatched.
    pub(crate) fn consume_pending_tip_recovery_effects<S: V2EffectServices>(
        &mut self,
        effects: Vec<AdapterEffect>,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        if effects.len() > MAX_EFFECTS_PER_STEP {
            return Err(self.close(
                EffectExecutorError::Contract(format!(
                    "one recovery transition emitted {} effects above the source bound {MAX_EFFECTS_PER_STEP}",
                    effects.len()
                )),
                services,
            ));
        }
        for effect in &effects {
            if let Err(error) = self.ensure_pending_tip_recovery_effect_is_local(effect) {
                return Err(self.close(error, services));
            }
        }
        if let Err(error) = self.retain_effect_batch(effects) {
            return Err(self.close(error, services));
        }
        self.drain_retained_effect_batch(services)
            .map_err(|error| self.close(error, services))
    }

    /// Run at most one serialized runtime step and dispatch all of its effects.
    pub(crate) fn step<S: V2EffectServices>(
        &mut self,
        now: Instant,
        services: &mut S,
    ) -> Result<EffectExecutorStep, EffectExecutorError> {
        self.ensure_open()?;
        if self.retained_effect_batch.is_some() {
            let count = self
                .drain_retained_effect_batch(services)
                .map_err(|error| self.close(error, services))?;
            return Ok(if count == 0 {
                EffectExecutorStep::Idle
            } else {
                EffectExecutorStep::Advanced { effects: count }
            });
        }
        let wal_step = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| {
                EffectExecutorError::FailClosed(
                    "process restart is required after a fatal consensus failure".to_owned(),
                )
            })?;
        let step = match self.runtime.step_effects(now) {
            Ok(step) => step,
            Err(reason) => {
                drop(wal_step);
                return Err(self.close(EffectExecutorError::Runtime(reason), services));
            }
        };
        // Runtime stepping includes the safety-WAL append. Release its permit
        // before invoking any service callback so service operations acquire
        // their own non-nested guard boundary.
        wal_step.complete();
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

    /// Run one serialized recovery step without allowing any network-producing effect.
    pub(crate) fn step_pending_tip_recovery<S: V2EffectServices>(
        &mut self,
        now: Instant,
        services: &mut S,
    ) -> Result<EffectExecutorStep, EffectExecutorError> {
        self.ensure_open()?;
        if self.retained_effect_batch.is_some() {
            let count = self
                .drain_retained_effect_batch(services)
                .map_err(|error| self.close(error, services))?;
            return Ok(if count == 0 {
                EffectExecutorStep::Idle
            } else {
                EffectExecutorStep::Advanced { effects: count }
            });
        }
        let wal_step = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| {
                EffectExecutorError::FailClosed(
                    "process restart is required after a fatal consensus failure".to_owned(),
                )
            })?;
        let step = match self.runtime.step_recovery_effects(now) {
            Ok(step) => step,
            Err(reason) => {
                drop(wal_step);
                return Err(self.close(EffectExecutorError::Runtime(reason), services));
            }
        };
        wal_step.complete();
        match step {
            RuntimeStep::Idle => {
                if let Err(error) = self.publish_status(services) {
                    return Err(self.close(error, services));
                }
                Ok(EffectExecutorStep::Idle)
            }
            RuntimeStep::Advanced(effects) => {
                let count = self.consume_pending_tip_recovery_effects(effects, services)?;
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
        let owner_plan = self.plan_body_pipeline_owner(tag, &manifest)?;
        if let Err(error) = self.begin_store_with_plans(
            tag,
            manifest,
            Arc::from(canonical_wire),
            StorePurpose::LocalProposal,
            None,
            Some(owner_plan),
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
        Ok(CompletionDisposition::Accepted)
    }

    /// Accept a body-store-minted durable completion under its immutable task.
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
            let manifest_hash = HashOf::new(&manifest);
            let retained_hash = self
                .retained_body_manifest_hash(key)
                .map_err(|error| self.close(error, services))?;
            if retained_hash.is_some_and(|retained| retained != manifest_hash)
                || self.recovered_bodies.get(&key).is_some_and(
                    |(existing_manifest, existing_receipt)| {
                        existing_manifest != &manifest || existing_receipt != &receipt
                    },
                )
                || self
                    .durable_bodies
                    .get(&key)
                    .is_some_and(|existing| existing != &receipt)
            {
                return Err(self.close(
                    EffectExecutorError::BodyStore(
                        "late body-store completion conflicts with retained exact-body ownership"
                            .to_owned(),
                    ),
                    services,
                ));
            }
            self.recovered_bodies
                .entry(key)
                .or_insert_with(|| (manifest, receipt.clone()));
            self.durable_bodies.entry(key).or_insert(receipt);
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
        if let Some(consumer) = &pending.consumer {
            let consumer_tag = match consumer {
                StoreConsumer::Reducer { tag } | StoreConsumer::LocalProposal { tag } => *tag,
            };
            if !self.exact_body_pipeline_stage_owned(consumer_tag, key, HashOf::new(&manifest)) {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "body-store completion consumer differs from its immutable pipeline owner"
                            .to_owned(),
                    ),
                    services,
                ));
            }
        }
        let stored_bytes = u64::try_from(pending.task.canonical_wire.len()).map_err(|_| {
            self.close(
                EffectExecutorError::Contract(
                    "pending-store byte count is not representable".to_owned(),
                ),
                services,
            )
        })?;
        let pending_store_bytes = self
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
        if self
            .recovered_bodies
            .get(&key)
            .is_some_and(|(existing_manifest, existing_receipt)| {
                existing_manifest != &manifest || existing_receipt != &receipt
            })
            || self
                .durable_bodies
                .get(&key)
                .is_some_and(|existing| existing != &receipt)
        {
            return Err(self.close(
                EffectExecutorError::BodyStore(
                    "body-store completion conflicts with retained durable ownership".to_owned(),
                ),
                services,
            ));
        }
        let validation_plan = match &pending.consumer {
            Some(StoreConsumer::LocalProposal { tag }) => Some(
                self.plan_begin_validation(
                    manifest.round,
                    manifest.subject,
                    receipt.clone(),
                    ValidationConsumer::LocalProposal {
                        tag: *tag,
                        manifest: manifest.clone(),
                    },
                    Some(&receipt),
                    None,
                    Some(completion.work_id()),
                )
                .map_err(|error| self.close(error, services))?,
            ),
            Some(StoreConsumer::Reducer { .. }) | None => None,
        };
        match &pending.consumer {
            Some(StoreConsumer::Reducer { tag }) => self
                .runtime
                .enqueue_body_stored(*tag, manifest.round, manifest.subject, receipt.clone())
                .map_err(runtime_enqueue_error)
                .map_err(|error| self.close(error, services))?,
            Some(StoreConsumer::LocalProposal { .. }) => self
                .admit_validation_start(
                    validation_plan
                        .as_ref()
                        .expect("local proposal preflighted validation"),
                    services,
                )
                .map_err(|error| self.close(error, services))?,
            None => {}
        }
        self.pending_stores.remove(&completion.work_id());
        self.pending_store_bytes = pending_store_bytes;
        self.recovered_bodies
            .insert(key, (manifest.clone(), receipt.clone()));
        self.durable_bodies.insert(key, receipt);
        if let Some(plan) = validation_plan {
            self.commit_validation_start(plan);
        }
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Accepted)
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
                if let Err(error) = self.record_validated_body(validated.clone()) {
                    return Err(self.close(error, services));
                }
            }
            return Ok(CompletionDisposition::Stale);
        };
        let round = pending.task.round();
        let subject = pending.task.subject();
        if let Err(error) =
            self.preflight_pending_validation_consumer(completion.work_id(), &pending)
        {
            return Err(self.close(error, services));
        }
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
        let rejection_reason = if let Some(validated) = completion.validated_receipt().cloned() {
            if validated.durable() != pending.task.durable_receipt() {
                return Err(self.close(
                    EffectExecutorError::BodyStore(
                        "validation completion covers a different durable body".to_owned(),
                    ),
                    services,
                ));
            }
            if let Err(error) = self.preflight_validated_body(&validated) {
                return Err(self.close(error, services));
            }
            let admission = match &pending.consumer {
                Some(ValidationConsumer::Reducer { tag }) => self
                    .runtime
                    .enqueue_validation_succeeded(*tag, round, subject, validated.clone())
                    .map_err(runtime_enqueue_error),
                Some(ValidationConsumer::LocalProposal { tag, manifest }) => self
                    .runtime
                    .enqueue_local_proposal(
                        *tag,
                        manifest.clone(),
                        pending.task.durable_receipt().clone(),
                        validated.clone(),
                    )
                    .map_err(runtime_enqueue_error),
                None => Ok(()),
            };
            if let Err(error) = admission {
                return Err(self.close(error, services));
            }
            let durable = validated.durable().clone();
            self.durable_bodies.entry(key).or_insert(durable);
            self.validated_bodies.entry(key).or_insert(validated);
            None
        } else {
            let reason = completion
                .rejection_reason()
                .ok_or_else(|| {
                    EffectExecutorError::BodyStore(
                        "validation completion has neither receipt nor rejection".to_owned(),
                    )
                })?
                .to_owned();
            if let Err(error) = self.preflight_rejected_body(key, pending.task.durable_receipt()) {
                return Err(self.close(error, services));
            }
            let admission = match &pending.consumer {
                Some(ValidationConsumer::Reducer { tag }) => self
                    .runtime
                    .enqueue_validation_failed(*tag, round, subject)
                    .map_err(runtime_enqueue_error),
                Some(ValidationConsumer::LocalProposal { .. }) | None => Ok(()),
            };
            if let Err(error) = admission {
                return Err(self.close(error, services));
            }
            let durable = pending.task.durable_receipt().clone();
            self.durable_bodies.entry(key).or_insert(durable.clone());
            self.rejected_bodies.entry(key).or_insert(durable);
            Some(reason)
        };
        self.deferred_merge_work.remove(&completion.work_id());
        self.pending_validations.remove(&completion.work_id());
        if let Some(reason) = rejection_reason {
            services.validation_rejected(round, subject, &reason);
        }
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Accepted)
    }

    /// Retry every retained validation or Apply task waiting for one exact
    /// certified merge entry after authentication and durable installation.
    ///
    /// The complete matching owner set is preflighted before callbacks. The
    /// pending tasks and work identifiers are reused verbatim, and deferred
    /// entries are removed only after every enqueue succeeds. A service
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
            if let Err(error) = self.preflight_deferred_work_owner(*work_id) {
                return Err(self.close(error, services));
            }
        }
        enum RetryTask {
            Validation(BodyValidationTask),
            Application(ApplyTask),
        }
        let plans = work_ids
            .iter()
            .map(|work_id| {
                match (
                    self.pending_validations.get(work_id),
                    self.pending_applications.get(work_id),
                ) {
                    (Some(pending), None) => Ok(RetryTask::Validation(pending.task.clone())),
                    (None, Some(pending)) => Ok(RetryTask::Application(pending.task.clone())),
                    (Some(_), Some(_)) => Err(EffectExecutorError::Contract(
                        "deferred merge sidecar has conflicting validation and application owners"
                            .to_owned(),
                    )),
                    (None, None) => Err(EffectExecutorError::Contract(
                        "deferred merge sidecar has no pending validation or application task"
                            .to_owned(),
                    )),
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| self.close(error, services))?;
        for plan in &plans {
            let result = match plan {
                RetryTask::Validation(task) => services.enqueue_body_validation(task.clone()),
                RetryTask::Application(task) => services.enqueue_apply(task.clone()),
            };
            if let Err(error) = result {
                return Err(self.close(service_error(error), services));
            }
        }
        for work_id in &work_ids {
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
    /// Validation owners and catalog updates are planned as one set; their
    /// reducer completions are atomically admitted before any waiter is
    /// removed.
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
        for work_id in &work_ids {
            if let Err(error) = self.preflight_deferred_work_owner(*work_id) {
                return Err(self.close(error, services));
            }
        }
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
        let mut keys = BTreeSet::new();
        let mut plans = Vec::with_capacity(work_ids.len());
        let mut failures = Vec::new();
        for work_id in &work_ids {
            let pending = self.pending_validations.get(work_id).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "deferred merge sidecar has no pending validation task".to_owned(),
                )
            });
            let pending = match pending {
                Ok(pending) => pending,
                Err(error) => return Err(self.close(error, services)),
            };
            let round = pending.task.round();
            let subject = pending.task.subject();
            let key = (round, subject);
            if !keys.insert(key) {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "one exact body has multiple deferred validation owners".to_owned(),
                    ),
                    services,
                ));
            }
            let durable = pending.task.durable_receipt().clone();
            if let Err(error) = self.preflight_rejected_body(key, &durable) {
                return Err(self.close(error, services));
            }
            if let Some(ValidationConsumer::Reducer { tag }) = &pending.consumer {
                failures.push((*tag, round, subject));
            }
            plans.push((*work_id, key, durable));
        }
        if let Err(error) = self
            .runtime
            .enqueue_validation_failures_atomically(&failures)
        {
            return Err(self.close(runtime_enqueue_error(error), services));
        }
        for (work_id, key, durable) in plans {
            self.durable_bodies.entry(key).or_insert(durable.clone());
            self.rejected_bodies.entry(key).or_insert(durable);
            self.deferred_merge_work.remove(&work_id);
            self.pending_validations.remove(&work_id);
            services.validation_rejected(key.0, key.1, &reason);
        }
        if !work_ids.is_empty() {
            self.publish_status(services)
                .map_err(|error| self.close(error, services))?;
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
        if let Err(error) = self.preflight_pending_application_owner(work_id, pending) {
            return Err(self.close(error, services));
        }
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
        if let Err(error) = self.preflight_deferred_work_owner(work_id) {
            return Err(self.close(error, services));
        }
        if self.pending_applications.contains_key(&work_id) {
            return Err(self.close(
                EffectExecutorError::BodyStore(
                    "decided Apply task could not register its certified merge sidecar".to_owned(),
                ),
                services,
            ));
        }
        if !self.pending_validations.contains_key(&work_id) {
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
        if self.output_guard.restart_required() {
            return Err(EffectTransportError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            ));
        }
        if let Some(reason) = &self.fatal_reason {
            return Err(EffectTransportError::FailClosed(reason.clone()));
        }
        let task = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?
            .task
            .clone();
        let manifest = task
            .manifest
            .as_ref()
            .ok_or(EffectTransportError::WrongFetchKind)?;
        let authenticated =
            authenticate_payload_chunk(&self.context, manifest, chunk, authenticated_sender)?;
        match services.accept_authenticated_chunk(&task, authenticated) {
            Ok(AuthenticatedChunkDisposition::Accepted) => {}
            Ok(AuthenticatedChunkDisposition::Rejected) => {
                self.reject_noncanonical_reconstruction(work_id, services)?;
                return Err(EffectTransportError::BodyMismatch(
                    "authenticated chunks reconstructed invalid or noncanonical body data",
                ));
            }
            Err(error) => {
                let reason = EffectExecutorError::Service(error.to_string()).to_string();
                self.fatal_reason = Some(reason.clone());
                services.fail_closed(&reason);
                return Err(EffectTransportError::FailClosed(reason));
            }
        }
        Ok(())
    }

    /// Complete authenticated-chunk reconstruction, including hybrid fetches.
    pub(crate) fn complete_body_reconstruction<S: V2EffectServices>(
        &mut self,
        task: &BodyFetchTask,
        manifest: wire::PayloadManifest,
        body: impl Into<Arc<[u8]>>,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectTransportError> {
        if self.output_guard.restart_required() {
            return Err(EffectTransportError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            ));
        }
        let work_id = task.id();
        let pending = self
            .pending_fetches
            .get(&work_id)
            .cloned()
            .ok_or(EffectTransportError::UnknownWork(work_id))?;
        if pending.task != *task {
            return Err(EffectTransportError::BodyMismatch(
                "completion task differs from executor ownership",
            ));
        }
        if let Some(hash) = pending.request_hash
            && self.certified_work.get(&hash) != Some(&work_id)
        {
            return Err(self.fail_closed_transport(
                "hybrid body reconstruction has mismatched certified-request ownership",
                services,
            ));
        }
        let ready_body =
            ReadyBody::derive(&self.context, task.round, task.subject, body).map_err(|_| {
                EffectTransportError::BodyMismatch(
                    "reconstructed body cannot reproduce its canonical chunk manifest",
                )
            })?;
        if ready_body.manifest != manifest {
            return self.reject_noncanonical_reconstruction(work_id, services);
        }
        self.finish_fetch(work_id, ready_body, services)
    }

    fn reject_noncanonical_reconstruction<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectTransportError> {
        let Some(pending) = self.pending_fetches.get(&work_id).cloned() else {
            return Err(self.fail_closed_transport(
                "noncanonical reconstruction lost its exact pending fetch",
                services,
            ));
        };
        let key = (pending.task.round, pending.task.subject);
        let certified_retirement = pending
            .request_hash
            .map(|request_hash| {
                self.plan_certified_fetch_retirement(work_id, request_hash)
                    .map_err(|error| self.fail_closed_transport(error, services))
            })
            .transpose()?;
        let Some(owner) = self.body_pipeline_owners.get(&key).copied() else {
            return Err(self.fail_closed_transport(
                "noncanonical reconstruction lost its reducer pipeline owner",
                services,
            ));
        };
        if owner.tag != pending.task.tag {
            return Err(self.fail_closed_transport(
                "noncanonical reconstruction found a conflicting reducer pipeline owner",
                services,
            ));
        }
        if let Err(error) = services.complete_body_reconstruction_fetch(&pending.task) {
            return Err(self.fail_closed_transport(error, services));
        }
        self.pending_fetches.remove(&work_id);
        self.body_pipeline_owners.remove(&key);
        if let Some(retirement) = certified_retirement {
            self.commit_certified_fetch_retirement(retirement);
        }
        Ok(CompletionDisposition::Rejected)
    }

    /// Authenticate a certified response against the exact outstanding signed
    /// request, rederive its canonical DA manifest, then enqueue body
    /// availability with the original fetch tag.
    pub(crate) fn accept_certified_body_response<S: V2EffectServices>(
        &mut self,
        response: wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectTransportError> {
        if self.output_guard.restart_required() {
            return Err(EffectTransportError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            ));
        }
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
        let Some(pending) = self.pending_fetches.get(&work_id) else {
            return Err(self.fail_closed_transport(
                "certified body response has no exact pending fetch",
                services,
            ));
        };
        if pending.request_hash != Some(response.request_hash) {
            return Err(self.fail_closed_transport(
                "certified body response differs from pending request ownership",
                services,
            ));
        }
        let task = pending.task.clone();
        if !task.matches_reconstructed_manifest(&response.manifest) {
            return Err(EffectTransportError::BodyMismatch(
                "certified response manifest differs from proposal authority",
            ));
        }
        let authenticated = self.outstanding_requests.authenticate_response(
            &self.context,
            response,
            authenticated_responder,
        )?;
        let response = authenticated.into_inner();
        let response_manifest = response.manifest;
        let ready_body = ReadyBody::derive(&self.context, task.round, task.subject, response.body)
            .map_err(|_| {
                EffectTransportError::BodyMismatch(
                    "certified body cannot reproduce its canonical chunk manifest",
                )
            })?;
        if ready_body.manifest != response_manifest {
            return Err(EffectTransportError::BodyMismatch(
                "certified response manifest is not canonical for its body",
            ));
        }
        let plan = self.plan_fetch_completion(&task, ready_body, None, services)?;
        if let Err(error) = services.complete_certified_body_fetch(&task) {
            self.abort_fetch_completion(plan);
            return Err(self.fail_closed_transport(error, services));
        }
        self.commit_fetch_completion(plan);
        Ok(CompletionDisposition::Accepted)
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
        Ok(CompletionDisposition::Accepted)
    }

    /// Current bounded operational status.
    pub(crate) fn status(&self) -> EffectExecutorStatus {
        let restart_required = self.output_guard.restart_required();
        let captured_at = Instant::now();
        let deferred_validation_merge_work = self
            .deferred_merge_work
            .keys()
            .filter(|work_id| self.pending_validations.contains_key(*work_id))
            .count();
        let deferred_application_merge_work = self
            .deferred_merge_work
            .keys()
            .filter(|work_id| self.pending_applications.contains_key(*work_id))
            .count();
        EffectExecutorStatus {
            height_context_id: self.context.id(),
            height: self.context.height,
            captured_at,
            fail_closed: self.fatal_reason.is_some() || restart_required,
            fatal_reason: self.fatal_reason.clone().or_else(|| {
                restart_required.then(|| {
                    "process restart is required after a fatal consensus failure".to_owned()
                })
            }),
            pending_signatures: self.pending_signatures.len(),
            // The production service overlays its height-local disk acquisition
            // ownership when this executor snapshot crosses that boundary.
            pending_candidate_loads: 0,
            pending_fetches: self.pending_fetches.len(),
            pending_stores: self.pending_stores.len(),
            pending_validations: self.pending_validations.len(),
            deferred_validation_merge_work,
            deferred_application_merge_work,
            deferred_merge_work: self.deferred_merge_work.len(),
            pending_applications: self.pending_applications.len(),
            ready_bodies: self.ready_bodies.len(),
            ready_body_bytes: self.ready_body_bytes,
            pending_store_bytes: self.pending_store_bytes,
            queued_runtime_completions: self.runtime.queued_commands(),
            // The executor does not own the upstream worker queue. Production
            // services replace this empty sentinel in `publish_effect_status`.
            effect_completion_queue: RuntimeQueueLaneSnapshot {
                depth: 0,
                capacity: 1,
                oldest_age: None,
                max_service_debt: 0,
            },
            effect_dispatch_queue: self.effect_dispatch_queue_snapshot(captured_at),
            runtime_queues: self.runtime.queue_snapshot(captured_at),
            watchdog_threshold: self.runtime.watchdog_threshold(),
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
                self.ensure_signature_slot(services)?;
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
            AdapterEffect::EnterView {
                tag,
                certificate,
                protected_body,
            } => self.install_view(tag, certificate, protected_body, services),
            AdapterEffect::ReportEquivocation {
                offender,
                round,
                kind,
            } => services
                .report_equivocation(offender, round, kind)
                .map_err(service_error),
            AdapterEffect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => services
                .report_invalid_certified_body(subject, certificate)
                .map_err(service_error),
        }
    }

    fn ensure_pending_tip_recovery_effect_is_local(
        &self,
        effect: &AdapterEffect,
    ) -> Result<(), EffectExecutorError> {
        let local_only_error = || {
            EffectExecutorError::Contract(
                "interrupted-tip recovery attempted a non-local consensus effect before finality"
                    .to_owned(),
            )
        };
        match effect {
            AdapterEffect::FetchBody { round, subject, .. } => self
                .recovered_bodies
                .contains_key(&(*round, *subject))
                .then_some(())
                .ok_or_else(local_only_error),
            AdapterEffect::StoreBody { round, subject, .. } => self
                .durable_bodies
                .contains_key(&(*round, *subject))
                .then_some(())
                .ok_or_else(local_only_error),
            AdapterEffect::ValidateBody { round, subject, .. }
            | AdapterEffect::Apply {
                subject,
                certificate: wire::QuorumCertificate { round, .. },
                ..
            } => self
                .validated_bodies
                .contains_key(&(*round, *subject))
                .then_some(())
                .ok_or_else(local_only_error),
            AdapterEffect::Sign { .. }
            | AdapterEffect::Broadcast(_)
            | AdapterEffect::EnterView { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => Err(local_only_error()),
        }
    }

    #[cfg(test)]
    fn bind_body_pipeline_owner(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, EffectExecutorError> {
        self.bind_body_pipeline_owner_hash(
            tag,
            (manifest.round, manifest.subject),
            Some(HashOf::new(manifest)),
        )
    }

    #[cfg(test)]
    fn bind_body_pipeline_owner_hash(
        &mut self,
        tag: EventTag,
        key: (wire::ConsensusRound, wire::BlockSubject),
        manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    ) -> Result<bool, EffectExecutorError> {
        let plan = self.plan_body_pipeline_owner_hash(tag, key, manifest_hash)?;
        let already_owned = plan.already_owned;
        self.commit_body_pipeline_owner(plan);
        Ok(already_owned)
    }

    fn plan_body_pipeline_owner_hash(
        &self,
        tag: EventTag,
        key: (wire::ConsensusRound, wire::BlockSubject),
        manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    ) -> Result<BodyPipelineOwnerBindingPlan, EffectExecutorError> {
        let incoming = Self::project_body_pipeline_owner(tag, key, manifest_hash);
        let current =
            self.body_pipeline_owners.get(&key).copied().map(|owner| {
                Self::project_body_pipeline_owner(owner.tag, key, owner.manifest_hash)
            });
        let Some(binding) = plan_exact_body_owner_binding(current, incoming) else {
            let reason = if current.is_some_and(|owner| owner.tag != incoming.tag) {
                "one exact body pipeline has conflicting reducer ownership"
            } else {
                "one exact body pipeline has conflicting manifest ownership"
            };
            return Err(EffectExecutorError::Contract(reason.to_owned()));
        };
        Ok(BodyPipelineOwnerBindingPlan {
            key,
            owner: BodyPipelineOwner {
                tag,
                manifest_hash: binding.owner.manifest_hash,
            },
            already_owned: binding.already_owned,
        })
    }

    fn project_body_pipeline_owner(
        tag: EventTag,
        key: (wire::ConsensusRound, wire::BlockSubject),
        manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    ) -> ExactBodyOwnerProjection<
        (wire::ConsensusRound, wire::BlockSubject),
        HashOf<wire::PayloadManifest>,
    > {
        ExactBodyOwnerProjection {
            tag: TagProjection {
                height: tag.height(),
                view: tag.view(),
                generation: tag.generation().get(),
            },
            key,
            manifest_hash,
        }
    }

    fn exact_body_pipeline_stage_owned(
        &self,
        tag: EventTag,
        key: (wire::ConsensusRound, wire::BlockSubject),
        manifest_hash: HashOf<wire::PayloadManifest>,
    ) -> bool {
        self.body_pipeline_owners.get(&key).is_some_and(|owner| {
            exact_body_stage_is_owned(
                Self::project_body_pipeline_owner(owner.tag, key, owner.manifest_hash),
                Self::project_body_pipeline_owner(tag, key, Some(manifest_hash)),
            )
        })
    }

    fn plan_body_pipeline_owner_rebind(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        previous_tag: EventTag,
        rebound_tag: EventTag,
        manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    ) -> Result<BodyPipelineOwner, EffectExecutorError> {
        let owner = self
            .body_pipeline_owners
            .get(&key)
            .copied()
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "protected body work lost its reducer pipeline owner".to_owned(),
                )
            })?;
        let current = Self::project_body_pipeline_owner(owner.tag, key, owner.manifest_hash);
        let previous = Self::project_body_pipeline_owner(previous_tag, key, manifest_hash);
        if !exact_body_stage_is_owned(current, previous) {
            return Err(EffectExecutorError::Contract(
                "protected body work differs from its immutable pipeline owner".to_owned(),
            ));
        }
        let rebound = plan_exact_body_owner_rebind(
            current,
            previous,
            TagProjection {
                height: rebound_tag.height(),
                view: rebound_tag.view(),
                generation: rebound_tag.generation().get(),
            },
        )
        .ok_or_else(|| {
            EffectExecutorError::Contract(
                "protected body consumer rebind did not strictly advance its incarnation"
                    .to_owned(),
            )
        })?;
        Ok(BodyPipelineOwner {
            tag: rebound_tag,
            manifest_hash: rebound.manifest_hash,
        })
    }

    fn plan_body_pipeline_owner(
        &self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<BodyPipelineOwnerBindingPlan, EffectExecutorError> {
        self.plan_body_pipeline_owner_hash(
            tag,
            (manifest.round, manifest.subject),
            Some(HashOf::new(manifest)),
        )
    }

    fn commit_body_pipeline_owner(&mut self, plan: BodyPipelineOwnerBindingPlan) {
        self.body_pipeline_owners.insert(plan.key, plan.owner);
    }

    fn plan_work_id(&self) -> Result<WorkIdPlan, EffectExecutorError> {
        Ok(WorkIdPlan {
            id: EffectWorkId(self.next_work_id),
            next: self
                .next_work_id
                .checked_add(1)
                .ok_or(EffectExecutorError::WorkIdExhausted)?,
        })
    }

    fn commit_work_id(&mut self, plan: WorkIdPlan) {
        self.next_work_id = plan.next;
    }

    fn retained_body_union(&self) -> Result<RetainedBodyUnion, EffectExecutorError> {
        let mut union = RetainedBodyUnion::default();
        if let Some((subject, bytes)) = &self.retained_locked_body {
            union.insert(*subject, Arc::clone(bytes))?;
        }
        for body in self.ready_bodies.values() {
            self.insert_retained_union_manifest(
                &mut union,
                &body.manifest,
                Arc::clone(&body.bytes),
            )?;
        }
        for pending in self.pending_stores.values() {
            self.insert_retained_union_manifest(
                &mut union,
                &pending.task.manifest,
                Arc::clone(&pending.task.canonical_wire),
            )?;
        }
        Ok(union)
    }

    fn insert_retained_union_manifest(
        &self,
        union: &mut RetainedBodyUnion,
        manifest: &wire::PayloadManifest,
        bytes: Arc<[u8]>,
    ) -> Result<(), EffectExecutorError> {
        manifest.validate(&self.context).map_err(|error| {
            EffectExecutorError::Contract(format!(
                "retained canonical-body manifest is invalid: {error}"
            ))
        })?;
        if u64::try_from(bytes.len()).ok() != Some(manifest.payload_size_bytes) {
            return Err(EffectExecutorError::Contract(
                "retained canonical bytes differ from their manifest length".to_owned(),
            ));
        }
        union.insert_manifest(manifest.clone(), bytes)
    }

    fn ensure_retained_body_union_bound(
        &self,
        union: &RetainedBodyUnion,
    ) -> Result<(), EffectExecutorError> {
        if union.total_bytes()? > self.config.max_ready_body_bytes {
            return Err(EffectExecutorError::ReadyBodyCapacity);
        }
        Ok(())
    }

    fn plan_retained_locked_body(
        &self,
        subject: wire::BlockSubject,
        bytes: Arc<[u8]>,
    ) -> Result<RetainedLockedBodyPlan, EffectExecutorError> {
        let body_len = u64::try_from(bytes.len()).map_err(|_| {
            EffectExecutorError::Contract(
                "retained locked-body byte count is not representable".to_owned(),
            )
        })?;
        let install = match self.retained_locked_body.as_ref() {
            Some((retained_subject, retained_bytes)) => {
                if *retained_subject != subject || retained_bytes.as_ref() != bytes.as_ref() {
                    return Err(EffectExecutorError::Contract(
                        "retained locked body conflicts with the current lock".to_owned(),
                    ));
                }
                false
            }
            None => true,
        };
        let ready_body_bytes = if install {
            self.ready_body_bytes
                .checked_add(body_len)
                .ok_or(EffectExecutorError::ReadyBodyCapacity)?
        } else {
            self.ready_body_bytes
        };
        let mut union = self.retained_body_union()?;
        if install {
            union.insert(subject, Arc::clone(&bytes))?;
        }
        self.ensure_retained_body_union_bound(&union)?;
        Ok(RetainedLockedBodyPlan {
            subject,
            bytes,
            install,
            ready_body_bytes,
        })
    }

    fn commit_retained_locked_body(&mut self, plan: RetainedLockedBodyPlan) {
        if plan.install {
            self.retained_locked_body = Some((plan.subject, plan.bytes));
            self.ready_body_bytes = plan.ready_body_bytes;
        }
    }

    fn plan_ready_body_release(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
    ) -> Result<ReadyBodyReleasePlan, EffectExecutorError> {
        let body = self.ready_bodies.get(&key).cloned().ok_or_else(|| {
            EffectExecutorError::Contract(
                "StoreBody has no matching reconstructed exact body".to_owned(),
            )
        })?;
        let bytes = u64::try_from(body.bytes.len()).map_err(|_| {
            EffectExecutorError::Contract("ready-body byte count is not representable".to_owned())
        })?;
        let remaining_ready_bytes = self.ready_body_bytes.checked_sub(bytes).ok_or_else(|| {
            EffectExecutorError::Contract("ready-body byte accounting underflow".to_owned())
        })?;
        Ok(ReadyBodyReleasePlan {
            key,
            body,
            remaining_ready_bytes,
        })
    }

    fn commit_ready_body_release(&mut self, plan: ReadyBodyReleasePlan) {
        self.ready_bodies.remove(&plan.key);
        self.ready_body_bytes = plan.remaining_ready_bytes;
    }

    fn plan_ready_body_install(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        body: ReadyBody,
        release: Option<ReadyBodyReleasePlan>,
    ) -> Result<ReadyBodyInstallPlan, EffectExecutorError> {
        self.plan_ready_body_install_with_retention(key, body, release, None)
    }

    fn plan_ready_body_install_with_retention(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        body: ReadyBody,
        release: Option<ReadyBodyReleasePlan>,
        retention: Option<&RetainedLockedBodyPlan>,
    ) -> Result<ReadyBodyInstallPlan, EffectExecutorError> {
        if body.manifest.round != key.0 || body.manifest.subject != key.1 {
            return Err(EffectExecutorError::Contract(
                "ready-body installation does not name its exact round and subject".to_owned(),
            ));
        }
        if let Some(release) = &release {
            if release.key != key || self.ready_bodies.get(&key) != Some(&release.body) {
                return Err(EffectExecutorError::Contract(
                    "ready-body replacement release no longer owns the exact staged body"
                        .to_owned(),
                ));
            }
            let released_len = u64::try_from(release.body.bytes.len()).map_err(|_| {
                EffectExecutorError::Contract(
                    "ready-body replacement byte count is not representable".to_owned(),
                )
            })?;
            if self.ready_body_bytes.checked_sub(released_len)
                != Some(release.remaining_ready_bytes)
            {
                return Err(EffectExecutorError::Contract(
                    "ready-body replacement release has stale byte accounting".to_owned(),
                ));
            }
        } else if self.ready_bodies.contains_key(&key) {
            return Err(EffectExecutorError::Contract(
                "ready-body installation would replace an unplanned exact owner".to_owned(),
            ));
        }
        let ready_count = self
            .ready_bodies
            .len()
            .checked_sub(usize::from(release.is_some()))
            .ok_or_else(|| {
                EffectExecutorError::Contract("ready-body replacement count underflow".to_owned())
            })?;
        if ready_count >= self.config.max_ready_bodies {
            return Err(EffectExecutorError::ReadyBodyCapacity);
        }
        let body_len = u64::try_from(body.bytes.len()).map_err(|_| {
            EffectExecutorError::Contract("ready-body byte count is not representable".to_owned())
        })?;
        let retained_base = retention.map_or(self.ready_body_bytes, |retention| {
            retention.ready_body_bytes
        });
        let base_ready_bytes = if let Some(release) = &release {
            let released_len = u64::try_from(release.body.bytes.len()).map_err(|_| {
                EffectExecutorError::Contract(
                    "ready-body replacement byte count is not representable".to_owned(),
                )
            })?;
            retained_base.checked_sub(released_len).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "ready-body replacement accounting underflowed after retention".to_owned(),
                )
            })?
        } else {
            retained_base
        };
        let ready_body_bytes = base_ready_bytes
            .checked_add(body_len)
            .ok_or(EffectExecutorError::ReadyBodyCapacity)?;
        let mut union = self.retained_body_union()?;
        if let Some(retention) = retention
            && retention.install
        {
            union.insert(retention.subject, Arc::clone(&retention.bytes))?;
        }
        if let Some(release) = &release {
            union.remove_manifest(&release.body.manifest, release.body.bytes.as_ref())?;
        }
        self.insert_retained_union_manifest(&mut union, &body.manifest, Arc::clone(&body.bytes))?;
        self.ensure_retained_body_union_bound(&union)?;
        Ok(ReadyBodyInstallPlan {
            key,
            body,
            ready_body_bytes,
            release,
        })
    }

    fn commit_ready_body_install(&mut self, plan: ReadyBodyInstallPlan) {
        if let Some(release) = plan.release {
            self.commit_ready_body_release(release);
        }
        self.ready_bodies.insert(plan.key, plan.body);
        self.ready_body_bytes = plan.ready_body_bytes;
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
        } else if manifest.is_none() || !sources.is_empty() {
            return Err(EffectExecutorError::Contract(
                "uncertified FetchBody requires a proposal manifest and no certified sources"
                    .to_owned(),
            ));
        }

        let key = (round, subject);
        let existing_id = self.pending_fetches.iter().find_map(|(id, pending)| {
            (pending.task.round == round && pending.task.subject == subject).then_some(*id)
        });
        if let Some(existing_id) = existing_id {
            let existing = self
                .pending_fetches
                .get(&existing_id)
                .expect("pending fetch ID came from this map")
                .clone();
            if existing.task.tag != tag {
                return Err(EffectExecutorError::Contract(
                    "conflicting retransmission for one body-fetch round/subject".to_owned(),
                ));
            }
            let merged_manifest = match (&existing.task.manifest, manifest) {
                (Some(existing), Some(incoming)) if existing != &incoming => {
                    return Err(EffectExecutorError::Contract(
                        "conflicting retransmission changed a body-fetch manifest".to_owned(),
                    ));
                }
                (Some(existing), _) => Some(existing.clone()),
                (None, incoming) => incoming,
            };

            let (merged_sources, merged_request, request_hash, request_plan) =
                if let Some(request) = existing.task.certified_request.clone() {
                    let request_hash = existing.request_hash.ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "certified body fetch lost its request hash".to_owned(),
                        )
                    })?;
                    if self.certified_work.get(&request_hash) != Some(&existing_id) {
                        return Err(EffectExecutorError::Contract(
                            "certified body fetch has mismatched request ownership".to_owned(),
                        ));
                    }
                    (
                        existing.task.sources.clone(),
                        Some(request),
                        Some(request_hash),
                        None,
                    )
                } else if let Some(certificate) = certificate {
                    let plan = self.plan_certified_fetch_request(
                        existing_id,
                        round,
                        subject,
                        certificate,
                        services,
                    )?;
                    (
                        sources,
                        Some(plan.request.clone()),
                        Some(plan.request_hash),
                        Some(plan),
                    )
                } else {
                    if existing.request_hash.is_some() {
                        return Err(EffectExecutorError::Contract(
                            "ordinary body fetch unexpectedly owns a certified request".to_owned(),
                        ));
                    }
                    (existing.task.sources.clone(), None, None, None)
                };
            let merged = BodyFetchTask {
                id: existing_id,
                tag,
                round,
                subject,
                manifest: merged_manifest,
                sources: merged_sources,
                certified_request: merged_request,
            };
            if !merged.monotonically_extends(&existing.task) {
                return Err(EffectExecutorError::Contract(
                    "body-fetch authority did not advance monotonically".to_owned(),
                ));
            }
            let owner_plan = self.plan_body_pipeline_owner_hash(
                tag,
                key,
                merged.manifest.as_ref().map(HashOf::new),
            )?;
            if !owner_plan.already_owned {
                return Err(EffectExecutorError::Contract(
                    "pending body fetch lost its exact pipeline ownership".to_owned(),
                ));
            }
            services
                .enqueue_body_fetch(merged.clone())
                .map_err(service_error)?;
            if let Some(plan) = request_plan {
                self.commit_certified_fetch_request(plan);
            }
            self.commit_body_pipeline_owner(owner_plan);
            let pending = self
                .pending_fetches
                .get_mut(&existing_id)
                .expect("serialized body-fetch owner remains present after admission");
            pending.task = merged;
            pending.request_hash = request_hash;
            return Ok(());
        }

        let mut staged_release = None;
        if !self.body_pipeline_owners.contains_key(&key)
            && let Some(staged) = self.ready_bodies.get(&key)
        {
            let staged_manifest = staged.manifest.clone();
            if manifest
                .as_ref()
                .is_none_or(|manifest| manifest == &staged_manifest)
            {
                let owner_plan = self.plan_body_pipeline_owner(tag, &staged_manifest)?;
                self.runtime
                    .enqueue_body_available(tag, staged_manifest)
                    .map_err(runtime_enqueue_error)?;
                self.commit_body_pipeline_owner(owner_plan);
                return Ok(());
            }

            // A Byzantine leader can advertise a structurally valid but
            // noncanonical manifest for the locked subject. Do not let that
            // untrusted proposal pin the trusted follower cache. Plan its
            // retirement, but keep the exact bytes owned until a replacement
            // acquisition or completion has actually been admitted.
            staged_release = Some(self.plan_ready_body_release(key)?);
        }

        if !self.body_pipeline_owners.contains_key(&key)
            && let Some((retained_subject, retained_bytes)) = self.retained_locked_body.as_ref()
            && *retained_subject == subject
        {
            let retained = ReadyBody::derive(&self.context, round, subject, retained_bytes.clone())
                .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
            if manifest
                .as_ref()
                .is_none_or(|manifest| manifest == &retained.manifest)
            {
                let retained_manifest = retained.manifest.clone();
                let ready_plan =
                    self.plan_ready_body_install(key, retained, staged_release.clone())?;
                let owner_plan = self.plan_body_pipeline_owner(tag, &retained_manifest)?;
                self.runtime
                    .enqueue_body_available(tag, retained_manifest)
                    .map_err(runtime_enqueue_error)?;
                self.commit_body_pipeline_owner(owner_plan);
                self.commit_ready_body_install(ready_plan);
                return Ok(());
            }
        }

        if self.body_pipeline_owners.contains_key(&key) {
            debug_assert!(staged_release.is_none());
            let owner_plan =
                self.plan_body_pipeline_owner_hash(tag, key, manifest.as_ref().map(HashOf::new))?;
            let retained_hash = self.retained_body_manifest_hash(key)?.ok_or_else(|| {
                EffectExecutorError::Contract(
                    "exact body pipeline owner has no pending or retained stage".to_owned(),
                )
            })?;
            if owner_plan
                .owner
                .manifest_hash
                .is_some_and(|owner_hash| owner_hash != retained_hash)
            {
                return Err(EffectExecutorError::Contract(
                    "exact body pipeline ownership differs from its retained stage".to_owned(),
                ));
            }
            // The exact reducer incarnation already owns acquisition or a
            // later body stage. Its successful transition has queued the only
            // completion allowed to advance that pipeline.
            self.commit_body_pipeline_owner(owner_plan);
            return Ok(());
        }

        let retained_hash =
            self.retained_body_manifest_hash_after_ready_release(key, staged_release.as_ref())?;

        // A certified view transition may fail to cancel an already-active
        // store. Its immutable bytes remain authoritative, but its old-view
        // reducer consumer is detached. Re-enter through BodyAvailable so the
        // current reducer incarnation still observes the ordinary
        // FetchBody -> StoreBody FIFO before attaching its own consumer.
        if let Some(stored_manifest) = self
            .pending_stores
            .values()
            .find(|pending| {
                pending.consumer.is_none()
                    && pending.task.manifest.round == round
                    && pending.task.manifest.subject == subject
            })
            .map(|pending| pending.task.manifest.clone())
        {
            if manifest
                .as_ref()
                .is_some_and(|manifest| manifest != &stored_manifest)
            {
                return Err(EffectExecutorError::Contract(
                    "current FetchBody manifest differs from retained body-store work".to_owned(),
                ));
            }
            if retained_hash != Some(HashOf::new(&stored_manifest)) {
                return Err(EffectExecutorError::Contract(
                    "retained body-store work differs from its exact pipeline stages".to_owned(),
                ));
            }
            let owner_plan = self.plan_body_pipeline_owner(tag, &stored_manifest)?;
            self.runtime
                .enqueue_body_available(tag, stored_manifest)
                .map_err(runtime_enqueue_error)?;
            if let Some(release) = staged_release {
                self.commit_ready_body_release(release);
            }
            self.commit_body_pipeline_owner(owner_plan);
            return Ok(());
        }

        if retained_hash.is_some() && !self.recovered_bodies.contains_key(&key) {
            return Err(EffectExecutorError::Contract(
                "retained exact-body stage has no reducer pipeline owner".to_owned(),
            ));
        }

        if let Some((recovered_manifest, receipt)) = self.recovered_bodies.get(&key).cloned() {
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
            if self
                .durable_bodies
                .get(&key)
                .is_some_and(|existing| existing != &receipt)
            {
                return Err(EffectExecutorError::BodyStore(
                    "reopened durable body conflicts with retained receipt ownership".to_owned(),
                ));
            }
            let owner_plan = self.plan_body_pipeline_owner(tag, &recovered_manifest)?;
            self.runtime
                .enqueue_body_available(tag, recovered_manifest)
                .map_err(runtime_enqueue_error)?;
            if let Some(release) = staged_release {
                self.commit_ready_body_release(release);
            }
            self.commit_body_pipeline_owner(owner_plan);
            self.durable_bodies.insert(key, receipt);
            return Ok(());
        }

        if self.pending_work() > self.config.max_pending_work {
            return Err(EffectExecutorError::Contract(
                "pending effect work exceeded its configured capacity".to_owned(),
            ));
        }
        if self.pending_work() == self.config.max_pending_work {
            // FetchBody is a reconstruction request, not successful body
            // delivery. The reducer deliberately remains in Missing and its
            // periodic proposal/QC/lock/Decision retransmission re-emits this
            // exact request after capacity changes. Do not retain it in the
            // causal effect suffix: an unresponsive Byzantine proposal source
            // must never sit ahead of the timeout/signing path after GST.
            iroha_logger::debug!(
                height = round.height,
                view = round.view,
                certified = certificate.is_some(),
                "deferred reconstructible Sumeragi v2 body fetch at pending-work capacity"
            );
            return Ok(());
        }
        let work = self.plan_work_id()?;
        let request_plan = if let Some(certificate) = certificate {
            Some(self.plan_certified_fetch_request(
                work.id,
                round,
                subject,
                certificate,
                services,
            )?)
        } else {
            None
        };
        let certified_request = request_plan.as_ref().map(|plan| plan.request.clone());
        let request_hash = request_plan.as_ref().map(|plan| plan.request_hash);
        let task = BodyFetchTask {
            id: work.id,
            tag,
            round,
            subject,
            manifest,
            sources,
            certified_request,
        };
        let owner_plan =
            self.plan_body_pipeline_owner_hash(tag, key, task.manifest.as_ref().map(HashOf::new))?;
        services
            .enqueue_body_fetch(task.clone())
            .map_err(service_error)?;
        if let Some(release) = staged_release {
            self.commit_ready_body_release(release);
        }
        self.commit_body_pipeline_owner(owner_plan);
        self.commit_work_id(work);
        if let Some(plan) = request_plan {
            self.commit_certified_fetch_request(plan);
        }
        self.pending_fetches
            .insert(work.id, PendingFetch { task, request_hash });
        Ok(())
    }

    fn retained_body_manifest_hash(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
    ) -> Result<Option<HashOf<wire::PayloadManifest>>, EffectExecutorError> {
        self.retained_body_manifest_hash_after_ready_release(key, None)
    }

    fn retained_body_manifest_hash_after_ready_release(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        ready_release: Option<&ReadyBodyReleasePlan>,
    ) -> Result<Option<HashOf<wire::PayloadManifest>>, EffectExecutorError> {
        if ready_release.is_some_and(|release| {
            release.key != key || self.ready_bodies.get(&key) != Some(&release.body)
        }) {
            return Err(EffectExecutorError::Contract(
                "planned ready-body retirement no longer matches exact ownership".to_owned(),
            ));
        }
        let mut hashes = Vec::new();
        if ready_release.is_none()
            && let Some(ready) = self.ready_bodies.get(&key)
        {
            hashes.push(HashOf::new(&ready.manifest));
        }
        hashes.extend(
            self.pending_stores
                .values()
                .filter(|pending| {
                    pending.task.manifest.round == key.0 && pending.task.manifest.subject == key.1
                })
                .map(|pending| HashOf::new(&pending.task.manifest)),
        );
        if let Some(receipt) = self.durable_bodies.get(&key) {
            hashes.push(receipt.manifest_hash());
        }
        hashes.extend(
            self.pending_validations
                .values()
                .filter(|pending| pending.task.round() == key.0 && pending.task.subject() == key.1)
                .map(|pending| pending.task.durable_receipt().manifest_hash()),
        );
        if let Some(receipt) = self.validated_bodies.get(&key) {
            hashes.push(receipt.durable().manifest_hash());
        }
        hashes.extend(
            self.pending_applications
                .values()
                .filter(|pending| {
                    pending.task.certificate.round == key.0 && pending.task.subject == key.1
                })
                .map(|pending| pending.task.validated_receipt.durable().manifest_hash()),
        );

        let Some(expected) = hashes.first().copied() else {
            return Ok(None);
        };
        if hashes.iter().any(|hash| *hash != expected) {
            return Err(EffectExecutorError::Contract(
                "retained exact-body pipeline has conflicting manifest ownership".to_owned(),
            ));
        }
        Ok(Some(expected))
    }

    fn plan_certified_fetch_request<S: V2EffectServices>(
        &self,
        id: EffectWorkId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
        services: &mut S,
    ) -> Result<CertifiedFetchRequestPlan, EffectExecutorError> {
        if self.outstanding_requests.len() >= self.config.max_certified_requests {
            return Err(EffectExecutorError::CertifiedRequestCapacity {
                capacity: self.config.max_certified_requests,
            });
        }
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
        if self.certified_work.contains_key(&request_hash) {
            return Err(EffectExecutorError::Contract(
                "certified body request hash already has a work owner".to_owned(),
            ));
        }
        let registration = self
            .outstanding_requests
            .plan_registration(authenticated)
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        Ok(CertifiedFetchRequestPlan {
            work_id: id,
            request,
            request_hash,
            registration,
        })
    }

    fn commit_certified_fetch_request(&mut self, plan: CertifiedFetchRequestPlan) {
        debug_assert!(!self.certified_work.contains_key(&plan.request_hash));
        self.outstanding_requests
            .commit_registration(plan.registration);
        self.certified_work.insert(plan.request_hash, plan.work_id);
    }

    fn plan_certified_fetch_retirement(
        &self,
        work_id: EffectWorkId,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Result<CertifiedFetchRetirementPlan, EffectExecutorError> {
        if self.certified_work.get(&request_hash) != Some(&work_id) {
            return Err(EffectExecutorError::Contract(
                "certified body fetch has mismatched exact work ownership".to_owned(),
            ));
        }
        let retirement = self
            .outstanding_requests
            .plan_retirement(request_hash)
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        Ok(CertifiedFetchRetirementPlan {
            request_hash,
            retirement,
        })
    }

    fn commit_certified_fetch_retirement(&mut self, plan: CertifiedFetchRetirementPlan) {
        self.certified_work.remove(&plan.request_hash);
        self.outstanding_requests.commit_retirement(plan.retirement);
    }

    fn plan_pending_fetch_retirement(
        &self,
        pending: &PendingFetch,
    ) -> Result<PendingFetchRetirementPlan, EffectExecutorError> {
        let work_id = pending.task.id();
        if self.pending_fetches.get(&work_id) != Some(pending) {
            return Err(EffectExecutorError::Contract(
                "body-fetch retirement differs from its exact executor owner".to_owned(),
            ));
        }
        let certified = pending
            .request_hash
            .map(|request_hash| self.plan_certified_fetch_retirement(work_id, request_hash))
            .transpose()?;
        Ok(PendingFetchRetirementPlan {
            pending: pending.clone(),
            certified,
        })
    }

    fn commit_pending_fetch_retirement(&mut self, plan: PendingFetchRetirementPlan) {
        let work_id = plan.pending.task.id();
        let removed = self.pending_fetches.remove(&work_id);
        debug_assert_eq!(removed.as_ref(), Some(&plan.pending));
        if let Some(certified) = plan.certified {
            self.commit_certified_fetch_retirement(certified);
        }
    }

    fn preflight_certified_fetch_indexes(&self) -> Result<(), EffectExecutorError> {
        for pending in self.pending_fetches.values() {
            if let Some(request_hash) = pending.request_hash {
                self.plan_certified_fetch_retirement(pending.task.id(), request_hash)?;
            }
        }
        Ok(())
    }

    /// Prove the byte counters equal the complete serialized owner sets.
    ///
    /// EnterView may retire one subset during lock reconciliation and a
    /// disjoint residual subset during stale-view cleanup. Direct reproposal
    /// reconciliation reaches the same cleanup outside EnterView. Exact global
    /// accounting at both entrypoints guarantees each later subtraction is a
    /// partition of an already-accounted owner set; a low counter cannot pass
    /// one subset and fail only after that subset has been committed.
    fn preflight_exact_body_byte_accounting(&self) -> Result<(), EffectExecutorError> {
        let retained_bytes = self
            .retained_locked_body
            .as_ref()
            .map(|(_, bytes)| {
                u64::try_from(bytes.len()).map_err(|_| {
                    EffectExecutorError::Contract(
                        "retained locked-body byte count is not representable".to_owned(),
                    )
                })
            })
            .transpose()?
            .unwrap_or(0);
        let ready_bytes = self.ready_bodies.values().try_fold(0u64, |total, body| {
            let bytes = u64::try_from(body.bytes.len()).map_err(|_| {
                EffectExecutorError::Contract(
                    "ready-body byte count is not representable".to_owned(),
                )
            })?;
            total.checked_add(bytes).ok_or_else(|| {
                EffectExecutorError::Contract("ready-body byte count overflowed".to_owned())
            })
        })?;
        let store_bytes = self
            .pending_stores
            .values()
            .try_fold(0u64, |total, pending| {
                let bytes = u64::try_from(pending.task.canonical_wire.len()).map_err(|_| {
                    EffectExecutorError::Contract(
                        "pending-store byte count is not representable".to_owned(),
                    )
                })?;
                total.checked_add(bytes).ok_or_else(|| {
                    EffectExecutorError::Contract("pending-store byte count overflowed".to_owned())
                })
            })?;
        let accounting = plan_exact_body_retirement_accounting(
            self.ready_body_bytes,
            retained_bytes,
            ready_bytes,
            self.pending_store_bytes,
            store_bytes,
        )
        .ok_or_else(|| {
            EffectExecutorError::Contract(
                "body byte accounting is lower than its serialized owners".to_owned(),
            )
        })?;
        if accounting.ready_after != 0 || accounting.store_after != 0 {
            return Err(EffectExecutorError::Contract(
                "body byte accounting exceeds its serialized owners".to_owned(),
            ));
        }
        Ok(())
    }

    fn plan_certified_view_body_cleanup(
        &self,
        tag: EventTag,
        protected_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    ) -> Result<CertifiedViewBodyCleanupPlan, EffectExecutorError> {
        let stale_stores = self
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
        let retired_store_bytes = stale_stores.iter().try_fold(0u64, |total, id| {
            let pending = self.pending_stores.get(id).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "stale body-store cleanup lost its executor owner".to_owned(),
                )
            })?;
            let key = (pending.task.manifest.round, pending.task.manifest.subject);
            if Some(key) == protected_body {
                return Ok(total);
            }
            let bytes = u64::try_from(pending.task.canonical_wire.len()).map_err(|_| {
                EffectExecutorError::Contract(
                    "pending-store byte count is not representable".to_owned(),
                )
            })?;
            total.checked_add(bytes).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "stale pending-store byte count overflowed".to_owned(),
                )
            })
        })?;

        let stale_ready = self
            .ready_bodies
            .keys()
            .filter(|(round, _)| round.view < tag.view())
            .copied()
            .collect::<Vec<_>>();
        let mut retired_ready_bytes = 0u64;
        let mut protected_ready_rebinds = Vec::new();
        for key in &stale_ready {
            let ready = self.ready_bodies.get(key).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "stale ready-body cleanup lost its executor owner".to_owned(),
                )
            })?;
            if Some(*key) == protected_body {
                if let Some(owner) = self.body_pipeline_owners.get(key).copied() {
                    protected_ready_rebinds.push(CertifiedViewReadyRebindPlan {
                        key: *key,
                        previous_tag: owner.tag,
                        manifest: ready.manifest.clone(),
                        owner: self.plan_body_pipeline_owner_rebind(
                            *key,
                            owner.tag,
                            tag,
                            Some(HashOf::new(&ready.manifest)),
                        )?,
                    });
                }
                continue;
            }
            if let Some(owner) = self.body_pipeline_owners.get(key)
                && owner.manifest_hash != Some(HashOf::new(&ready.manifest))
            {
                return Err(EffectExecutorError::Contract(
                    "stale ready body differs from its exact pipeline ownership".to_owned(),
                ));
            }
            let bytes = u64::try_from(ready.bytes.len()).map_err(|_| {
                EffectExecutorError::Contract(
                    "ready-body byte count is not representable".to_owned(),
                )
            })?;
            retired_ready_bytes = retired_ready_bytes.checked_add(bytes).ok_or_else(|| {
                EffectExecutorError::Contract("stale ready-body byte count overflowed".to_owned())
            })?;
        }
        let accounting = plan_exact_body_retirement_accounting(
            self.ready_body_bytes,
            0,
            retired_ready_bytes,
            self.pending_store_bytes,
            retired_store_bytes,
        )
        .ok_or_else(|| {
            EffectExecutorError::Contract(
                "certified-view body cleanup byte accounting underflow or leakage".to_owned(),
            )
        })?;

        Ok(CertifiedViewBodyCleanupPlan {
            stale_stores,
            stale_ready,
            protected_ready_rebinds,
            accounting,
        })
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
            let owner_matches =
                self.exact_body_pipeline_stage_owned(tag, key, receipt.manifest_hash());
            let recovered_matches = self.recovered_bodies.get(&key).is_none_or(
                |(recovered_manifest, recovered_receipt)| {
                    recovered_receipt == &receipt
                        && HashOf::new(recovered_manifest) == receipt.manifest_hash()
                },
            );
            if receipt.context_id() != self.context.id()
                || receipt.round() != round
                || receipt.subject() != subject
                || !owner_matches
                || !recovered_matches
            {
                return Err(EffectExecutorError::BodyStore(
                    "durable StoreBody fast path conflicts with exact pipeline ownership"
                        .to_owned(),
                ));
            }
            let ready_release = if let Some(ready) = self.ready_bodies.get(&key) {
                if HashOf::new(&ready.manifest) != receipt.manifest_hash() {
                    return Err(EffectExecutorError::BodyStore(
                        "durable StoreBody fast path conflicts with retained ready body".to_owned(),
                    ));
                }
                Some(self.plan_ready_body_release(key)?)
            } else {
                None
            };
            self.runtime
                .enqueue_body_stored(tag, round, subject, receipt)
                .map_err(runtime_enqueue_error)?;
            if let Some(release) = ready_release {
                self.commit_ready_body_release(release);
            }
            return Ok(());
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
        let release = self.plan_ready_body_release(key)?;
        self.begin_store_with_release(
            tag,
            release.body.manifest.clone(),
            Arc::clone(&release.body.bytes),
            StorePurpose::Reducer,
            Some(release),
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
        let receipt = self.durable_bodies.get(&key).cloned().ok_or_else(|| {
            EffectExecutorError::Contract(
                "ValidateBody has no matching durable body receipt".to_owned(),
            )
        })?;
        self.begin_validation(
            round,
            subject,
            receipt,
            ValidationConsumer::Reducer { tag },
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
        self.begin_store_with_release(tag, manifest, canonical_wire, purpose, None, services)
    }

    #[allow(clippy::too_many_arguments)]
    fn begin_store_with_release<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        canonical_wire: Arc<[u8]>,
        purpose: StorePurpose,
        ready_release: Option<ReadyBodyReleasePlan>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.begin_store_with_plans(
            tag,
            manifest,
            canonical_wire,
            purpose,
            ready_release,
            None,
            services,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn begin_store_with_plans<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        canonical_wire: Arc<[u8]>,
        purpose: StorePurpose,
        ready_release: Option<ReadyBodyReleasePlan>,
        supplied_owner_plan: Option<BodyPipelineOwnerBindingPlan>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let key = (manifest.round, manifest.subject);
        let consumer = StoreConsumer::new(tag, purpose);
        if let Some(release) = &ready_release
            && (release.key != key
                || release.body.manifest != manifest
                || release.body.bytes.as_ref() != canonical_wire.as_ref())
        {
            return Err(EffectExecutorError::Contract(
                "ready-body release differs from its planned body-store admission".to_owned(),
            ));
        }
        let current_owner_plan = self.plan_body_pipeline_owner(tag, &manifest)?;
        let owner_plan = match (purpose, supplied_owner_plan) {
            (StorePurpose::LocalProposal, Some(supplied)) => {
                if supplied != current_owner_plan {
                    return Err(EffectExecutorError::Contract(
                        "preplanned local body-store owner differs from serialized ownership"
                            .to_owned(),
                    ));
                }
                supplied
            }
            (StorePurpose::LocalProposal, None) => {
                return Err(EffectExecutorError::Contract(
                    "local body store began without its preplanned pipeline owner".to_owned(),
                ));
            }
            (StorePurpose::Reducer, Some(_)) => {
                return Err(EffectExecutorError::Contract(
                    "reducer body store received a local owner-creation plan".to_owned(),
                ));
            }
            (StorePurpose::Reducer, None) => current_owner_plan,
        };
        if !owner_plan.already_owned && purpose != StorePurpose::LocalProposal {
            return Err(EffectExecutorError::Contract(
                "body store began without an exact reducer pipeline owner".to_owned(),
            ));
        }
        if let Some(receipt) = self.durable_bodies.get(&key).cloned() {
            if receipt.manifest_hash() != HashOf::new(&manifest)
                || self.recovered_bodies.get(&key).is_some_and(
                    |(recovered_manifest, recovered_receipt)| {
                        recovered_manifest != &manifest || recovered_receipt != &receipt
                    },
                )
            {
                return Err(EffectExecutorError::BodyStore(
                    "durable body catalogue differs from requested StoreBody manifest".to_owned(),
                ));
            }
            match purpose {
                StorePurpose::Reducer => self
                    .runtime
                    .enqueue_body_stored(tag, manifest.round, manifest.subject, receipt)
                    .map_err(runtime_enqueue_error)?,
                StorePurpose::LocalProposal => {
                    let validation = self.plan_begin_validation(
                        manifest.round,
                        manifest.subject,
                        receipt,
                        ValidationConsumer::LocalProposal {
                            tag,
                            manifest: manifest.clone(),
                        },
                        None,
                        Some(&owner_plan),
                        None,
                    )?;
                    self.admit_validation_start(&validation, services)?;
                    self.commit_body_pipeline_owner(owner_plan);
                    if let Some(release) = ready_release {
                        self.commit_ready_body_release(release);
                    }
                    self.commit_validation_start(validation);
                    return Ok(());
                }
            }
            self.commit_body_pipeline_owner(owner_plan);
            if let Some(release) = ready_release {
                self.commit_ready_body_release(release);
            }
            return Ok(());
        }
        if let Some(existing_id) = self.pending_stores.iter().find_map(|(id, pending)| {
            (pending.task.manifest.round == manifest.round
                && pending.task.manifest.subject == manifest.subject)
                .then_some(*id)
        }) {
            let existing = self
                .pending_stores
                .get(&existing_id)
                .expect("pending store ID came from this map");
            if existing.task.manifest != manifest
                || existing.task.canonical_wire.as_ref() != canonical_wire.as_ref()
            {
                return Err(EffectExecutorError::Contract(
                    "conflicting body-store retry for one round/subject".to_owned(),
                ));
            }
            if ready_release.is_some() {
                return Err(EffectExecutorError::Contract(
                    "ready-body transfer found an existing body-store owner".to_owned(),
                ));
            }
            return match &existing.consumer {
                Some(existing_consumer) if existing_consumer == &consumer => {
                    services
                        .enqueue_body_store(existing.task.clone())
                        .map_err(service_error)?;
                    self.commit_body_pipeline_owner(owner_plan);
                    Ok(())
                }
                Some(_) => Err(EffectExecutorError::Contract(
                    "conflicting body-store consumer for one round/subject".to_owned(),
                )),
                None => {
                    self.commit_body_pipeline_owner(owner_plan);
                    self.pending_stores
                        .get_mut(&existing_id)
                        .expect("detached pending store remains present")
                        .consumer = Some(consumer);
                    Ok(())
                }
            };
        }
        let body_len = u64::try_from(canonical_wire.len()).map_err(|_| {
            EffectExecutorError::Contract("body-store task length is not representable".to_owned())
        })?;
        let mut retained_union = self.retained_body_union()?;
        if let Some(release) = &ready_release {
            retained_union.remove_manifest(&release.body.manifest, release.body.bytes.as_ref())?;
        }
        self.insert_retained_union_manifest(
            &mut retained_union,
            &manifest,
            Arc::clone(&canonical_wire),
        )?;
        self.ensure_retained_body_union_bound(&retained_union)?;
        self.ensure_pending_slot()?;
        let work = self.plan_work_id()?;
        let task = BodyStoreTask {
            id: work.id,
            tag,
            manifest,
            canonical_wire,
        };
        let pending_store_bytes = self
            .pending_store_bytes
            .checked_add(body_len)
            .ok_or(EffectExecutorError::ReadyBodyCapacity)?;
        services
            .enqueue_body_store(task.clone())
            .map_err(service_error)?;
        self.commit_body_pipeline_owner(owner_plan);
        if let Some(release) = ready_release {
            self.commit_ready_body_release(release);
        }
        self.commit_work_id(work);
        self.pending_stores.insert(
            work.id,
            PendingStore {
                task: task.clone(),
                consumer: Some(consumer),
            },
        );
        self.pending_store_bytes = pending_store_bytes;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn begin_validation<S: V2EffectServices>(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        durable_receipt: DurableBodyReceipt,
        consumer: ValidationConsumer,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let plan = self.plan_begin_validation(
            round,
            subject,
            durable_receipt,
            consumer,
            None,
            None,
            None,
        )?;
        self.admit_validation_start(&plan, services)?;
        self.commit_validation_start(plan);
        Ok(())
    }

    fn plan_begin_validation(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        durable_receipt: DurableBodyReceipt,
        consumer: ValidationConsumer,
        planned_durable_receipt: Option<&DurableBodyReceipt>,
        planned_owner: Option<&BodyPipelineOwnerBindingPlan>,
        replacing_store: Option<EffectWorkId>,
    ) -> Result<ValidationStartPlan, EffectExecutorError> {
        let key = (round, subject);
        if durable_receipt.context_id() != self.context.id()
            || durable_receipt.round() != round
            || durable_receipt.subject() != subject
        {
            return Err(EffectExecutorError::BodyStore(
                "validation task receipt differs from its round/subject".to_owned(),
            ));
        }
        let durable_catalog_matches = match self.durable_bodies.get(&key) {
            Some(existing) => existing == &durable_receipt,
            None => planned_durable_receipt == Some(&durable_receipt),
        };
        if !durable_catalog_matches {
            return Err(EffectExecutorError::BodyStore(
                "validation task receipt differs from the durable body catalog".to_owned(),
            ));
        }
        let planned_owner_matches = planned_owner.is_some_and(|plan| {
            plan.key == key
                && plan.owner.tag == consumer.tag()
                && plan.owner.manifest_hash == Some(durable_receipt.manifest_hash())
        });
        if !self.validation_consumer_matches_owner(&consumer, &durable_receipt)
            && !planned_owner_matches
        {
            return Err(EffectExecutorError::Contract(
                "validation consumer does not own the exact durable body pipeline".to_owned(),
            ));
        }
        if self.validated_bodies.contains_key(&key) && self.rejected_bodies.contains_key(&key) {
            return Err(EffectExecutorError::Contract(
                "one exact durable body has both validated and rejected outcomes".to_owned(),
            ));
        }
        if let Some(validated) = self.validated_bodies.get(&key).cloned() {
            if validated.durable() != &durable_receipt {
                return Err(EffectExecutorError::BodyStore(
                    "cached validation covers a different durable body".to_owned(),
                ));
            }
            let admission = match consumer {
                ValidationConsumer::Reducer { tag } => ValidationAdmissionPlan::RuntimeSucceeded {
                    tag,
                    round,
                    subject,
                    receipt: validated,
                },
                ValidationConsumer::LocalProposal { tag, manifest } => {
                    ValidationAdmissionPlan::RuntimeLocalProposal {
                        tag,
                        manifest,
                        durable_receipt,
                        validated_receipt: validated,
                    }
                }
            };
            return Ok(ValidationStartPlan {
                admission,
                commit: ValidationCommitPlan::None,
            });
        }
        if let Some(rejected) = self.rejected_bodies.get(&key) {
            if rejected != &durable_receipt {
                return Err(EffectExecutorError::BodyStore(
                    "cached rejection covers a different durable body".to_owned(),
                ));
            }
            let admission = match consumer {
                ValidationConsumer::Reducer { tag } => ValidationAdmissionPlan::RuntimeFailed {
                    tag,
                    round,
                    subject,
                },
                ValidationConsumer::LocalProposal { .. } => ValidationAdmissionPlan::None,
            };
            return Ok(ValidationStartPlan {
                admission,
                commit: ValidationCommitPlan::None,
            });
        }
        if let Some((existing_id, existing)) = self
            .pending_validations
            .iter()
            .find(|(_, pending)| pending.task.round() == round && pending.task.subject() == subject)
            .map(|(id, pending)| (*id, pending.clone()))
        {
            if existing.task.durable_receipt != durable_receipt {
                return Err(EffectExecutorError::Contract(
                    "conflicting validation retry for one durable body".to_owned(),
                ));
            }
            let commit = match &existing.consumer {
                Some(attached) if attached == &consumer => ValidationCommitPlan::None,
                Some(_) => {
                    return Err(EffectExecutorError::Contract(
                        "one immutable validation task has conflicting consumers".to_owned(),
                    ));
                }
                None if matches!(&consumer, ValidationConsumer::Reducer { .. }) => {
                    ValidationCommitPlan::Attach {
                        id: existing_id,
                        consumer,
                    }
                }
                None => {
                    return Err(EffectExecutorError::Contract(
                        "detached validation may only be adopted by the reducer pipeline"
                            .to_owned(),
                    ));
                }
            };
            let admission = if self.deferred_merge_work.contains_key(&existing.task.id()) {
                ValidationAdmissionPlan::None
            } else {
                ValidationAdmissionPlan::Service(existing.task)
            };
            return Ok(ValidationStartPlan { admission, commit });
        }
        if let Some(store_id) = replacing_store {
            let store = self.pending_stores.get(&store_id).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "validation capacity transfer lost its pending store owner".to_owned(),
                )
            })?;
            let same_local_consumer = matches!(
                (&store.consumer, &consumer),
                (
                    Some(StoreConsumer::LocalProposal { tag: store_tag }),
                    ValidationConsumer::LocalProposal {
                        tag: validation_tag,
                        ..
                    }
                ) if store_tag == validation_tag
            );
            if store.task.manifest.round != round
                || store.task.manifest.subject != subject
                || !same_local_consumer
            {
                return Err(EffectExecutorError::Contract(
                    "validation capacity transfer differs from its local pending store".to_owned(),
                ));
            }
            if self.pending_work() > self.config.max_pending_work {
                return Err(EffectExecutorError::PendingWorkCapacity {
                    capacity: self.config.max_pending_work,
                });
            }
        } else {
            self.ensure_pending_slot()?;
        }
        let work = self.plan_work_id()?;
        let task = BodyValidationTask {
            id: work.id,
            durable_receipt,
        };
        Ok(ValidationStartPlan {
            admission: ValidationAdmissionPlan::Service(task.clone()),
            commit: ValidationCommitPlan::Insert {
                work,
                pending: PendingValidation {
                    task,
                    consumer: Some(consumer),
                },
            },
        })
    }

    fn admit_validation_start<S: V2EffectServices>(
        &mut self,
        plan: &ValidationStartPlan,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        match &plan.admission {
            ValidationAdmissionPlan::None => Ok(()),
            ValidationAdmissionPlan::Service(task) => services
                .enqueue_body_validation(task.clone())
                .map_err(service_error),
            ValidationAdmissionPlan::RuntimeSucceeded {
                tag,
                round,
                subject,
                receipt,
            } => self
                .runtime
                .enqueue_validation_succeeded(*tag, *round, *subject, receipt.clone())
                .map_err(runtime_enqueue_error),
            ValidationAdmissionPlan::RuntimeFailed {
                tag,
                round,
                subject,
            } => self
                .runtime
                .enqueue_validation_failed(*tag, *round, *subject)
                .map_err(runtime_enqueue_error),
            ValidationAdmissionPlan::RuntimeLocalProposal {
                tag,
                manifest,
                durable_receipt,
                validated_receipt,
            } => self
                .runtime
                .enqueue_local_proposal(
                    *tag,
                    manifest.clone(),
                    durable_receipt.clone(),
                    validated_receipt.clone(),
                )
                .map_err(runtime_enqueue_error),
        }
    }

    fn commit_validation_start(&mut self, plan: ValidationStartPlan) {
        match plan.commit {
            ValidationCommitPlan::None => {}
            ValidationCommitPlan::Attach { id, consumer } => {
                self.pending_validations
                    .get_mut(&id)
                    .expect("preflighted pending validation remains serialized")
                    .consumer = Some(consumer);
            }
            ValidationCommitPlan::Insert { work, pending } => {
                self.commit_work_id(work);
                self.pending_validations.insert(work.id, pending);
            }
        }
    }

    fn validation_consumer_matches_owner(
        &self,
        consumer: &ValidationConsumer,
        durable_receipt: &DurableBodyReceipt,
    ) -> bool {
        let key = (durable_receipt.round(), durable_receipt.subject());
        if !self.exact_body_pipeline_stage_owned(
            consumer.tag(),
            key,
            durable_receipt.manifest_hash(),
        ) {
            return false;
        }
        match consumer {
            ValidationConsumer::Reducer { .. } => true,
            ValidationConsumer::LocalProposal { manifest, .. } => {
                manifest.round == key.0
                    && manifest.subject == key.1
                    && HashOf::new(manifest) == durable_receipt.manifest_hash()
            }
        }
    }

    fn preflight_pending_validation_consumer(
        &self,
        work_id: EffectWorkId,
        pending: &PendingValidation,
    ) -> Result<(), EffectExecutorError> {
        let durable = pending.task.durable_receipt();
        let key = (pending.task.round(), pending.task.subject());
        if pending.task.id() != work_id
            || durable.context_id() != self.context.id()
            || durable.round() != key.0
            || durable.subject() != key.1
        {
            return Err(EffectExecutorError::Contract(
                "pending validation task differs from its serialized work owner".to_owned(),
            ));
        }
        if self.durable_bodies.get(&key) != Some(durable)
            || !self
                .recovered_bodies
                .get(&key)
                .is_some_and(|(manifest, recovered)| {
                    recovered == durable && HashOf::new(manifest) == durable.manifest_hash()
                })
        {
            return Err(EffectExecutorError::BodyStore(
                "pending validation differs from its recovered durable body".to_owned(),
            ));
        }
        if let Some(consumer) = &pending.consumer
            && !self.validation_consumer_matches_owner(consumer, durable)
        {
            return Err(EffectExecutorError::Contract(
                "validation completion consumer differs from its immutable pipeline owner"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn preflight_pending_application_owner(
        &self,
        work_id: EffectWorkId,
        pending: &PendingApply,
    ) -> Result<(), EffectExecutorError> {
        let task = &pending.task;
        let certificate = task.certificate();
        let validated = task.validated_receipt();
        let durable = validated.durable();
        let key = (certificate.round, task.subject());
        if task.id() != work_id
            || task.tag().height() != self.context.height
            || certificate.phase != wire::GlobalPhase::Commit
            || certificate.round.context_id != self.context.id()
            || certificate.round.height != self.context.height
            || certificate.subject != task.subject()
            || durable.context_id() != self.context.id()
            || durable.round() != certificate.round
            || durable.subject() != task.subject()
            || validated.execution_commitment() != certificate.execution_commitment
            || self.protected_decision != Some(key)
            || !self.decision_body_drained
            || self.durable_bodies.get(&key) != Some(durable)
            || self.validated_bodies.get(&key) != Some(validated)
        {
            return Err(EffectExecutorError::Contract(
                "deferred application differs from its exact decided-body owner".to_owned(),
            ));
        }
        Ok(())
    }

    fn preflight_deferred_work_owner(
        &self,
        work_id: EffectWorkId,
    ) -> Result<(), EffectExecutorError> {
        match (
            self.pending_validations.get(&work_id),
            self.pending_applications.get(&work_id),
        ) {
            (Some(pending), None) => self.preflight_pending_validation_consumer(work_id, pending),
            (None, Some(pending)) => self.preflight_pending_application_owner(work_id, pending),
            (Some(_), Some(_)) => Err(EffectExecutorError::Contract(
                "deferred merge sidecar has conflicting validation and application owners"
                    .to_owned(),
            )),
            (None, None) => Err(EffectExecutorError::Contract(
                "deferred merge sidecar has no pending validation or application task".to_owned(),
            )),
        }
    }

    fn preflight_exact_durable_body(
        &self,
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<(), EffectExecutorError> {
        let key = (durable_receipt.round(), durable_receipt.subject());
        if durable_receipt.context_id() != self.context.id() {
            return Err(EffectExecutorError::BodyStore(
                "validation receipt belongs to a different height context".to_owned(),
            ));
        }
        if let Some((manifest, recovered)) = self.recovered_bodies.get(&key)
            && (recovered != durable_receipt
                || HashOf::new(manifest) != durable_receipt.manifest_hash())
        {
            return Err(EffectExecutorError::BodyStore(
                "validation receipt conflicts with the recovered durable body".to_owned(),
            ));
        }
        if let Some(existing) = self.durable_bodies.get(&key)
            && existing != durable_receipt
        {
            return Err(EffectExecutorError::BodyStore(
                "validation receipt conflicts with the durable body catalog".to_owned(),
            ));
        }
        Ok(())
    }

    fn preflight_validated_body(
        &self,
        validated: &ValidatedBodyReceipt,
    ) -> Result<(), EffectExecutorError> {
        let durable = validated.durable();
        let key = (durable.round(), durable.subject());
        self.preflight_exact_durable_body(durable)?;
        if self.rejected_bodies.contains_key(&key) {
            return Err(EffectExecutorError::Contract(
                "one exact durable body produced both validated and rejected outcomes".to_owned(),
            ));
        }
        if let Some(existing) = self.validated_bodies.get(&key)
            && existing != validated
        {
            return Err(EffectExecutorError::BodyStore(
                "one exact durable body produced conflicting validation receipts".to_owned(),
            ));
        }
        Ok(())
    }

    fn record_validated_body(
        &mut self,
        validated: ValidatedBodyReceipt,
    ) -> Result<(), EffectExecutorError> {
        let durable = validated.durable().clone();
        let key = (durable.round(), durable.subject());
        self.preflight_validated_body(&validated)?;
        self.durable_bodies.entry(key).or_insert(durable);
        self.validated_bodies.entry(key).or_insert(validated);
        Ok(())
    }

    fn preflight_rejected_body(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<(), EffectExecutorError> {
        if key != (durable_receipt.round(), durable_receipt.subject()) {
            return Err(EffectExecutorError::BodyStore(
                "validation rejection differs from its durable body".to_owned(),
            ));
        }
        self.preflight_exact_durable_body(durable_receipt)?;
        if self.validated_bodies.contains_key(&key) {
            return Err(EffectExecutorError::Contract(
                "one exact durable body produced both validated and rejected outcomes".to_owned(),
            ));
        }
        if let Some(existing) = self.rejected_bodies.get(&key)
            && existing != durable_receipt
        {
            return Err(EffectExecutorError::BodyStore(
                "one body key produced conflicting durable rejection receipts".to_owned(),
            ));
        }
        Ok(())
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
        self.reconcile_decision_work(
            (key.0, key.1, certificate.execution_commitment),
            true,
            services,
        )?;
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

    fn reconcile_runtime_decision<S: V2EffectServices>(
        &mut self,
        services: &mut S,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        EffectExecutorError,
    > {
        let decision = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)?;
        if let Some(decision) = decision {
            self.reconcile_decision_work(decision, false, services)?;
        }
        Ok(decision)
    }

    /// Reconcile volatile ownership immediately after the reducer installs a
    /// durable Decision, before dispatching the Decision's body-recovery
    /// effect. The exact decided pipeline remains live until Apply begins;
    /// every competing owner is retired so it cannot consume the capacity
    /// needed to recover, validate, and apply the decision.
    fn reconcile_decision_work<S: V2EffectServices>(
        &mut self,
        durable_decision: (
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        ),
        drain_decision_body: bool,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let (decision_round, decision_subject, decision_commitment) = durable_decision;
        let decision = (decision_round, decision_subject);
        if decision_round.context_id != self.context.id()
            || decision_round.height != self.context.height
        {
            return Err(EffectExecutorError::Contract(
                "durable Decision is outside the frozen height context".to_owned(),
            ));
        }
        match self.protected_decision {
            Some(existing) if existing != decision => {
                return Err(EffectExecutorError::Contract(
                    "one height installed two different durable Decisions".to_owned(),
                ));
            }
            Some(_) if !drain_decision_body || self.decision_body_drained => return Ok(()),
            _ => {}
        }
        if drain_decision_body && !self.pending_applications.is_empty() {
            return Err(EffectExecutorError::Contract(
                "terminal body cleanup began after application ownership was installed".to_owned(),
            ));
        }
        let first_install = self.protected_decision.is_none();
        let retire_key = |key: (wire::ConsensusRound, wire::BlockSubject)| {
            drain_decision_body || key != decision
        };

        self.preflight_exact_body_byte_accounting()?;

        let exact_local_stores = self
            .pending_stores
            .iter()
            .filter_map(|(id, pending)| {
                ((pending.task.manifest.round, pending.task.manifest.subject) == decision
                    && matches!(
                        &pending.consumer,
                        Some(StoreConsumer::LocalProposal { .. }) | None
                    ))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        let exact_local_validations = self
            .pending_validations
            .iter()
            .filter_map(|(id, pending)| {
                ((pending.task.round(), pending.task.subject()) == decision
                    && matches!(
                        &pending.consumer,
                        Some(ValidationConsumer::LocalProposal { .. }) | None
                    ))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        if exact_local_stores
            .len()
            .saturating_add(exact_local_validations.len())
            > 1
        {
            return Err(EffectExecutorError::Contract(
                "decided body has multiple local-proposal pipeline owners".to_owned(),
            ));
        }
        let fetches = self
            .pending_fetches
            .values()
            .filter(|pending| retire_key((pending.task.round, pending.task.subject)))
            .map(|pending| self.plan_pending_fetch_retirement(pending))
            .collect::<Result<Vec<_>, _>>()?;
        let mut proposal_retirement = DecisionProposalRetirement::default();
        if first_install {
            proposal_retirement = self
                .runtime
                .retire_proposal_work_after_decision(
                    decision_round,
                    decision_subject,
                    decision_commitment,
                )
                .map_err(EffectExecutorError::Runtime)?;
            services
                .retire_all_outbound_payloads()
                .map_err(service_error)?;
            services
                .retire_candidate_work_after_decision(decision_round, decision_subject)
                .map_err(service_error)?;
        }
        if usize::from(proposal_retirement.retained_local_proposal().is_some())
            .saturating_add(exact_local_stores.len())
            .saturating_add(exact_local_validations.len())
            > 1
        {
            return Err(EffectExecutorError::Contract(
                "decided body has duplicate local-proposal completion ownership".to_owned(),
            ));
        }
        if let Some(retained_tag) = proposal_retirement.retained_local_proposal() {
            let owner = self.body_pipeline_owners.get(&decision).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "retained decided local completion has no executor pipeline owner".to_owned(),
                )
            })?;
            let retained_hash = self.retained_body_manifest_hash(decision)?.ok_or_else(|| {
                EffectExecutorError::Contract(
                    "retained decided local completion has no durable body evidence".to_owned(),
                )
            })?;
            let validated = self.validated_bodies.get(&decision).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "retained decided local completion has no validation receipt".to_owned(),
                )
            })?;
            if owner.tag != retained_tag
                || owner.manifest_hash != Some(retained_hash)
                || validated.durable().manifest_hash() != retained_hash
                || validated.execution_commitment() != decision_commitment
            {
                return Err(EffectExecutorError::Contract(
                    "retained decided local completion differs from its Decision evidence"
                        .to_owned(),
                ));
            }
        }
        let detach_decision_pipeline = (proposal_retirement.retained_local_proposal().is_none()
            && proposal_retirement.retired_for_recovery() != 0)
            || !exact_local_stores.is_empty()
            || !exact_local_validations.is_empty();

        let pipeline_keys = self
            .body_pipeline_owners
            .iter()
            .filter_map(|(key, owner)| {
                (retire_key(*key) && !(detach_decision_pipeline && *key == decision))
                    .then_some((*key, *owner))
            })
            .collect::<Vec<_>>();
        for (key, owner) in &pipeline_keys {
            self.runtime
                .retire_body_pipeline_completions(owner.tag, key.0, key.1)
                .map_err(EffectExecutorError::Runtime)?;
        }

        let signatures = self.pending_signatures.keys().copied().collect::<Vec<_>>();
        for id in &signatures {
            services.cancel_consensus_sign(*id).map_err(service_error)?;
        }
        for plan in &fetches {
            services
                .cancel_body_fetch(&plan.pending.task)
                .map_err(service_error)?;
        }
        let stores = self
            .pending_stores
            .iter()
            .filter_map(|(id, pending)| {
                retire_key((pending.task.manifest.round, pending.task.manifest.subject))
                    .then_some((*id, pending.task.canonical_wire.len()))
            })
            .collect::<Vec<_>>();
        for (id, _) in &stores {
            services.cancel_body_store(*id).map_err(service_error)?;
        }
        let validations = self
            .pending_validations
            .iter()
            .filter_map(|(id, pending)| {
                retire_key((pending.task.round(), pending.task.subject())).then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in &validations {
            if !self.deferred_merge_work.contains_key(id) {
                services
                    .cancel_body_validation(*id)
                    .map_err(service_error)?;
            }
        }

        for id in &exact_local_stores {
            self.pending_stores
                .get_mut(id)
                .expect("preflighted decided local store remains serialized")
                .consumer = None;
        }
        for id in &exact_local_validations {
            self.pending_validations
                .get_mut(id)
                .expect("preflighted decided local validation remains serialized")
                .consumer = None;
        }
        if detach_decision_pipeline {
            self.body_pipeline_owners.remove(&decision);
        }
        for plan in fetches {
            self.commit_pending_fetch_retirement(plan);
        }
        let retired_store_bytes = stores.iter().try_fold(0u64, |total, (_, bytes)| {
            let bytes = u64::try_from(*bytes).map_err(|_| {
                EffectExecutorError::Contract(
                    "pending-store byte count is not representable".to_owned(),
                )
            })?;
            total.checked_add(bytes).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "terminal retired-store byte count overflowed".to_owned(),
                )
            })
        })?;
        for (id, _) in stores {
            self.pending_stores.remove(&id);
        }
        for id in validations {
            self.deferred_merge_work.remove(&id);
            self.pending_validations.remove(&id);
        }

        let retire_retained = self
            .retained_locked_body
            .as_ref()
            .is_some_and(|(subject, _)| drain_decision_body || *subject != decision_subject);
        let retired_retained_bytes = if retire_retained {
            self.retained_locked_body
                .as_ref()
                .map_or(Ok(0u64), |(_, bytes)| {
                    u64::try_from(bytes.len()).map_err(|_| {
                        EffectExecutorError::Contract(
                            "retained locked-body byte count is not representable".to_owned(),
                        )
                    })
                })?
        } else {
            0
        };
        let retired_ready_bytes =
            self.ready_bodies
                .iter()
                .try_fold(retired_retained_bytes, |total, (key, body)| {
                    if !retire_key(*key) {
                        return Ok(total);
                    }
                    let bytes = u64::try_from(body.bytes.len()).map_err(|_| {
                        EffectExecutorError::Contract(
                            "ready-body byte count is not representable".to_owned(),
                        )
                    })?;
                    total.checked_add(bytes).ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "terminal retired ready-body byte count overflowed".to_owned(),
                        )
                    })
                })?;

        self.pending_signatures.clear();
        self.body_pipeline_owners.retain(|key, _| !retire_key(*key));
        self.ready_bodies.retain(|key, _| !retire_key(*key));
        if retire_retained {
            self.retained_locked_body = None;
        }
        self.ready_body_bytes = self
            .ready_body_bytes
            .checked_sub(retired_ready_bytes)
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "terminal ready-body byte accounting underflow".to_owned(),
                )
            })?;
        self.pending_store_bytes = self
            .pending_store_bytes
            .checked_sub(retired_store_bytes)
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "terminal pending-store byte accounting underflow".to_owned(),
                )
            })?;
        if drain_decision_body
            && (!self.certified_work.is_empty() || !self.outstanding_requests.is_empty())
        {
            return Err(EffectExecutorError::Contract(
                "terminal cleanup left an unowned certified-body request".to_owned(),
            ));
        }
        self.protected_decision = Some(decision);
        self.decision_body_drained |= drain_decision_body;
        Ok(())
    }

    fn plan_fetch_completion<S: V2EffectServices>(
        &mut self,
        task: &BodyFetchTask,
        ready_body: ReadyBody,
        retention: Option<&RetainedLockedBodyPlan>,
        services: &mut S,
    ) -> Result<FetchCompletionPlan, EffectTransportError> {
        if let Some(reason) = &self.fatal_reason {
            return Err(EffectTransportError::FailClosed(reason.clone()));
        }
        let work_id = task.id();
        let pending = self
            .pending_fetches
            .get(&work_id)
            .cloned()
            .ok_or(EffectTransportError::UnknownWork(work_id))?;
        if pending.task != *task {
            return Err(EffectTransportError::BodyMismatch(
                "completion task differs from executor ownership",
            ));
        }
        let ReadyBody { manifest, bytes } = &ready_body;
        manifest
            .validate(&self.context)
            .map_err(|_| EffectTransportError::BodyMismatch("manifest is invalid for context"))?;
        if manifest.round != task.round || manifest.subject != task.subject {
            return Err(EffectTransportError::BodyMismatch(
                "manifest round or subject differs from executor ownership",
            ));
        }
        if task
            .manifest
            .as_ref()
            .is_some_and(|expected| expected != manifest)
        {
            return Err(EffectTransportError::BodyMismatch(
                "manifest differs from proposal authority",
            ));
        }
        if u64::try_from(bytes.len()).ok() != Some(manifest.payload_size_bytes) {
            return Err(EffectTransportError::BodyMismatch(
                "body length differs from manifest",
            ));
        }
        if Hash::new(bytes.as_ref()) != task.subject.payload_hash {
            return Err(EffectTransportError::BodyMismatch(
                "body hash differs from certified subject",
            ));
        }
        let tag = task.tag;
        let key = (task.round, task.subject);
        let owner_plan = match self.plan_body_pipeline_owner(tag, manifest) {
            Ok(plan) if plan.already_owned => plan,
            Ok(_) => {
                return Err(self.fail_closed_transport(
                    "completed body fetch had no exact pipeline owner",
                    services,
                ));
            }
            Err(error) => return Err(self.fail_closed_transport(error, services)),
        };

        // An unprotected old-view store may complete after this current fetch
        // started. Exact matching durable state wins idempotently: the current
        // reducer incarnation still receives BodyAvailable and its subsequent
        // StoreBody observes the already-minted receipt without duplicate I/O.
        // A ready body can overlap for the reverse ordering (fetch completion
        // immediately before the retired store completion), so collapse that
        // exact duplicate into the later durable stage and release its bytes.
        let certified_retirement = pending
            .request_hash
            .map(|request_hash| {
                self.plan_certified_fetch_retirement(work_id, request_hash)
                    .map_err(|error| self.fail_closed_transport(error, services))
            })
            .transpose()?;
        let manifest_hash = HashOf::new(manifest);
        let mut reuses_existing_stage = false;
        let mut ready_release = None;
        if let Some(receipt) = self.durable_bodies.get(&key).cloned() {
            let recovered_conflicts = self.recovered_bodies.get(&key).is_some_and(
                |(recovered_manifest, recovered_receipt)| {
                    recovered_manifest != manifest || recovered_receipt != &receipt
                },
            );
            let ready_conflicts = self.ready_bodies.get(&key).is_some_and(|ready| {
                &ready.manifest != manifest || ready.bytes.as_ref() != bytes.as_ref()
            });
            if receipt.context_id() != self.context.id()
                || receipt.round() != task.round
                || receipt.subject() != task.subject
                || receipt.manifest_hash() != manifest_hash
                || recovered_conflicts
                || ready_conflicts
            {
                return Err(self.fail_closed_transport(
                    "completed fetch conflicts with retained durable body identity",
                    services,
                ));
            }
            if self.ready_bodies.contains_key(&key) {
                ready_release = Some(
                    self.plan_ready_body_release(key)
                        .map_err(|error| self.fail_closed_transport(error, services))?,
                );
            }
            reuses_existing_stage = true;
        } else if let Some(ready) = self.ready_bodies.get(&key) {
            if &ready.manifest != manifest || ready.bytes.as_ref() != bytes.as_ref() {
                return Err(self.fail_closed_transport(
                    "completed fetch conflicts with retained ready body identity",
                    services,
                ));
            }
            reuses_existing_stage = true;
        }
        let runtime_manifest = manifest.clone();
        let ready = if reuses_existing_stage {
            let mut union = self
                .retained_body_union()
                .map_err(|error| self.fail_closed_transport(error, services))?;
            if let Some(retention) = retention
                && retention.install
            {
                union
                    .insert(retention.subject, Arc::clone(&retention.bytes))
                    .map_err(|error| self.fail_closed_transport(error, services))?;
            }
            if let Some(release) = &ready_release {
                union
                    .remove_manifest(&release.body.manifest, release.body.bytes.as_ref())
                    .map_err(|error| self.fail_closed_transport(error, services))?;
            }
            self.ensure_retained_body_union_bound(&union)
                .map_err(|error| match error {
                    EffectExecutorError::ReadyBodyCapacity => EffectTransportError::Backpressure,
                    error => self.fail_closed_transport(error, services),
                })?;
            FetchReadyCommitPlan::Reuse {
                release: ready_release,
            }
        } else {
            let install = self
                .plan_ready_body_install_with_retention(key, ready_body, None, retention)
                .map_err(|error| match error {
                    EffectExecutorError::ReadyBodyCapacity => EffectTransportError::Backpressure,
                    error => self.fail_closed_transport(error, services),
                })?;
            FetchReadyCommitPlan::Install(install)
        };
        let runtime_reservation = match self.runtime.reserve_body_available(tag, runtime_manifest) {
            Ok(reservation) => reservation,
            Err(EnqueueError::Full | EnqueueError::ReservedCapacity) => {
                return Err(EffectTransportError::Backpressure);
            }
            Err(
                error @ (EnqueueError::FailClosed | EnqueueError::DuplicateCompletionOwnership),
            ) => {
                return Err(self.fail_closed_transport(runtime_enqueue_error(error), services));
            }
        };
        Ok(FetchCompletionPlan {
            work_id,
            owner: owner_plan,
            ready,
            certified_retirement,
            runtime_reservation,
        })
    }

    fn abort_fetch_completion(&mut self, plan: FetchCompletionPlan) {
        self.runtime.abort_body_available(plan.runtime_reservation);
    }

    fn commit_fetch_completion(&mut self, plan: FetchCompletionPlan) {
        self.commit_body_pipeline_owner(plan.owner);
        match plan.ready {
            FetchReadyCommitPlan::Reuse { release } => {
                if let Some(release) = release {
                    self.commit_ready_body_release(release);
                }
            }
            FetchReadyCommitPlan::Install(install) => {
                self.commit_ready_body_install(install);
            }
        }
        self.pending_fetches.remove(&plan.work_id);
        if let Some(retirement) = plan.certified_retirement {
            self.commit_certified_fetch_retirement(retirement);
        }
        self.runtime.commit_body_available(plan.runtime_reservation);
    }

    fn finish_fetch<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        ready_body: ReadyBody,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectTransportError> {
        let task = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?
            .task
            .clone();
        let plan = self.plan_fetch_completion(&task, ready_body, None, services)?;
        if let Err(error) = services.complete_body_reconstruction_fetch(&task) {
            self.abort_fetch_completion(plan);
            return Err(self.fail_closed_transport(error, services));
        }
        self.commit_fetch_completion(plan);
        Ok(CompletionDisposition::Accepted)
    }

    fn install_view<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        certificate: wire::TimeoutCertificate,
        protected_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
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

        if let Some((round, _)) = protected_body
            && (round.context_id != self.context.id()
                || round.height != self.context.height
                || round.view >= tag.view())
        {
            return Err(EffectExecutorError::Contract(
                "EnterView protected body is outside the installed height/view".to_owned(),
            ));
        }
        if let Some(highest) = certificate.highest_prepare_qc() {
            let Some((protected_round, protected_subject)) = protected_body else {
                return Err(EffectExecutorError::Contract(
                    "EnterView omitted the body protected by its highest PrepareQC".to_owned(),
                ));
            };
            if protected_round.view < highest.round.view
                || (protected_round.view == highest.round.view
                    && protected_subject != highest.subject)
            {
                return Err(EffectExecutorError::Contract(
                    "EnterView protected body is lower than its highest PrepareQC".to_owned(),
                ));
            }
        }

        // A certified-request index mismatch must be diagnosed before lock
        // reconciliation, which can itself retire runtime/service ownership.
        // Protected fetch rebinding is also fully checked here so no fallible
        // executor lookup remains after its service callback acknowledges.
        self.preflight_certified_fetch_indexes()?;
        self.preflight_exact_body_byte_accounting()?;
        for pending in self
            .pending_fetches
            .values()
            .filter(|pending| pending.task.round.view < tag.view())
        {
            let key = (pending.task.round, pending.task.subject);
            if Some(key) != protected_body {
                continue;
            }
            pending.task.rebind_consumer(tag).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "protected body-fetch consumer did not advance to the certified view"
                        .to_owned(),
                )
            })?;
            self.plan_body_pipeline_owner_rebind(
                key,
                pending.task.tag,
                tag,
                pending.task.manifest.as_ref().map(HashOf::new),
            )?;
        }

        self.reconcile_protected_lock(tag, protected_body, services)?;
        let stale_body_cleanup = self.plan_certified_view_body_cleanup(tag, protected_body)?;

        let stale_fetches = self
            .pending_fetches
            .values()
            .filter(|pending| pending.task.round.view < tag.view())
            .map(|pending| {
                let key = (pending.task.round, pending.task.subject);
                if Some(key) == protected_body {
                    let rebound = pending.task.rebind_consumer(tag).ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "protected body-fetch consumer did not advance to the certified view"
                                .to_owned(),
                        )
                    })?;
                    let owner = self.plan_body_pipeline_owner_rebind(
                        key,
                        pending.task.tag,
                        tag,
                        pending.task.manifest.as_ref().map(HashOf::new),
                    )?;
                    Ok(StaleFetchTransitionPlan::Rebind {
                        pending: pending.clone(),
                        rebound,
                        owner,
                    })
                } else {
                    self.plan_pending_fetch_retirement(pending)
                        .map(StaleFetchTransitionPlan::Retire)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        for plan in &stale_fetches {
            match plan {
                StaleFetchTransitionPlan::Rebind {
                    pending, rebound, ..
                } => {
                    services
                        .rebind_body_fetch(&pending.task, rebound.clone())
                        .map_err(service_error)?;
                }
                StaleFetchTransitionPlan::Retire(retirement) => {
                    services
                        .cancel_body_fetch(&retirement.pending.task)
                        .map_err(service_error)?;
                }
            }
        }
        for plan in stale_fetches {
            match plan {
                StaleFetchTransitionPlan::Rebind {
                    pending,
                    rebound,
                    owner,
                } => {
                    let work_id = pending.task.id();
                    let current = self
                        .pending_fetches
                        .get_mut(&work_id)
                        .expect("preflighted protected fetch remains serialized");
                    debug_assert_eq!(current, &pending);
                    current.task = rebound;
                    self.body_pipeline_owners
                        .insert((pending.task.round, pending.task.subject), owner);
                }
                StaleFetchTransitionPlan::Retire(retirement) => {
                    self.commit_pending_fetch_retirement(retirement);
                }
            }
        }

        let stale = self
            .pending_signatures
            .iter()
            .filter_map(|(id, pending)| (pending.tag.view() < tag.view()).then_some(*id))
            .collect::<Vec<_>>();
        for id in stale {
            services.cancel_consensus_sign(id).map_err(service_error)?;
            self.pending_signatures.remove(&id);
        }

        // Validation work is immutable and receipt-bound, while its reducer
        // consumer belongs to one view incarnation. Preserve only the exact
        // effective durable-lock work item and detach that consumer; the new view must
        // adopt it through its ordinary FetchBody -> StoreBody -> ValidateBody
        // FIFO before any completion can affect reducer state.
        let prior_view = self
            .pending_validations
            .iter()
            .filter_map(|(id, pending)| (pending.task.round().view < tag.view()).then_some(*id))
            .collect::<Vec<_>>();
        for id in prior_view {
            let pending = self
                .pending_validations
                .get(&id)
                .expect("prior-view validation ID came from this map");
            let key = (pending.task.round(), pending.task.subject());
            if Some(key) == protected_body {
                let durable = pending.task.durable_receipt();
                let recovered_matches =
                    self.recovered_bodies
                        .get(&key)
                        .is_some_and(|(manifest, recovered)| {
                            recovered == durable && HashOf::new(manifest) == durable.manifest_hash()
                        });
                if self.durable_bodies.get(&key) != Some(durable) || !recovered_matches {
                    return Err(EffectExecutorError::BodyStore(
                        "protected validation is not backed by the exact recovered durable body"
                            .to_owned(),
                    ));
                }
                self.pending_validations
                    .get_mut(&id)
                    .expect("protected validation ID remains present")
                    .consumer = None;
            } else {
                // A merge-sidecar deferral is retained only after the original
                // validation completion was delivered and its I/O ownership
                // acknowledged. There is no queued or active worker command to
                // cancel in that state; attempting cancellation would
                // misclassify the deliberately detached task as lost work.
                if !self.deferred_merge_work.contains_key(&id) {
                    services.cancel_body_validation(id).map_err(service_error)?;
                }
                self.deferred_merge_work.remove(&id);
                self.pending_validations.remove(&id);
            }
        }

        // Byte residuals for the complete store/ready cleanup were checked
        // before any cancellation. A corrupt counter therefore cannot retire
        // worker or runtime ownership and only then discover the underflow.
        for id in &stale_body_cleanup.stale_stores {
            let key = self
                .pending_stores
                .get(id)
                .map(|pending| (pending.task.manifest.round, pending.task.manifest.subject))
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "stale body-store work lost its executor owner".to_owned(),
                    )
                })?;
            if Some(key) != protected_body {
                services.cancel_body_store(*id).map_err(service_error)?;
            }
        }
        for rebind in &stale_body_cleanup.protected_ready_rebinds {
            let rebound = self
                .runtime
                .rebind_body_available(rebind.previous_tag, tag, &rebind.manifest)
                .map_err(EffectExecutorError::Runtime)?;
            if !rebound {
                return Err(EffectExecutorError::Contract(
                    "protected ready body has no queued reducer completion to rebind".to_owned(),
                ));
            }
        }
        for key in &stale_body_cleanup.stale_ready {
            // A staged protected body can exist before FetchBody establishes
            // its reducer owner. Preserve it without inventing a completion;
            // the new view's ordinary FetchBody effect adopts the bytes and
            // enqueues BodyAvailable under its current tag.
            if Some(*key) == protected_body {
                continue;
            }
            if let Some(owner) = self.body_pipeline_owners.get(key).copied() {
                let ready = self.ready_bodies.get(key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "superseded ready body disappeared during completion retirement".to_owned(),
                    )
                })?;
                let retired = self
                    .runtime
                    .retire_body_available(owner.tag, &ready.manifest)
                    .map_err(EffectExecutorError::Runtime)?;
                if !retired {
                    return Err(EffectExecutorError::Contract(
                        "superseded ready body has no queued reducer completion to retire"
                            .to_owned(),
                    ));
                }
            }
        }
        // Every fallible store/runtime callback has now acknowledged. Commit
        // the preflighted ownership removals and both exact residual counters
        // as one infallible serialized phase.
        for id in &stale_body_cleanup.stale_stores {
            let key = self
                .pending_stores
                .get(id)
                .map(|pending| (pending.task.manifest.round, pending.task.manifest.subject))
                .expect("preflighted stale body-store work remains serialized");
            if Some(key) == protected_body {
                // Persistence work and its canonical bytes are immutable. A
                // timeout may replace the reducer consumer, but it must not
                // restart the exact durable-lock store or race cancellation
                // against a completion already minted by the worker.
                self.pending_stores
                    .get_mut(id)
                    .expect("preflighted protected store remains serialized")
                    .consumer = None;
            } else {
                self.pending_stores
                    .remove(id)
                    .expect("preflighted retired store remains serialized");
            }
        }
        for key in &stale_body_cleanup.stale_ready {
            if Some(*key) != protected_body {
                self.body_pipeline_owners.remove(key);
                self.ready_bodies
                    .remove(key)
                    .expect("preflighted stale ready body remains serialized");
            }
        }
        for rebind in stale_body_cleanup.protected_ready_rebinds {
            self.body_pipeline_owners.insert(rebind.key, rebind.owner);
        }
        self.ready_body_bytes = stale_body_cleanup.accounting.ready_after;
        self.pending_store_bytes = stale_body_cleanup.accounting.store_after;

        let retained_apply_owners = self
            .pending_applications
            .values()
            .map(|pending| (pending.task.certificate.round, pending.task.subject))
            .collect::<BTreeSet<_>>();
        self.body_pipeline_owners.retain(|key, owner| {
            owner.tag.view() >= tag.view() || retained_apply_owners.contains(key)
        });

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

    /// Reserve a signing slot, retiring one reconstructible fetch if needed.
    ///
    /// A durable signing intent is progress-critical and cannot depend on an
    /// unresponsive body source. Selection is stable by `(class, work_id)`:
    /// the oldest speculative fetch, then the oldest certified non-lock fetch,
    /// then the oldest durable-lock fetch. The decided body is never eligible.
    fn ensure_signature_slot<S: V2EffectServices>(
        &mut self,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let pending_work = self.pending_work();
        if pending_work < self.config.max_pending_work {
            return Ok(());
        }
        if pending_work > self.config.max_pending_work {
            return Err(EffectExecutorError::Contract(
                "pending effect work exceeded its configured capacity".to_owned(),
            ));
        }

        let selected = self
            .pending_fetches
            .iter()
            .filter_map(|(work_id, pending)| {
                let key = (pending.task.round, pending.task.subject);
                if Some(key) == self.protected_decision {
                    return None;
                }
                let class = if Some(key) == self.protected_lock {
                    2u8
                } else if pending.request_hash.is_none() {
                    0
                } else {
                    1
                };
                Some(((class, *work_id), pending.clone()))
            })
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, pending)| pending);
        let Some(pending) = selected else {
            return Err(EffectExecutorError::PendingWorkCapacity {
                capacity: self.config.max_pending_work,
            });
        };

        let key = (pending.task.round, pending.task.subject);
        let expected_owner = BodyPipelineOwner {
            tag: pending.task.tag,
            manifest_hash: pending.task.manifest.as_ref().map(HashOf::new),
        };
        if self.body_pipeline_owners.get(&key) != Some(&expected_owner) {
            return Err(EffectExecutorError::Contract(
                "signature preemption found a body fetch without its exact pipeline owner"
                    .to_owned(),
            ));
        }
        if self.ready_bodies.contains_key(&key)
            || self
                .pending_stores
                .values()
                .any(|store| (store.task.manifest.round, store.task.manifest.subject) == key)
            || self
                .pending_validations
                .values()
                .any(|validation| (validation.task.round(), validation.task.subject()) == key)
            || self.pending_applications.values().any(|application| {
                (application.task.certificate.round, application.task.subject) == key
            })
        {
            return Err(EffectExecutorError::Contract(
                "signature preemption found a body fetch overlapping a later pipeline stage"
                    .to_owned(),
            ));
        }
        let retirement = self.plan_pending_fetch_retirement(&pending)?;
        services
            .cancel_body_fetch(&pending.task)
            .map_err(service_error)?;
        self.commit_pending_fetch_retirement(retirement);
        let removed_owner = self.body_pipeline_owners.remove(&key);
        debug_assert_eq!(removed_owner, Some(expected_owner));
        iroha_logger::debug!(
            work_id = pending.task.id().get(),
            height = pending.task.round.height,
            view = pending.task.round.view,
            protected_lock = Some(key) == self.protected_lock,
            certified = pending.request_hash.is_some(),
            "preempted reconstructible body fetch for durable Sumeragi v2 signing"
        );
        self.ensure_pending_slot()
    }

    fn effect_dispatch_queue_snapshot(&self, now: Instant) -> RuntimeQueueLaneSnapshot {
        self.retained_effect_batch.as_ref().map_or(
            RuntimeQueueLaneSnapshot {
                depth: 0,
                capacity: MAX_EFFECTS_PER_STEP,
                oldest_age: None,
                max_service_debt: 0,
            },
            |batch| RuntimeQueueLaneSnapshot {
                depth: batch.effects.len(),
                capacity: MAX_EFFECTS_PER_STEP,
                oldest_age: Some(now.saturating_duration_since(batch.oldest_at)),
                // This suffix is always attempted before another runtime
                // transition. Pending-work capacity can make its head
                // temporarily ineligible, but the head never loses an
                // eligible scheduler dispatch to another queue.
                max_service_debt: 0,
            },
        )
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
        if self.output_guard.restart_required() {
            return Err(EffectExecutorError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            ));
        }
        match &self.fatal_reason {
            Some(reason) => Err(EffectExecutorError::FailClosed(reason.clone())),
            None => Ok(()),
        }
    }

    fn fail_closed_transport<S: V2EffectServices>(
        &mut self,
        reason: impl fmt::Display,
        services: &mut S,
    ) -> EffectTransportError {
        self.output_guard.activate_restart_required();
        let reason = self
            .fatal_reason
            .get_or_insert_with(|| reason.to_string())
            .clone();
        services.fail_closed(&reason);
        EffectTransportError::FailClosed(reason)
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
        self.output_guard.activate_restart_required();
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
    /// One runtime transition advanced or a retained causal suffix made progress.
    Advanced {
        /// Number of effects bound during this call. Any capacity-blocked
        /// suffix remains visible in the `EffectDispatch` status lane.
        effects: usize,
    },
}

fn verify_signer_completion(
    context: &wire::HeightContext,
    local_validator: Option<wire::ValidatorIndex>,
    request: &SignRequest,
    signature: &[u8],
) -> Result<(), String> {
    let signer = match request {
        SignRequest::Proposal(proposal) => proposal.proposer,
        SignRequest::Vote(vote) => vote.signer,
        SignRequest::TimeoutVote(vote) => vote.signer,
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
        .verify(
            context.roster[index].validator.public_key(),
            &request.signature_preimage(),
        )
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
) -> Result<Option<VerifiedPendingGenesisNexusAmxContext>, EffectExecutorError> {
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
    Ok(
        (context.height == 1).then_some(VerifiedPendingGenesisNexusAmxContext {
            hash: context.nexus_amx_context_hash,
        }),
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, num::NonZeroU64};

    use crate::sumeragi::{
        v2::{
            AdapterError, AdapterFingerprints, DecisionLocalProposalDisposition, SumeragiV2Adapter,
            VerifiedHeightContext, classify_decided_local_proposal,
        },
        v2_block_sync::{CommitCertificateAdmissionError, V2BlockSyncDiscovery},
        v2_core::Generation,
        v2_runtime::RuntimeQueueConfig,
    };
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

    #[derive(Clone, Debug, PartialEq, Eq)]
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
        reserved_body_available: Option<BodyAvailableReservation>,
        decided_body: Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        decision_on_next_step: Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        round_tag: Option<EventTag>,
        fail_enqueue: bool,
        fail_enqueue_hits: usize,
        panic_step: bool,
    }

    /// Actual executor/runtime ownership projected without retaining a shadow
    /// state machine. Fatal fail-stop metadata is deliberately excluded: a
    /// rejected admission may latch it, but no body owner or accounting value
    /// may change.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct BodyOwnershipProjection {
        next_work_id: u64,
        pending_fetches: BTreeMap<EffectWorkId, PendingFetch>,
        pending_stores: BTreeMap<EffectWorkId, PendingStore>,
        pending_validations: BTreeMap<EffectWorkId, PendingValidation>,
        deferred_merge_work: BTreeMap<EffectWorkId, HashOf<MergeLedgerEntry>>,
        body_pipeline_owners:
            BTreeMap<(wire::ConsensusRound, wire::BlockSubject), BodyPipelineOwner>,
        certified_work: BTreeMap<HashOf<wire::CertifiedBodyRequest>, EffectWorkId>,
        outstanding_request_hashes: BTreeSet<HashOf<wire::CertifiedBodyRequest>>,
        ready_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ReadyBody>,
        retained_locked_body: Option<(wire::BlockSubject, Arc<[u8]>)>,
        ready_body_bytes: u64,
        pending_store_bytes: u64,
        recovered_bodies: BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            (wire::PayloadManifest, DurableBodyReceipt),
        >,
        durable_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
        validated_bodies:
            BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
        rejected_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
        runtime_completions: Vec<RuntimeCompletion>,
        runtime_body_reservation: Option<BodyAvailableReservation>,
    }

    impl V2EffectExecutor<FakeRuntime> {
        fn body_ownership_projection(&self) -> BodyOwnershipProjection {
            BodyOwnershipProjection {
                next_work_id: self.next_work_id,
                pending_fetches: self.pending_fetches.clone(),
                pending_stores: self.pending_stores.clone(),
                pending_validations: self.pending_validations.clone(),
                deferred_merge_work: self.deferred_merge_work.clone(),
                body_pipeline_owners: self.body_pipeline_owners.clone(),
                certified_work: self.certified_work.clone(),
                outstanding_request_hashes: self.outstanding_requests.hashes(),
                ready_bodies: self.ready_bodies.clone(),
                retained_locked_body: self.retained_locked_body.clone(),
                ready_body_bytes: self.ready_body_bytes,
                pending_store_bytes: self.pending_store_bytes,
                recovered_bodies: self.recovered_bodies.clone(),
                durable_bodies: self.durable_bodies.clone(),
                validated_bodies: self.validated_bodies.clone(),
                rejected_bodies: self.rejected_bodies.clone(),
                runtime_completions: self.runtime.completions.clone(),
                runtime_body_reservation: self.runtime.reserved_body_available.clone(),
            }
        }
    }

    impl FakeRuntime {
        fn push(&mut self, completion: RuntimeCompletion) -> Result<(), EnqueueError> {
            if self.fail_enqueue {
                self.fail_enqueue_hits = self.fail_enqueue_hits.saturating_add(1);
                return Err(EnqueueError::Full);
            }
            self.completions.push(completion);
            Ok(())
        }
    }

    impl EffectRuntime for FakeRuntime {
        fn step_effects(&mut self, _now: Instant) -> Result<RuntimeStep<AdapterEffect>, String> {
            assert!(!self.panic_step, "model safety-WAL step panic");
            let step = self.steps.pop_front().unwrap_or(Ok(RuntimeStep::Idle));
            if matches!(&step, Ok(RuntimeStep::Advanced(_)))
                && let Some(decision) = self.decision_on_next_step.take()
            {
                self.decided_body = Some(decision);
            }
            step
        }

        fn step_recovery_effects(
            &mut self,
            now: Instant,
        ) -> Result<RuntimeStep<AdapterEffect>, String> {
            self.step_effects(now)
        }

        fn decided_body(
            &self,
        ) -> Result<
            Option<(
                wire::ConsensusRound,
                wire::BlockSubject,
                wire::ExecutionCommitment,
            )>,
            String,
        > {
            Ok(self.decided_body)
        }

        fn enqueue_body_available(
            &mut self,
            tag: EventTag,
            manifest: wire::PayloadManifest,
        ) -> Result<(), EnqueueError> {
            self.push(RuntimeCompletion::BodyAvailable(tag, manifest))
        }

        fn reserve_body_available(
            &mut self,
            tag: EventTag,
            manifest: wire::PayloadManifest,
        ) -> Result<BodyAvailableReservation, EnqueueError> {
            let mut logical_owners = 0usize;
            let mut exact_owners = 0usize;
            for completion in &self.completions {
                if let RuntimeCompletion::BodyAvailable(queued_tag, queued_manifest) = completion
                    && *queued_tag == tag
                    && queued_manifest.round == manifest.round
                    && queued_manifest.subject == manifest.subject
                {
                    logical_owners = logical_owners.saturating_add(1);
                    exact_owners =
                        exact_owners.saturating_add(usize::from(queued_manifest == &manifest));
                }
            }
            match (logical_owners, exact_owners) {
                (1, 1) => return Ok(BodyAvailableReservation::coalesced(tag, manifest)),
                (0, 0) => {}
                _ => return Err(EnqueueError::DuplicateCompletionOwnership),
            }
            if self.fail_enqueue {
                self.fail_enqueue_hits = self.fail_enqueue_hits.saturating_add(1);
                return Err(EnqueueError::Full);
            }
            if self.reserved_body_available.is_some() || self.completions.len() >= 16 {
                return Err(EnqueueError::Full);
            }
            let reservation = BodyAvailableReservation::reserved(tag, manifest);
            self.reserved_body_available = Some(reservation.clone());
            Ok(reservation)
        }

        fn commit_body_available(&mut self, reservation: BodyAvailableReservation) {
            if !reservation.owns_new_slot() {
                return;
            }
            if self.reserved_body_available.as_ref() != Some(&reservation) {
                return;
            }
            self.reserved_body_available = None;
            self.completions.push(RuntimeCompletion::BodyAvailable(
                reservation.tag(),
                reservation.manifest().clone(),
            ));
        }

        fn abort_body_available(&mut self, reservation: BodyAvailableReservation) {
            if reservation.owns_new_slot()
                && self.reserved_body_available.as_ref() == Some(&reservation)
            {
                self.reserved_body_available = None;
            }
        }

        fn rebind_body_available(
            &mut self,
            previous: EventTag,
            rebound: EventTag,
            manifest: &wire::PayloadManifest,
        ) -> Result<bool, String> {
            let mut rebound_count = 0usize;
            for completion in &mut self.completions {
                if let RuntimeCompletion::BodyAvailable(tag, queued) = completion
                    && *tag == previous
                    && queued == manifest
                {
                    *tag = rebound;
                    rebound_count = rebound_count.saturating_add(1);
                }
            }
            if rebound_count > 1 {
                return Err("duplicate queued body-available completions".to_owned());
            }
            Ok(rebound_count == 1)
        }

        fn retire_body_available(
            &mut self,
            tag: EventTag,
            manifest: &wire::PayloadManifest,
        ) -> Result<bool, String> {
            let before = self.completions.len();
            self.completions.retain(|completion| {
                !matches!(
                    completion,
                    RuntimeCompletion::BodyAvailable(queued_tag, queued_manifest)
                        if *queued_tag == tag && queued_manifest == manifest
                )
            });
            let retired = before.saturating_sub(self.completions.len());
            if retired > 1 {
                return Err("duplicate queued body-available completions".to_owned());
            }
            Ok(retired == 1)
        }

        fn retire_body_pipeline_completions(
            &mut self,
            tag: EventTag,
            round: wire::ConsensusRound,
            subject: wire::BlockSubject,
        ) -> Result<RetiredBodyPipelineCompletions, String> {
            let mut retired = RetiredBodyPipelineCompletions::default();
            self.completions.retain(|completion| {
                let remove = match completion {
                    RuntimeCompletion::BodyAvailable(queued_tag, manifest)
                        if *queued_tag == tag
                            && manifest.round == round
                            && manifest.subject == subject =>
                    {
                        retired.record_body_available();
                        true
                    }
                    RuntimeCompletion::BodyStored(queued_tag, queued_round, queued_subject, _)
                        if *queued_tag == tag
                            && *queued_round == round
                            && *queued_subject == subject =>
                    {
                        retired.record_body_stored();
                        true
                    }
                    RuntimeCompletion::ValidationSucceeded(
                        queued_tag,
                        queued_round,
                        queued_subject,
                        _,
                    )
                    | RuntimeCompletion::ValidationFailed(
                        queued_tag,
                        queued_round,
                        queued_subject,
                    ) if *queued_tag == tag
                        && *queued_round == round
                        && *queued_subject == subject =>
                    {
                        retired.record_validation();
                        true
                    }
                    RuntimeCompletion::LocalProposal(queued_tag, manifest, ..)
                        if *queued_tag == tag
                            && manifest.round == round
                            && manifest.subject == subject =>
                    {
                        retired.record_local_proposal();
                        true
                    }
                    RuntimeCompletion::BodyAvailable(..)
                    | RuntimeCompletion::BodyStored(..)
                    | RuntimeCompletion::ValidationSucceeded(..)
                    | RuntimeCompletion::ValidationFailed(..)
                    | RuntimeCompletion::Signature(..)
                    | RuntimeCompletion::Application(..)
                    | RuntimeCompletion::LocalProposal(..) => false,
                };
                !remove
            });
            Ok(retired)
        }

        fn retire_unsafe_proposals_for_lock(
            &mut self,
            _locked_round: wire::ConsensusRound,
            _locked_subject: wire::BlockSubject,
        ) -> Result<usize, String> {
            Ok(0)
        }

        fn retire_proposal_work_after_decision(
            &mut self,
            decision_round: wire::ConsensusRound,
            decision_subject: wire::BlockSubject,
            decision_commitment: wire::ExecutionCommitment,
        ) -> Result<DecisionProposalRetirement, String> {
            let decision_tag = self.round_tag.unwrap_or_else(|| {
                EventTag::new(
                    decision_round.height,
                    decision_round.view,
                    Generation::new(7),
                )
            });
            let mut retainable = 0usize;
            let mut recovery_only = 0usize;
            let mut conflicting = 0usize;
            for completion in &self.completions {
                let RuntimeCompletion::LocalProposal(
                    queued_tag,
                    manifest,
                    durable_receipt,
                    validated_receipt,
                ) = completion
                else {
                    continue;
                };
                match classify_decided_local_proposal(
                    *queued_tag,
                    manifest,
                    durable_receipt,
                    validated_receipt,
                    decision_tag,
                    decision_round,
                    decision_subject,
                    decision_commitment,
                ) {
                    Some(DecisionLocalProposalDisposition::Retain) => {
                        retainable = retainable.saturating_add(1);
                    }
                    Some(DecisionLocalProposalDisposition::RetireForRecovery) => {
                        recovery_only = recovery_only.saturating_add(1);
                    }
                    Some(DecisionLocalProposalDisposition::Conflict) => {
                        conflicting = conflicting.saturating_add(1);
                    }
                    None => {}
                }
            }
            if conflicting != 0 {
                return Err(
                    "decided local-proposal evidence conflicts with the durable Decision"
                        .to_owned(),
                );
            }
            if retainable.saturating_add(recovery_only) > 1 {
                return Err("duplicate exact decided local-proposal completions".to_owned());
            }
            self.completions.retain(|completion| {
                if let RuntimeCompletion::LocalProposal(
                    queued_tag,
                    manifest,
                    durable_receipt,
                    validated_receipt,
                ) = completion
                    && manifest.round.height == decision_round.height
                {
                    return matches!(
                        classify_decided_local_proposal(
                            *queued_tag,
                            manifest,
                            durable_receipt,
                            validated_receipt,
                            decision_tag,
                            decision_round,
                            decision_subject,
                            decision_commitment,
                        ),
                        Some(DecisionLocalProposalDisposition::Retain)
                    );
                }
                true
            });
            Ok(DecisionProposalRetirement::new(
                (retainable == 1).then_some(decision_tag),
                recovery_only,
            ))
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

        fn enqueue_validation_failures_atomically(
            &mut self,
            failures: &[(EventTag, wire::ConsensusRound, wire::BlockSubject)],
        ) -> Result<(), EnqueueError> {
            if self.fail_enqueue {
                self.fail_enqueue_hits = self.fail_enqueue_hits.saturating_add(1);
                return Err(EnqueueError::Full);
            }
            self.completions
                .extend(failures.iter().copied().map(|(tag, round, subject)| {
                    RuntimeCompletion::ValidationFailed(tag, round, subject)
                }));
            Ok(())
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

        fn remaining_completion_capacity(&self) -> usize {
            16usize.saturating_sub(
                self.completions
                    .len()
                    .saturating_add(usize::from(self.reserved_body_available.is_some())),
            )
        }

        fn queue_snapshot(&self, _now: Instant) -> RuntimeQueueSnapshot {
            let empty = RuntimeQueueLaneSnapshot {
                depth: 0,
                capacity: 16,
                oldest_age: None,
                max_service_debt: 0,
            };
            RuntimeQueueSnapshot {
                normal: empty,
                progress: empty,
                completion: RuntimeQueueLaneSnapshot {
                    depth: self.completions.len(),
                    ..empty
                },
            }
        }

        fn watchdog_threshold(&self) -> Duration {
            Duration::from_secs(12)
        }
    }

    #[derive(Default)]
    struct FakeServices {
        _body_directory: Option<TempDir>,
        body_store: Option<V2BodyStore>,
        requester_key: Option<KeyPair>,
        effect_service_order: Vec<&'static str>,
        sign_tasks: Vec<ConsensusSignTask>,
        cancelled_signatures: Vec<EffectWorkId>,
        retired_outbound_subjects: Vec<wire::BlockSubject>,
        retired_all_outbound: usize,
        retired_candidate_work: usize,
        broadcasts: Vec<wire::ConsensusMessageV2>,
        fetch_tasks: Vec<BodyFetchTask>,
        cancelled_fetches: Vec<EffectWorkId>,
        completed_reconstruction_fetches: Vec<EffectWorkId>,
        completed_certified_fetches: Vec<EffectWorkId>,
        chunks: Vec<EffectWorkId>,
        reject_authenticated_chunks: bool,
        store_tasks: Vec<BodyStoreTask>,
        cancelled_stores: Vec<EffectWorkId>,
        inflight_stores: BTreeSet<EffectWorkId>,
        validation_tasks: Vec<BodyValidationTask>,
        cancelled_validations: Vec<EffectWorkId>,
        deferred_merge_sidecars: Vec<(
            EffectWorkId,
            wire::ConsensusRound,
            wire::BlockSubject,
            CertifiedMergeLedgerReference,
        )>,
        apply_tasks: Vec<ApplyTask>,
        entered_views: Vec<EventTag>,
        equivocations: Vec<(PeerId, wire::ConsensusRound, EquivocationKind)>,
        invalid_bodies: Vec<wire::BlockSubject>,
        rejected_validations: Vec<String>,
        statuses: Vec<EffectExecutorStatus>,
        closed: Vec<String>,
        fail_on: Option<&'static str>,
        fail_on_call: Option<(&'static str, usize)>,
        operation_calls: BTreeMap<&'static str, usize>,
        validation_error: Option<String>,
    }

    impl FakeServices {
        fn check(&mut self, operation: &'static str) -> Result<(), String> {
            let call = *self
                .operation_calls
                .entry(operation)
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
            if self.fail_on == Some(operation) {
                self.fail_on = None;
                Err(format!("{operation} failed"))
            } else if self.fail_on_call == Some((operation, call)) {
                self.fail_on_call = None;
                Err(format!("{operation} call {call} failed"))
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
            self.effect_service_order.push("sign");
            self.sign_tasks.push(task);
            Ok(())
        }

        fn cancel_consensus_sign(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
            self.check("cancel-sign")?;
            self.cancelled_signatures.push(work_id);
            Ok(())
        }

        fn retire_outbound_payload_for_subject(
            &mut self,
            subject: wire::BlockSubject,
        ) -> Result<(), Self::Error> {
            self.check("retire-outbound-subject")?;
            self.retired_outbound_subjects.push(subject);
            Ok(())
        }

        fn retire_all_outbound_payloads(&mut self) -> Result<(), Self::Error> {
            self.check("retire-all-outbound")?;
            self.retired_all_outbound = self.retired_all_outbound.saturating_add(1);
            Ok(())
        }

        fn retire_candidate_work_after_decision(
            &mut self,
            _decision_round: wire::ConsensusRound,
            _decision_subject: wire::BlockSubject,
        ) -> Result<(), Self::Error> {
            self.check("retire-candidate-work")?;
            self.retired_candidate_work = self.retired_candidate_work.saturating_add(1);
            Ok(())
        }

        fn broadcast_consensus(
            &mut self,
            message: wire::ConsensusMessageV2,
        ) -> Result<(), Self::Error> {
            self.check("broadcast")?;
            self.effect_service_order.push("broadcast");
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

        fn rebind_body_fetch(
            &mut self,
            previous: &BodyFetchTask,
            rebound: BodyFetchTask,
        ) -> Result<(), Self::Error> {
            self.check("rebind-fetch")?;
            if !rebound.rebinds_consumer_of(previous) {
                return Err("invalid body-fetch consumer rebind".to_owned());
            }
            let owned = self
                .fetch_tasks
                .iter_mut()
                .rev()
                .find(|task| task.id() == previous.id())
                .ok_or_else(|| "body-fetch consumer rebind has no service owner".to_owned())?;
            if owned != previous {
                return Err("body-fetch consumer rebind differs from service ownership".to_owned());
            }
            *owned = rebound;
            Ok(())
        }

        fn cancel_body_fetch(&mut self, task: &BodyFetchTask) -> Result<(), Self::Error> {
            self.check("cancel-fetch")?;
            self.cancelled_fetches.push(task.id());
            Ok(())
        }

        fn complete_body_reconstruction_fetch(
            &mut self,
            task: &BodyFetchTask,
        ) -> Result<(), Self::Error> {
            self.check("complete-reconstruction-fetch")?;
            self.completed_reconstruction_fetches.push(task.id());
            Ok(())
        }

        fn complete_certified_body_fetch(
            &mut self,
            task: &BodyFetchTask,
        ) -> Result<(), Self::Error> {
            self.check("complete-certified-fetch")?;
            self.completed_certified_fetches.push(task.id());
            Ok(())
        }

        fn accept_authenticated_chunk(
            &mut self,
            task: &BodyFetchTask,
            _chunk: AuthenticatedPayloadChunk,
        ) -> Result<AuthenticatedChunkDisposition, Self::Error> {
            self.check("chunk")?;
            self.chunks.push(task.id());
            Ok(if self.reject_authenticated_chunks {
                AuthenticatedChunkDisposition::Rejected
            } else {
                AuthenticatedChunkDisposition::Accepted
            })
        }

        fn enqueue_body_store(&mut self, task: BodyStoreTask) -> Result<(), Self::Error> {
            self.check("store")?;
            self.store_tasks.push(task);
            Ok(())
        }

        fn cancel_body_store(&mut self, work_id: EffectWorkId) -> Result<bool, Self::Error> {
            self.check("cancel-store")?;
            self.cancelled_stores.push(work_id);
            Ok(!self.inflight_stores.contains(&work_id))
        }

        fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error> {
            self.check("validation")?;
            self.validation_tasks.push(task);
            Ok(())
        }

        fn cancel_body_validation(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
            self.check("cancel-validation")?;
            self.cancelled_validations.push(work_id);
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
            offender: PeerId,
            round: wire::ConsensusRound,
            kind: EquivocationKind,
        ) -> Result<(), Self::Error> {
            self.check("equivocation")?;
            self.effect_service_order.push("equivocation");
            self.equivocations.push((offender, round, kind));
            Ok(())
        }

        fn report_invalid_certified_body(
            &mut self,
            subject: wire::BlockSubject,
            _certificate: wire::QuorumCertificate,
        ) -> Result<(), Self::Error> {
            self.check("invalid-body")?;
            self.effect_service_order.push("invalid-body");
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
                snapshot_bootstrap: None,
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

    struct ProductionTransportFixture {
        _directory: TempDir,
        context: wire::HeightContext,
        validator_keys: Vec<KeyPair>,
        requester_key: KeyPair,
        responder_key: KeyPair,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        conflicting_commitment: wire::ExecutionCommitment,
        executor: V2EffectExecutor,
    }

    impl ProductionTransportFixture {
        fn new() -> Self {
            let mut validator_keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic BLS validator key")
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
                chain_id: "v2-production-transport-regression".into(),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("dual quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"production transport nexus/amx context"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::Plain,
                    chunk_size_bytes: 1_048_576,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 1_048_576,
                    max_chunk_count: 1,
                },
                leader_seed: [0x62; 32],
            };
            let round = round(&context, 0);
            let body = b"production transport commitment regression".to_vec();
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"production transport block",
                )),
                payload_hash: Hash::new(&body),
            };
            let manifest = wire::PayloadManifest::derive(
                &context,
                round,
                subject,
                u64::try_from(body.len()).expect("body length"),
                std::slice::from_ref(&body),
            )
            .expect("canonical production transport manifest");
            let durable =
                DurableBodyReceipt::for_test(context.id(), round, subject, HashOf::new(&manifest));
            let validated = ValidatedBodyReceipt::for_test(durable.clone());
            let canonical_commitment = validated.execution_commitment();
            let conflicting_commitment = wire::ExecutionCommitment::without_topups(
                Hash::new(b"conflicting parent state"),
                Hash::new(b"conflicting post state"),
                Hash::new(b"conflicting ordinary writes"),
                Hash::new(b"conflicting executed block wire"),
            );
            assert_ne!(canonical_commitment, conflicting_commitment);

            let proofs = validator_keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("validator proof of possession")
                })
                .collect::<Vec<_>>();
            let verified =
                VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified context");
            let directory = TempDir::new().expect("production runtime directory");
            let (adapter, startup_effects) = SumeragiV2Adapter::open(
                directory.path().join("transport-regression-safety.wal"),
                verified,
                None,
                Generation::new(1),
                [0x63; 32],
                AdapterFingerprints {
                    node: Hash::new(b"production transport node"),
                    build: Hash::new(b"production transport build"),
                    config: Hash::new(b"production transport config"),
                },
            )
            .expect("open production adapter");
            assert!(startup_effects.is_empty());
            let started = Instant::now();
            let (mut runtime, startup_effects) = SerializedV2Runtime::new(
                adapter,
                startup_effects,
                started,
                Duration::from_secs(10),
                RuntimeQueueConfig::new(8, 2, 2),
            )
            .expect("serialized production runtime");
            assert!(startup_effects.is_empty());
            runtime
                .recover_validated_body(&manifest, &validated)
                .expect("bind locally validated execution commitment");

            let requester_key = KeyPair::try_from_seed(vec![90; 32], Algorithm::BlsNormal)
                .expect("deterministic requester key");
            let responder_key = KeyPair::try_from_seed(vec![91; 32], Algorithm::BlsNormal)
                .expect("deterministic responder key");
            let recovered_bodies = BTreeMap::from([((round, subject), (manifest, durable))]);
            let executor = V2EffectExecutor::with_runtime(
                runtime,
                recovered_bodies,
                context.clone(),
                PeerId::new(requester_key.public_key().clone()),
                None,
                EffectQueueConfig::default(),
            )
            .expect("production effect executor");

            Self {
                _directory: directory,
                context,
                validator_keys,
                requester_key,
                responder_key,
                round,
                subject,
                conflicting_commitment,
                executor,
            }
        }

        fn quorum_certificate(
            &self,
            phase: wire::GlobalPhase,
            execution_commitment: wire::ExecutionCommitment,
        ) -> wire::QuorumCertificate {
            let signers = vec![0, 1, 2];
            let preimage = wire::Vote {
                round: self.round,
                phase,
                subject: self.subject,
                execution_commitment,
                signer: signers[0],
                signature: Vec::new(),
            }
            .signature_preimage();
            let shares = signers
                .iter()
                .map(|signer| {
                    Signature::new(
                        self.validator_keys[usize::try_from(*signer).expect("small signer index")]
                            .private_key(),
                        &preimage,
                    )
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
            wire::QuorumCertificate {
                round: self.round,
                phase,
                subject: self.subject,
                execution_commitment,
                signers,
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                    .expect("aggregate quorum certificate"),
            }
        }

        fn certified_body_request(
            &self,
            certificate: wire::QuorumCertificate,
        ) -> wire::CertifiedBodyRequest {
            let mut request = wire::CertifiedBodyRequest {
                round: self.round,
                subject: self.subject,
                certificate,
                requester: PeerId::new(self.requester_key.public_key().clone()),
                signature: Vec::new(),
            };
            request.signature = Signature::new(
                self.requester_key.private_key(),
                &request.signature_preimage(),
            )
            .payload()
            .to_vec();
            request
        }
    }

    #[test]
    fn production_certified_body_request_rejects_locally_conflicting_qc_without_fail_close() {
        let fixture = ProductionTransportFixture::new();
        let certificate =
            fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.conflicting_commitment);
        let request = fixture.certified_body_request(certificate);
        let requester = PeerId::new(fixture.requester_key.public_key().clone());

        assert!(matches!(
            fixture
                .executor
                .authenticate_certified_body_request(request, &requester),
            Err(V2TransportError::CertificateRejected(reason))
                if reason.contains("conflicting Sumeragi v2 execution commitments")
        ));
        assert!(fixture.executor.runtime.driver().ingress_ready());
        assert!(!fixture.executor.status().fail_closed);
    }

    #[test]
    fn production_commit_certificate_response_conflict_keeps_discovery_outstanding_and_runtime_open()
     {
        let mut fixture = ProductionTransportFixture::new();
        let requester = PeerId::new(fixture.requester_key.public_key().clone());
        let mut discovery = V2BlockSyncDiscovery::new(fixture.context.clone(), requester, 1)
            .expect("current-height discovery");
        let request_envelope = discovery
            .begin(&fixture.requester_key)
            .expect("begin signed current-height request");
        let wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) =
            request_envelope.payload
        else {
            panic!("discovery emits a CommitCertificateRequest")
        };
        let request_hash = HashOf::new(&request);
        let mut response = wire::CommitCertificateResponse {
            request_hash,
            certificate: fixture
                .quorum_certificate(wire::GlobalPhase::Commit, fixture.conflicting_commitment),
            responder: PeerId::new(fixture.responder_key.public_key().clone()),
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.responder_key.private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        let responder = PeerId::new(fixture.responder_key.public_key().clone());
        let retry = response.clone();
        let discovered = discovery
            .authenticate_response(response, &responder)
            .expect("authenticate signed outer response");

        let admission = discovery.enqueue_and_complete(discovered, |message| {
            fixture.executor.enqueue_network(message).map(|_| ())
        });
        assert!(matches!(
            admission,
            Err(CommitCertificateAdmissionError::Enqueue(
                NetworkIngressError::Authentication(AdapterError::ConflictingExecutionCommitment)
            ))
        ));
        assert_eq!(discovery.outstanding_len(), 1);
        assert!(discovery.retransmit(request_hash).is_some());
        let _authenticated_retry = discovery
            .authenticate_response(retry, &responder)
            .expect("rejected runtime handoff leaves the response retryable");
        assert!(fixture.executor.runtime.driver().ingress_ready());
        assert!(!fixture.executor.status().fail_closed);
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
            Hash::new(b"effects fixture executed block wire"),
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

    fn proposal(fixture: &Fixture) -> wire::ConsensusMessageV2 {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
            round: fixture.manifest.round,
            proposer: fixture.context.leader(fixture.manifest.round.view),
            subject: fixture.manifest.subject,
            manifest: fixture.manifest.clone(),
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: vec![0x91],
        }))
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

    fn distinct_body(fixture: &Fixture) -> (wire::BlockSubject, Vec<u8>) {
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            2_000,
            0,
        );
        let signature =
            SignatureOf::try_from_hash(fixture.validator_keys[0].private_key(), header.hash())
                .expect("distinct block signature");
        let block = SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
        let body = block.encode_wire().expect("distinct canonical body");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        (subject, body)
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

    fn timeout_sign(fixture: &Fixture, view: u64) -> AdapterEffect {
        AdapterEffect::Sign {
            tag: tag(view),
            request: SignRequest::TimeoutVote(wire::TimeoutVote {
                round: round(&fixture.context, view),
                highest_prepare_qc: None,
                signer: 0,
                signature: Vec::new(),
            }),
        }
    }

    fn prepare_qc_for_subject(
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> wire::QuorumCertificate {
        wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: fixture_execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        }
    }

    fn certified_sources(fixture: &Fixture, certificate: &wire::QuorumCertificate) -> Vec<PeerId> {
        certificate
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect()
    }

    fn manifest_for_payload(fixture: &Fixture, label: &'static [u8]) -> wire::PayloadManifest {
        let body = label.to_vec();
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
            payload_hash: Hash::new(&body),
        };
        wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            subject,
            u64::try_from(body.len()).expect("payload length"),
            std::slice::from_ref(&body),
        )
        .expect("payload manifest")
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
        let manifest = wire::PayloadManifest::derive(
            &fixture.context,
            round,
            subject,
            u64::try_from(fixture.body.len()).expect("body length"),
            std::slice::from_ref(&fixture.body),
        )
        .expect("merge carrier manifest");
        let durable_receipt = DurableBodyReceipt::for_test(
            fixture.context.id(),
            round,
            subject,
            HashOf::new(&manifest),
        );
        let task = BodyValidationTask {
            id: EffectWorkId(77),
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
                consumer: Some(ValidationConsumer::Reducer { tag: tag(3) }),
            },
            reference,
            entry_hash,
        )
    }

    fn begin_reachable_merge_validation(
        fixture: &Fixture,
        executor: &mut V2EffectExecutor<FakeRuntime>,
        services: &mut FakeServices,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> BodyValidationTask {
        let manifest = wire::PayloadManifest::derive(
            &fixture.context,
            round,
            subject,
            u64::try_from(fixture.body.len()).expect("body length"),
            std::slice::from_ref(&fixture.body),
        )
        .expect("reachable merge carrier manifest");
        let durable = DurableBodyReceipt::for_test(
            fixture.context.id(),
            round,
            subject,
            HashOf::new(&manifest),
        );
        let key = (round, subject);
        executor
            .recovered_bodies
            .insert(key, (manifest.clone(), durable.clone()));
        executor.durable_bodies.insert(key, durable.clone());
        executor
            .bind_body_pipeline_owner(tag(round.view), &manifest)
            .expect("bind the exact production validation owner");
        executor
            .begin_validation(
                round,
                subject,
                durable,
                ValidationConsumer::Reducer {
                    tag: tag(round.view),
                },
                services,
            )
            .expect("start validation through the production admission path");
        services
            .validation_tasks
            .last()
            .expect("production validation task")
            .clone()
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
    fn queue_configuration_rejects_zero_and_pending_capacity_retains_causal_tail() {
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
        assert!(executor.can_admit_local_proposal());
        let effects = vec![
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            },
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            },
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            },
        ];
        assert_eq!(
            executor
                .consume_effects(effects, &mut services)
                .expect("retain the capacity-blocked causal suffix"),
            1
        );
        assert_eq!(executor.status().pending_signatures, 1);
        assert_eq!(executor.status().effect_dispatch_queue.depth, 2);
        assert_eq!(
            executor.status().effect_dispatch_queue.capacity,
            MAX_EFFECTS_PER_STEP
        );
        assert_eq!(executor.status().effect_dispatch_queue.max_service_debt, 0);
        assert!(!executor.can_admit_local_proposal());
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());

        let first = services.sign_tasks[0].clone();
        let signature = Signature::new(
            fixture.validator_keys[0].private_key(),
            &first.request.signature_preimage(),
        )
        .payload()
        .to_vec();
        executor
            .complete_consensus_signature(first.id(), signature, &mut services)
            .expect("release the first signing slot");
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("drain retained signing debt before another runtime step"),
            EffectExecutorStep::Advanced { effects: 1 }
        );
        assert_eq!(services.sign_tasks.len(), 2);
        assert_eq!(executor.status().pending_signatures, 1);
        assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
        assert_eq!(
            executor.status().effect_dispatch_queue.max_service_debt,
            0,
            "capacity retry is not scheduler debt and cannot transfer between FIFO heads"
        );

        let second = services.sign_tasks[1].clone();
        let signature = Signature::new(
            fixture.validator_keys[0].private_key(),
            &second.request.signature_preimage(),
        )
        .payload()
        .to_vec();
        executor
            .complete_consensus_signature(second.id(), signature, &mut services)
            .expect("release the second signing slot");
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("drain the final retained signing effect"),
            EffectExecutorStep::Advanced { effects: 1 }
        );
        assert_eq!(services.sign_tasks.len(), 3);
        assert_eq!(executor.status().pending_signatures, 1);
        assert_eq!(executor.status().effect_dispatch_queue.depth, 0);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn proposal_a_distinct_prepare_qc_b_and_timeout_sign_progress_at_capacity_two() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(2, 4, 1 << 20, 4));
        let mut services = fixture.services();

        // reducer.rs::on_proposal: an authenticated Proposal A with a missing
        // body emits the ordinary reconstruction request.
        fixture
            .manifest
            .validate(&fixture.context)
            .expect("Proposal A manifest is structurally valid");
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
            .expect("Proposal A starts ordinary reconstruction");
        let proposal_a_work = services.fetch_tasks[0].id();

        // reducer.rs::on_prepare_qc: a valid same-view PrepareQC for distinct
        // subject B is independently progress-relevant and starts a certified
        // reconstruction owner.
        let (subject_b, _) = distinct_body(&fixture);
        assert_ne!(subject_b, fixture.manifest.subject);
        let prepare_b = prepare_qc_for_subject(fixture.manifest.round, subject_b);
        prepare_b
            .validate(&fixture.context)
            .expect("distinct PrepareQC B is structurally valid");
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: prepare_b.round,
                    subject: prepare_b.subject,
                    manifest: None,
                    certified_sources: certified_sources(&fixture, &prepare_b),
                    certificate: Some(prepare_b.clone()),
                }],
                &mut services,
            )
            .expect("PrepareQC B starts certified reconstruction");
        assert_eq!(executor.pending_work(), 2);

        // reducer.rs::on_timeout: durable TimeoutVote signing must not fail
        // closed behind either body source. It deterministically retires the
        // lower-evidence Proposal A fetch and owns the released slot.
        assert_eq!(
            executor
                .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
                .expect("timeout signing preempts reconstructible work"),
            1
        );
        assert_eq!(services.cancelled_fetches, vec![proposal_a_work]);
        assert_eq!(executor.pending_work(), 2);
        assert_eq!(executor.pending_signatures.len(), 1);
        assert_eq!(executor.pending_fetches.len(), 1);
        assert!(executor.pending_fetches.values().all(|pending| {
            pending.task.round == prepare_b.round && pending.task.subject == prepare_b.subject
        }));
        assert!(executor.retained_effect_batch.is_none());
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn serialized_runtime_emits_proposal_a_prepare_qc_b_timeout_capacity_trace() {
        let ProductionTransportFixture {
            context,
            validator_keys,
            requester_key,
            ..
        } = ProductionTransportFixture::new();
        let proofs = validator_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified =
            VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified context");
        let directory = TempDir::new().expect("serialized capacity-trace directory");
        let (adapter, startup_effects) = SumeragiV2Adapter::open(
            directory.path().join("capacity-trace-safety.wal"),
            verified,
            Some(0),
            Generation::new(1),
            [0x74; 32],
            AdapterFingerprints {
                node: Hash::new(b"capacity trace node"),
                build: Hash::new(b"capacity trace build"),
                config: Hash::new(b"capacity trace config"),
            },
        )
        .expect("open source-faithful adapter");
        assert!(startup_effects.is_empty());
        let started = Instant::now();
        let (runtime, startup_effects) = SerializedV2Runtime::new(
            adapter,
            startup_effects,
            started,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
        )
        .expect("serialized runtime");
        assert!(startup_effects.is_empty());
        let mut executor = V2EffectExecutor::with_runtime(
            runtime,
            BTreeMap::new(),
            context.clone(),
            PeerId::new(requester_key.public_key().clone()),
            Some(0),
            EffectQueueConfig::new(2, 4, 1 << 20, 4),
        )
        .expect("capacity-two executor");
        executor
            .arm_live_clocks(started)
            .expect("arm source-faithful timeout");
        let mut services = FakeServices {
            requester_key: Some(requester_key),
            ..FakeServices::default()
        };

        let round = round(&context, 0);
        let body_a = b"authenticated Proposal A body".to_vec();
        let subject_a = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"Proposal A block")),
            payload_hash: Hash::new(&body_a),
        };
        let manifest_a = wire::PayloadManifest::derive(
            &context,
            round,
            subject_a,
            u64::try_from(body_a.len()).expect("Proposal A length"),
            std::slice::from_ref(&body_a),
        )
        .expect("Proposal A manifest");
        let proposer = context.leader(0);
        let mut proposal_a = wire::Proposal {
            round,
            proposer,
            subject: subject_a,
            manifest: manifest_a,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        proposal_a.signature = Signature::new(
            validator_keys[usize::try_from(proposer).expect("leader index")].private_key(),
            &proposal_a.signature_preimage(),
        )
        .payload()
        .to_vec();
        executor
            .enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal_a),
            ))
            .expect("authenticate Proposal A through production ingress");
        for _ in 0..8 {
            let _ = executor
                .step(started, &mut services)
                .expect("drive Proposal A reducer transition");
            if executor.pending_fetches.len() == 1 {
                break;
            }
        }
        assert_eq!(executor.pending_fetches.len(), 1);
        let proposal_a_work = services.fetch_tasks[0].id();

        let body_b = b"distinct certified subject B".to_vec();
        let subject_b = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"PrepareQC B block")),
            payload_hash: Hash::new(&body_b),
        };
        let commitment_b = fixture_execution_commitment();
        let signers = vec![0, 1, 2];
        let vote_preimage = wire::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject_b,
            execution_commitment: commitment_b,
            signer: signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    validator_keys[usize::try_from(*signer).expect("signer index")].private_key(),
                    &vote_preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let prepare_b = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject_b,
            execution_commitment: commitment_b,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate PrepareQC B"),
        };
        executor
            .enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(prepare_b.clone()),
            ))
            .expect("authenticate distinct PrepareQC B through production ingress");
        for _ in 0..8 {
            let _ = executor
                .step(started, &mut services)
                .expect("drive PrepareQC B reducer transition");
            if executor.pending_fetches.len() == 2 {
                break;
            }
        }
        assert_eq!(executor.pending_fetches.len(), 2);

        let timeout_now = started + Duration::from_secs(30);
        for _ in 0..8 {
            let _ = executor
                .step(timeout_now, &mut services)
                .expect("drive durable timeout transition");
            if !executor.pending_signatures.is_empty() {
                break;
            }
        }
        assert_eq!(services.cancelled_fetches, vec![proposal_a_work]);
        assert_eq!(executor.pending_signatures.len(), 1);
        assert_eq!(executor.pending_fetches.len(), 1);
        assert!(executor.pending_fetches.values().all(|pending| {
            pending.task.round == prepare_b.round && pending.task.subject == prepare_b.subject
        }));
        assert!(matches!(
            services.sign_tasks.last().map(|task| &task.request),
            Some(SignRequest::TimeoutVote(_))
        ));
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn full_capacity_certified_fetch_remains_missing_and_retransmit_later_adopts_it() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
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
            .expect("fill the only slot with Proposal A reconstruction");
        let proposal_a_task = services.fetch_tasks[0].clone();

        let (subject_b, _) = distinct_body(&fixture);
        let prepare_b = prepare_qc_for_subject(fixture.manifest.round, subject_b);
        let fetch_b = AdapterEffect::FetchBody {
            tag: tag(0),
            round: prepare_b.round,
            subject: prepare_b.subject,
            manifest: None,
            certified_sources: certified_sources(&fixture, &prepare_b),
            certificate: Some(prepare_b.clone()),
        };
        executor
            .consume_effects(vec![fetch_b.clone()], &mut services)
            .expect("full-capacity Fetch is deferred as reducer reconstruction debt");
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(
            !executor
                .body_pipeline_owners
                .contains_key(&(prepare_b.round, prepare_b.subject))
        );
        assert!(executor.retained_effect_batch.is_none());
        assert_eq!(executor.status().effect_dispatch_queue.depth, 0);
        assert!(!executor.status().fail_closed);

        executor
            .complete_body_reconstruction(
                &proposal_a_task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("Proposal A reconstruction terminates and releases capacity");
        executor
            .consume_effects(vec![fetch_b], &mut services)
            .expect("reducer retransmission re-emits certified Fetch B");
        assert_eq!(services.fetch_tasks.len(), 2);
        assert!(executor.pending_fetches.values().any(|pending| {
            pending.task.round == prepare_b.round && pending.task.subject == prepare_b.subject
        }));
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn durable_sign_preemption_orders_speculative_certified_and_locked_fetches() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(4, 8, 1 << 20, 8));
        let mut services = fixture.services();
        let speculative_old = manifest_for_payload(&fixture, b"oldest speculative fetch");
        let speculative_new = manifest_for_payload(&fixture, b"newer speculative fetch");
        let certified = manifest_for_payload(&fixture, b"certified non-lock fetch");
        let locked = manifest_for_payload(&fixture, b"durable locked fetch");

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: speculative_old.round,
                    subject: speculative_old.subject,
                    manifest: Some(speculative_old),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("start oldest speculative fetch");
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: speculative_new.round,
                    subject: speculative_new.subject,
                    manifest: Some(speculative_new),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("start newer speculative fetch");
        let certified_qc = prepare_qc_for_subject(certified.round, certified.subject);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: certified.round,
                    subject: certified.subject,
                    manifest: None,
                    certified_sources: certified_sources(&fixture, &certified_qc),
                    certificate: Some(certified_qc),
                }],
                &mut services,
            )
            .expect("start certified non-lock fetch");
        let locked_qc = prepare_qc_for_subject(locked.round, locked.subject);
        executor.protected_lock = Some((locked.round, locked.subject));
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: locked.round,
                    subject: locked.subject,
                    manifest: None,
                    certified_sources: certified_sources(&fixture, &locked_qc),
                    certificate: Some(locked_qc),
                }],
                &mut services,
            )
            .expect("start protected locked fetch");
        let fetch_ids = services
            .fetch_tasks
            .iter()
            .map(BodyFetchTask::id)
            .collect::<Vec<_>>();
        assert_eq!(executor.pending_work(), 4);

        for _ in 0..4 {
            executor
                .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
                .expect("each durable Sign owns one deterministically preempted slot");
        }
        assert_eq!(services.cancelled_fetches, fetch_ids);
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.pending_signatures.len(), 4);
        assert!(executor.retained_effect_batch.is_none());
        assert!(!executor.status().fail_closed);

        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1 << 20, 2));
        let mut services = fixture.services();
        let decided_qc = fixture.qc(wire::GlobalPhase::Prepare);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: decided_qc.round,
                    subject: decided_qc.subject,
                    manifest: None,
                    certified_sources: certified_sources(&fixture, &decided_qc),
                    certificate: Some(decided_qc.clone()),
                }],
                &mut services,
            )
            .expect("start the exact decided-body fetch fixture");
        executor.protected_decision = Some((decided_qc.round, decided_qc.subject));
        executor
            .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
            .expect("decided fetch is protected and Sign remains bounded debt");
        assert!(services.cancelled_fetches.is_empty());
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
        assert!(!executor.status().fail_closed);
        let decided_task = services.fetch_tasks[0].clone();
        executor
            .complete_body_reconstruction(
                &decided_task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("exact decided Fetch terminates normally");
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("retained Sign drains before another runtime transition"),
            EffectExecutorStep::Advanced { effects: 1 }
        );
        assert!(services.cancelled_fetches.is_empty());
        assert_eq!(executor.pending_fetches.len(), 0);
        assert_eq!(executor.pending_signatures.len(), 1);
        assert!(executor.retained_effect_batch.is_none());
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn retained_effect_batch_rejects_overtaking_and_oversize_before_partial_dispatch() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1 << 20, 2));
        let mut services = fixture.services();
        executor
            .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
            .expect("fill signing capacity");
        executor
            .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
            .expect("retain a second durable Sign");
        assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::Broadcast(proposal(&fixture))],
                &mut services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("overtook retained causal dispatch debt")
        ));
        assert!(services.broadcasts.is_empty());

        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let oversized = (0..=MAX_EFFECTS_PER_STEP)
            .map(|_| AdapterEffect::Broadcast(proposal(&fixture)))
            .collect::<Vec<_>>();
        assert!(matches!(
            executor.consume_effects(oversized, &mut services),
            Err(EffectExecutorError::Contract(reason)) if reason.contains("source bound")
        ));
        assert!(services.broadcasts.is_empty());
        assert!(executor.status().fail_closed);
    }

    #[test]
    fn retained_effect_tail_is_fifo_and_refilters_after_durable_decision() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
        let mut services = fixture.services();
        executor
            .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
            .expect("fill signing capacity");
        services.effect_service_order.clear();
        let message = proposal(&fixture);
        executor
            .consume_effects(
                vec![
                    AdapterEffect::Broadcast(message.clone()),
                    timeout_sign(&fixture, 0),
                    AdapterEffect::ReportEquivocation {
                        offender: fixture.context.roster[1].validator.clone(),
                        round: fixture.manifest.round,
                        kind: EquivocationKind::Vote,
                    },
                ],
                &mut services,
            )
            .expect("dispatch prefix and retain exact causal suffix");
        assert_eq!(services.effect_service_order, vec!["broadcast"]);
        assert_eq!(executor.status().effect_dispatch_queue.depth, 2);
        let first = services.sign_tasks[0].clone();
        let signature = Signature::new(
            fixture.validator_keys[0].private_key(),
            &first.request.signature_preimage(),
        )
        .payload()
        .to_vec();
        executor
            .complete_consensus_signature(first.id(), signature, &mut services)
            .expect("release retained FIFO head");
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("drain exact retained suffix"),
            EffectExecutorStep::Advanced { effects: 2 }
        );
        assert_eq!(
            services.effect_service_order,
            vec!["broadcast", "sign", "equivocation"]
        );

        let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
        let mut services = fixture.services();
        executor
            .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
            .expect("fill signing capacity before Decision");
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        let exact_commit = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(commit.clone()),
        );
        executor
            .consume_effects(
                vec![
                    timeout_sign(&fixture, 0),
                    AdapterEffect::Broadcast(proposal(&fixture)),
                    AdapterEffect::Broadcast(exact_commit.clone()),
                ],
                &mut services,
            )
            .expect("retain pre-Decision suffix");
        assert_eq!(executor.status().effect_dispatch_queue.depth, 3);
        executor.runtime.decided_body =
            Some((commit.round, commit.subject, commit.execution_commitment));
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("Decision refilters retained suffix before retry"),
            EffectExecutorStep::Advanced { effects: 1 }
        );
        assert_eq!(services.broadcasts, vec![exact_commit]);
        assert_eq!(services.sign_tasks.len(), 1);
        assert!(executor.retained_effect_batch.is_none());
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn pending_work_producer_inventory_is_exhaustive_and_source_linked() {
        let fixture = Fixture::new();
        let certificate = fixture.qc(wire::GlobalPhase::Commit);
        let cases = [
            (
                timeout_sign(&fixture, 0),
                Some(PendingWorkProducer::Sign),
                RestartEffectSource::DurableConsensusEvidence,
            ),
            (
                AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                },
                Some(PendingWorkProducer::Fetch),
                RestartEffectSource::BodyReconstruction,
            ),
            (
                AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                },
                Some(PendingWorkProducer::Store),
                RestartEffectSource::BodyReconstruction,
            ),
            (
                AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                },
                Some(PendingWorkProducer::Validate),
                RestartEffectSource::DurableBody,
            ),
            (
                AdapterEffect::Apply {
                    tag: tag(0),
                    subject: fixture.manifest.subject,
                    certificate: certificate.clone(),
                },
                Some(PendingWorkProducer::Apply),
                RestartEffectSource::DurableDecision,
            ),
            (
                AdapterEffect::Broadcast(proposal(&fixture)),
                None,
                RestartEffectSource::DurableConsensusEvidence,
            ),
            (
                AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_at_view(&fixture, 0),
                    protected_body: None,
                },
                None,
                RestartEffectSource::RecoveredView,
            ),
            (
                AdapterEffect::ReportEquivocation {
                    offender: fixture.context.roster[1].validator.clone(),
                    round: fixture.manifest.round,
                    kind: EquivocationKind::Vote,
                },
                None,
                RestartEffectSource::DiagnosticOnly,
            ),
            (
                AdapterEffect::ReportInvalidCertifiedBody {
                    subject: fixture.manifest.subject,
                    certificate,
                },
                None,
                RestartEffectSource::DiagnosticOnly,
            ),
        ];
        for (effect, expected_producer, expected_restart_source) in cases {
            assert_eq!(
                V2EffectExecutor::<FakeRuntime>::pending_work_producer(&effect),
                expected_producer
            );
            assert_eq!(
                V2EffectExecutor::<FakeRuntime>::restart_effect_source(&effect),
                expected_restart_source
            );
        }
    }

    #[test]
    fn retained_locked_body_reenters_current_view_store_and_validation_pipeline() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let manifest = manifest_at_view(&fixture, 3);
        let current_tag = tag(3);
        let key = (manifest.round, manifest.subject);

        executor
            .retain_locked_body_for_reproposal(
                current_tag,
                manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("stage exact locked bytes under the current round");
        assert_eq!(executor.ready_bodies[&key].manifest, manifest);
        assert!(services.fetch_tasks.is_empty());

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: current_tag,
                    round: manifest.round,
                    subject: manifest.subject,
                    manifest: Some(manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("authenticated proposal adopts the retained exact body");
        assert!(services.fetch_tasks.is_empty());
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(tag, completion_manifest))
                if *tag == current_tag && completion_manifest == &manifest
        ));

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: current_tag,
                    round: manifest.round,
                    subject: manifest.subject,
                }],
                &mut services,
            )
            .expect("current round requests durable storage");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("current-round body store completes");
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: current_tag,
                    round: manifest.round,
                    subject: manifest.subject,
                }],
                &mut services,
            )
            .expect("current round starts deterministic validation");
        let validation_id = services.validation_tasks[0].id();
        let validated = services.execute_validation(validation_id);
        executor
            .complete_body_validation(validated, &mut services)
            .expect("current-round validation completion is rebound to the follower");

        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::ValidationSucceeded(
                tag,
                completion_round,
                completion_subject,
                receipt,
            )) if *tag == current_tag
                && *completion_round == manifest.round
                && *completion_subject == manifest.subject
                && receipt.durable().round() == manifest.round
        ));
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn retained_locked_body_finishes_an_already_started_current_view_fetch() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let manifest = manifest_at_view(&fixture, 4);
        let current_tag = tag(4);

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: current_tag,
                    round: manifest.round,
                    subject: manifest.subject,
                    manifest: Some(manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("start ordinary current-view reconstruction");
        let fetch_id = services.fetch_tasks[0].id();
        assert_eq!(executor.pending_fetches.len(), 1);

        executor
            .retain_locked_body_for_reproposal(
                current_tag,
                manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("trusted locked bytes win the current-view acquisition race");

        assert!(services.cancelled_fetches.is_empty());
        assert_eq!(services.completed_reconstruction_fetches, vec![fetch_id]);
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.ready_bodies.len(), 1);
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(tag, completion_manifest))
                if *tag == current_tag && completion_manifest == &manifest
        ));
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn retained_locked_body_rebinds_after_a_later_view_proposal() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let original_manifest = manifest_at_view(&fixture, 3);
        let later_manifest = manifest_at_view(&fixture, 4);
        let later_tag = tag(4);

        executor
            .retain_locked_body_for_reproposal(
                tag(3),
                original_manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("retain exact locked bytes independently of their load view");
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: later_tag,
                    round: later_manifest.round,
                    subject: later_manifest.subject,
                    manifest: Some(later_manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("later authenticated proposal rebinds retained exact bytes");

        assert!(services.fetch_tasks.is_empty());
        assert!(
            executor
                .ready_bodies
                .contains_key(&(later_manifest.round, later_manifest.subject))
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(tag, completion_manifest))
                if *tag == later_tag && completion_manifest == &later_manifest
        ));
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn same_tag_higher_lock_retires_reproposal_round_ownership_before_staging() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let consumer = EventTag::new(1, 3, Generation::new(80));
        let first_lock = (round(&fixture.context, 0), fixture.manifest.subject);
        executor
            .reconcile_locked_body_for_reproposal(consumer, first_lock, &mut services)
            .expect("publish the initial exact lock rank");
        executor
            .retain_locked_body_for_reproposal(
                consumer,
                fixture.manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("stage the first lock under the current reproposal round");
        let staged = manifest_at_view(&fixture, consumer.view());
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer,
                    round: staged.round,
                    subject: staged.subject,
                    manifest: Some(staged.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("bind the staged cache to a queued reducer completion");
        assert_ne!(first_lock.0, staged.round);
        assert_eq!(executor.ready_bodies.len(), 1);
        assert_eq!(executor.body_pipeline_owners.len(), 1);
        assert_eq!(executor.runtime.completions.len(), 1);

        let (replacement_subject, replacement_body) = distinct_body(&fixture);
        let replacement = (round(&fixture.context, 1), replacement_subject);
        executor
            .reconcile_locked_body_for_reproposal(consumer, replacement, &mut services)
            .expect("same-tag higher lock retires every active owner of the old subject");
        assert_eq!(executor.protected_lock, Some(replacement));
        assert!(executor.retained_locked_body.is_none());
        assert!(executor.ready_bodies.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert!(executor.runtime.completions.is_empty());
        assert_eq!(executor.ready_body_bytes, 0);

        executor
            .retain_locked_body_for_reproposal(
                consumer,
                replacement_subject,
                replacement_body,
                &mut services,
            )
            .expect("the replacement lock claims the released bounded cache");
        assert_eq!(executor.ready_bodies.len(), 1);
        assert_eq!(
            executor
                .retained_locked_body
                .as_ref()
                .map(|(subject, _)| *subject),
            Some(replacement_subject)
        );
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn same_tag_higher_lock_retires_fetch_store_and_validation_owners() {
        let fixture = Fixture::new();
        let consumer = EventTag::new(1, 3, Generation::new(83));
        let first = (round(&fixture.context, 0), fixture.manifest.subject);
        let (replacement_subject, _) = distinct_body(&fixture);
        let higher = (round(&fixture.context, 1), replacement_subject);
        let staged = manifest_at_view(&fixture, consumer.view());

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .reconcile_locked_body_for_reproposal(consumer, first, &mut services)
                .expect("publish fetch-stage lock");
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                        manifest: Some(staged.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .expect("start reproposal-round fetch");
            let fetch_id = services.fetch_tasks[0].id();
            executor
                .reconcile_locked_body_for_reproposal(consumer, higher, &mut services)
                .expect("higher lock retires superseded fetch ownership");
            assert!(executor.pending_fetches.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
            assert_eq!(services.cancelled_fetches, vec![fetch_id]);
        }

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .reconcile_locked_body_for_reproposal(consumer, first, &mut services)
                .expect("publish store-stage lock");
            executor
                .retain_locked_body_for_reproposal(
                    consumer,
                    staged.subject,
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("stage exact bytes");
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                        manifest: Some(staged.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .expect("bind ready completion");
            executor.runtime.completions.clear();
            executor
                .consume_effects(
                    vec![AdapterEffect::StoreBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                    }],
                    &mut services,
                )
                .expect("start reproposal-round store");
            let store_id = services.store_tasks[0].id();
            assert_ne!(executor.pending_store_bytes, 0);
            executor
                .reconcile_locked_body_for_reproposal(consumer, higher, &mut services)
                .expect("higher lock retires superseded store ownership");
            assert!(executor.pending_stores.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
            assert_eq!(executor.pending_store_bytes, 0);
            assert_eq!(services.cancelled_stores, vec![store_id]);
        }

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .reconcile_locked_body_for_reproposal(consumer, first, &mut services)
                .expect("publish validation-stage lock");
            executor
                .retain_locked_body_for_reproposal(
                    consumer,
                    staged.subject,
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("stage exact bytes");
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                        manifest: Some(staged.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .expect("bind ready completion");
            executor.runtime.completions.clear();
            executor
                .consume_effects(
                    vec![AdapterEffect::StoreBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                    }],
                    &mut services,
                )
                .expect("start exact store");
            let store_id = services.store_tasks[0].id();
            let stored = services.execute_store(store_id);
            executor
                .complete_body_store(stored, &mut services)
                .expect("complete exact store");
            executor.runtime.completions.clear();
            executor
                .consume_effects(
                    vec![AdapterEffect::ValidateBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                    }],
                    &mut services,
                )
                .expect("start reproposal-round validation");
            let validation_id = services.validation_tasks[0].id();
            executor
                .reconcile_locked_body_for_reproposal(consumer, higher, &mut services)
                .expect("higher lock retires superseded validation ownership");
            assert!(executor.pending_validations.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
            assert_eq!(services.cancelled_validations, vec![validation_id]);
        }
    }

    #[test]
    fn first_lock_retires_unlocked_fetch_store_and_validation_owners() {
        let fixture = Fixture::new();
        let consumer = EventTag::new(1, 3, Generation::new(84));
        let staged = manifest_at_view(&fixture, consumer.view());
        let (replacement_subject, _) = distinct_body(&fixture);
        let first_lock = (
            round(&fixture.context, consumer.view()),
            replacement_subject,
        );

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                        manifest: Some(staged.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .expect("start unlocked candidate fetch");
            let fetch_id = services.fetch_tasks[0].id();

            executor
                .reconcile_locked_body_for_reproposal(consumer, first_lock, &mut services)
                .expect("first different lock retires the unlocked fetch");

            assert!(executor.pending_fetches.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
            assert_eq!(services.cancelled_fetches, vec![fetch_id]);
            assert_eq!(services.retired_outbound_subjects, vec![staged.subject]);
        }

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .admit_local_proposal(
                    consumer,
                    staged.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("start unlocked local-proposal store");
            let store_id = services.store_tasks[0].id();

            executor
                .reconcile_locked_body_for_reproposal(consumer, first_lock, &mut services)
                .expect("first different lock retires the unlocked store");

            assert!(executor.pending_stores.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
            assert_eq!(executor.pending_store_bytes, 0);
            assert_eq!(services.cancelled_stores, vec![store_id]);
        }

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .admit_local_proposal(
                    consumer,
                    staged.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("start unlocked local-proposal pipeline");
            let store_id = services.store_tasks[0].id();
            let stored = services.execute_store(store_id);
            executor
                .complete_body_store(stored, &mut services)
                .expect("advance unlocked candidate to validation");
            let validation_id = services.validation_tasks[0].id();

            executor
                .reconcile_locked_body_for_reproposal(consumer, first_lock, &mut services)
                .expect("first different lock retires the unlocked validation");

            assert!(executor.pending_validations.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
            assert_eq!(services.cancelled_validations, vec![validation_id]);
        }
    }

    #[test]
    fn first_lock_retires_queued_store_validation_and_local_proposal_completions() {
        let fixture = Fixture::new();
        let consumer = EventTag::new(1, 3, Generation::new(85));
        let staged = manifest_at_view(&fixture, consumer.view());
        let (replacement_subject, _) = distinct_body(&fixture);
        let first_lock = (
            round(&fixture.context, consumer.view()),
            replacement_subject,
        );

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .retain_locked_body_for_reproposal(
                    consumer,
                    staged.subject,
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("stage unlocked reducer bytes");
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                        manifest: Some(staged.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .expect("bind unlocked BodyAvailable completion");
            executor.runtime.completions.clear();
            executor
                .consume_effects(
                    vec![AdapterEffect::StoreBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                    }],
                    &mut services,
                )
                .expect("start unlocked reducer store");
            let store_id = services.store_tasks[0].id();
            let stored = services.execute_store(store_id);
            executor
                .complete_body_store(stored, &mut services)
                .expect("queue BodyStored before lock installation");
            assert!(matches!(
                executor.runtime.completions.as_slice(),
                [RuntimeCompletion::BodyStored(tag, round, subject, _)]
                    if *tag == consumer && *round == staged.round && *subject == staged.subject
            ));

            executor
                .reconcile_locked_body_for_reproposal(consumer, first_lock, &mut services)
                .expect("first lock retires queued BodyStored");
            assert!(executor.runtime.completions.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
        }

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .admit_local_proposal(
                    consumer,
                    staged.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("start unlocked local proposal");
            let store_id = services.store_tasks[0].id();
            let stored = services.execute_store(store_id);
            executor
                .complete_body_store(stored, &mut services)
                .expect("advance local proposal to validation");
            let validation_id = services.validation_tasks[0].id();
            let validated = services.execute_validation(validation_id);
            executor
                .complete_body_validation(validated, &mut services)
                .expect("queue LocalProposalReady before lock installation");
            assert!(matches!(
                executor.runtime.completions.as_slice(),
                [RuntimeCompletion::LocalProposal(tag, manifest, ..)]
                    if *tag == consumer && manifest == &staged
            ));

            executor
                .reconcile_locked_body_for_reproposal(consumer, first_lock, &mut services)
                .expect("first lock retires queued LocalProposalReady");
            assert!(executor.runtime.completions.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
        }

        {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            executor
                .retain_locked_body_for_reproposal(
                    consumer,
                    staged.subject,
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("stage unlocked reducer bytes for validation");
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                        manifest: Some(staged.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .expect("bind unlocked completion");
            executor.runtime.completions.clear();
            executor
                .consume_effects(
                    vec![AdapterEffect::StoreBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                    }],
                    &mut services,
                )
                .expect("start reducer store");
            let store_id = services.store_tasks[0].id();
            let stored = services.execute_store(store_id);
            executor
                .complete_body_store(stored, &mut services)
                .expect("record durable reducer body");
            executor.runtime.completions.clear();
            executor
                .consume_effects(
                    vec![AdapterEffect::ValidateBody {
                        tag: consumer,
                        round: staged.round,
                        subject: staged.subject,
                    }],
                    &mut services,
                )
                .expect("start reducer validation");
            let validation_id = services.validation_tasks[0].id();
            let validated = services.execute_validation(validation_id);
            executor
                .complete_body_validation(validated, &mut services)
                .expect("queue ValidationSucceeded before lock installation");
            assert!(matches!(
                executor.runtime.completions.as_slice(),
                [RuntimeCompletion::ValidationSucceeded(tag, round, subject, _)]
                    if *tag == consumer && *round == staged.round && *subject == staged.subject
            ));

            executor
                .reconcile_locked_body_for_reproposal(consumer, first_lock, &mut services)
                .expect("first lock retires queued ValidationSucceeded");
            assert!(executor.runtime.completions.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
        }
    }

    #[test]
    fn lock_reconciliation_rejects_same_round_conflict_and_late_lower_lock() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let consumer = EventTag::new(1, 3, Generation::new(81));
        let first = (round(&fixture.context, 0), fixture.manifest.subject);
        let (replacement_subject, _) = distinct_body(&fixture);
        executor
            .reconcile_locked_body_for_reproposal(consumer, first, &mut services)
            .expect("publish initial lock");

        assert!(matches!(
            executor.reconcile_locked_body_for_reproposal(
                consumer,
                (first.0, replacement_subject),
                &mut services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("strictly increase PrepareQC round")
        ));
        assert_eq!(executor.protected_lock, Some(first));
        assert!(!executor.output_guard.restart_required());
        assert!(services.closed.is_empty());

        let higher = (round(&fixture.context, 1), replacement_subject);
        executor
            .reconcile_locked_body_for_reproposal(consumer, higher, &mut services)
            .expect("publish strictly higher lock");
        assert!(matches!(
            executor.reconcile_locked_body_for_reproposal(consumer, first, &mut services),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("strictly increase PrepareQC round")
        ));
        assert_eq!(executor.protected_lock, Some(higher));
        assert!(!executor.output_guard.restart_required());
        assert!(services.closed.is_empty());
    }

    #[test]
    fn higher_round_same_subject_preserves_current_proposal_pipeline_with_same_tag() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let consumer = EventTag::new(1, 3, Generation::new(82));
        let body_len = u64::try_from(fixture.body.len()).expect("body length");
        let first = (round(&fixture.context, 0), fixture.manifest.subject);
        executor
            .reconcile_locked_body_for_reproposal(consumer, first, &mut services)
            .expect("publish initial same-subject lock");
        executor
            .retain_locked_body_for_reproposal(
                consumer,
                fixture.manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("retain and stage initial bytes");
        let staged = manifest_at_view(&fixture, consumer.view());
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer,
                    round: staged.round,
                    subject: staged.subject,
                    manifest: Some(staged.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("queue the old round-bound completion");
        assert_eq!(executor.ready_body_bytes, body_len * 2);

        let higher = (round(&fixture.context, 1), fixture.manifest.subject);
        executor
            .reconcile_locked_body_for_reproposal(consumer, higher, &mut services)
            .expect("higher same-subject lock preserves valid current-round ownership");
        assert_eq!(executor.protected_lock, Some(higher));
        assert_eq!(executor.ready_bodies.len(), 1);
        assert_eq!(executor.body_pipeline_owners.len(), 1);
        assert_eq!(executor.runtime.completions.len(), 1);
        assert!(executor.retained_locked_body.is_some());
        assert_eq!(executor.ready_body_bytes, body_len * 2);

        executor
            .retain_locked_body_for_reproposal(
                consumer,
                fixture.manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("the current-round stage is already owned idempotently");
        executor
            .reconcile_locked_body_for_reproposal(consumer, higher, &mut services)
            .expect("exact lock reconciliation is idempotent");
        executor
            .retain_locked_body_for_reproposal(
                consumer,
                fixture.manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("exact cache restaging is idempotent");
        assert_eq!(executor.ready_bodies.len(), 1);
        assert_eq!(executor.ready_body_bytes, body_len * 2);
        assert!(!executor.status().fail_closed);

        // Exact lock repetition used to return before global byte accounting
        // was checked. Exercise the direct reproposal entrypoint with both
        // low and inflated counters: neither corruption may hide behind the
        // idempotent lock fast path or mutate an exact owner.
        for corruption in ["low", "high"] {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let consumer = EventTag::new(1, 3, Generation::new(82));
            let exact_lock = (round(&fixture.context, 0), fixture.manifest.subject);
            executor
                .reconcile_locked_body_for_reproposal(consumer, exact_lock, &mut services)
                .expect("publish the exact lock before staging bytes");
            executor
                .retain_locked_body_for_reproposal(
                    consumer,
                    fixture.manifest.subject,
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("stage one exact retained owner");
            executor.ready_body_bytes = match corruption {
                "low" => 0,
                "high" => executor
                    .ready_body_bytes
                    .checked_add(1)
                    .expect("small test counter"),
                _ => unreachable!("the test enumerates low and high corruption"),
            };
            let before = executor.body_ownership_projection();

            assert!(matches!(
                executor.reconcile_locked_body_for_reproposal(
                    consumer,
                    exact_lock,
                    &mut services,
                ),
                Err(EffectExecutorError::Contract(reason))
                    if reason.contains("body byte accounting")
            ));
            assert_eq!(executor.body_ownership_projection(), before);
            assert_eq!(executor.protected_lock, Some(exact_lock));
            assert!(services.cancelled_fetches.is_empty());
            assert!(services.cancelled_stores.is_empty());
            assert!(services.cancelled_validations.is_empty());
        }
    }

    #[test]
    fn retained_locked_body_survives_same_lock_view_churn_before_fetch_adopts_it() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let protected = (fixture.manifest.round, fixture.manifest.subject);
        let body_len = u64::try_from(fixture.body.len()).expect("body length");

        executor
            .retain_locked_body_for_reproposal(
                EventTag::new(1, 0, Generation::new(40)),
                fixture.manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("stage one view-independent locked-body cache");
        assert_eq!(executor.ready_body_bytes, body_len * 2);
        assert!(executor.body_pipeline_owners.is_empty());

        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, 1, Generation::new(41)),
                    certificate: timeout_at_view(&fixture, 0),
                    protected_body: Some(protected),
                }],
                &mut services,
            )
            .expect("an omitted TC high preserves the effective local lock cache");
        assert_eq!(executor.ready_body_bytes, body_len * 2);
        assert!(executor.retained_locked_body.is_some());
        assert_eq!(executor.ready_bodies.len(), 1);

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: EventTag::new(1, 1, Generation::new(41)),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("the new view adopts staged bytes without starting network work");
        assert!(services.fetch_tasks.is_empty());
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
                if *completion_tag == EventTag::new(1, 1, Generation::new(41))
                    && manifest == &fixture.manifest
        ));

        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, 2, Generation::new(42)),
                    certificate: timeout_at_view(&fixture, 1),
                    protected_body: Some(protected),
                }],
                &mut services,
            )
            .expect("the queued completion rebinds on repeated same-lock churn");
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
                if *completion_tag == EventTag::new(1, 2, Generation::new(42))
                    && manifest == &fixture.manifest
        ));
        assert_eq!(executor.ready_body_bytes, body_len * 2);
        assert!(executor.retained_locked_body.is_some());
    }

    #[test]
    fn higher_different_lock_releases_retained_cache_before_replacement_staging() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .retain_locked_body_for_reproposal(
                EventTag::new(1, 0, Generation::new(50)),
                fixture.manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("retain the original lock cache");

        let replacement_header = BlockHeader::new(
            NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            2_000,
            0,
        );
        let replacement_signature = SignatureOf::try_from_hash(
            fixture.validator_keys[0].private_key(),
            replacement_header.hash(),
        )
        .expect("replacement block signature");
        let replacement_block = SignedBlock::presigned(
            BlockSignature::new(0, replacement_signature),
            replacement_header,
            Vec::new(),
        );
        let replacement_body = replacement_block
            .encode_wire()
            .expect("replacement canonical body");
        let replacement_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: replacement_block.hash(),
            payload_hash: Hash::new(&replacement_body),
        };
        let replacement_round = round(&fixture.context, 1);
        let mut replacement = fixture.qc(wire::GlobalPhase::Prepare);
        replacement.round = replacement_round;
        replacement.subject = replacement_subject;
        let mut timeout = timeout_at_view(&fixture, 1);
        timeout.groups[0].highest_prepare_qc = Some(replacement);
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, 2, Generation::new(52)),
                    certificate: timeout,
                    protected_body: Some((replacement_round, replacement_subject)),
                }],
                &mut services,
            )
            .expect("the higher different lock retires the old subject cache");
        assert!(executor.retained_locked_body.is_none());
        assert!(executor.ready_bodies.is_empty());
        assert_eq!(executor.ready_body_bytes, 0);

        executor
            .retain_locked_body_for_reproposal(
                EventTag::new(1, 2, Generation::new(52)),
                replacement_subject,
                replacement_body.clone(),
                &mut services,
            )
            .expect("replacement lock can claim all released cache capacity");
        assert_eq!(
            executor
                .retained_locked_body
                .as_ref()
                .map(|(subject, _)| *subject),
            Some(replacement_subject)
        );
        assert_eq!(
            executor.ready_body_bytes,
            u64::try_from(replacement_body.len()).expect("replacement body length") * 2
        );
    }

    #[test]
    fn higher_round_same_subject_reuses_only_the_view_independent_locked_cache() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let body_len = u64::try_from(fixture.body.len()).expect("body length");
        let original_tag = EventTag::new(1, 0, Generation::new(70));

        executor
            .retain_locked_body_for_reproposal(
                original_tag,
                fixture.manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("retain and stage the original-round locked body");
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: original_tag,
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("queue the original-round BodyAvailable completion");
        assert_eq!(executor.ready_body_bytes, body_len * 2);
        assert_eq!(executor.runtime.completions.len(), 1);

        let replacement_manifest = manifest_at_view(&fixture, 1);
        let mut replacement = fixture.qc(wire::GlobalPhase::Prepare);
        replacement.round = replacement_manifest.round;
        replacement.subject = replacement_manifest.subject;
        let mut timeout = timeout_at_view(&fixture, 1);
        timeout.groups[0].highest_prepare_qc = Some(replacement.clone());
        let replacement_tag = EventTag::new(1, 2, Generation::new(72));
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: replacement_tag,
                    certificate: timeout,
                    protected_body: Some((replacement.round, replacement.subject)),
                }],
                &mut services,
            )
            .expect("the higher round retires only the old round-bound stage");

        assert!(executor.ready_bodies.is_empty());
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert!(executor.retained_locked_body.is_some());
        assert_eq!(executor.ready_body_bytes, body_len);

        let sources = replacement
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: replacement_tag,
                    round: replacement.round,
                    subject: replacement.subject,
                    manifest: Some(replacement_manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(replacement),
                }],
                &mut services,
            )
            .expect("the new round remints its stage from the subject cache");
        assert_eq!(executor.ready_body_bytes, body_len * 2);
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
                if *completion_tag == replacement_tag && manifest == &replacement_manifest
        ));
        assert!(services.fetch_tasks.is_empty());
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
    fn failed_lock_cleanup_keeps_exact_owner_and_requires_restart() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let certified_sources = prepare
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
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("admit superseded body recovery");
        let before = executor.body_ownership_projection();
        let (replacement_subject, _) = distinct_body(&fixture);
        services.fail_on = Some("cancel-fetch");

        assert!(matches!(
            executor.reconcile_locked_body_for_reproposal(
                tag(1),
                (round(&fixture.context, 0), replacement_subject),
                &mut services,
            ),
            Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.protected_lock, None);
        assert!(executor.output_guard.restart_required());
        assert!(executor.status().fail_closed);
        assert_eq!(services.closed.len(), 1);
        assert!(matches!(
            executor.reconcile_locked_body_for_reproposal(
                tag(1),
                (round(&fixture.context, 0), replacement_subject),
                &mut services,
            ),
            Err(EffectExecutorError::FailClosed(_))
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn lock_cleanup_rejects_inconsistent_certified_request_before_mutation() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let certified_sources = prepare
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
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("admit certified body recovery");
        let request_hash = *executor
            .certified_work
            .keys()
            .next()
            .expect("certified request index");
        assert!(executor.outstanding_requests.cancel(request_hash));
        let before = executor.body_ownership_projection();
        let (replacement_subject, _) = distinct_body(&fixture);

        assert!(matches!(
            executor.reconcile_locked_body_for_reproposal(
                tag(1),
                (round(&fixture.context, 0), replacement_subject),
                &mut services,
            ),
            Err(EffectExecutorError::Contract(_))
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.cancelled_fetches.is_empty());
        assert_eq!(executor.protected_lock, None);
        assert!(executor.output_guard.restart_required());
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn lock_cleanup_status_failure_preserves_committed_replacement() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let certified_sources = prepare
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
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("admit superseded certified recovery");
        let old_work_id = services.fetch_tasks[0].id();
        let (replacement_subject, _) = distinct_body(&fixture);
        let replacement = (round(&fixture.context, 0), replacement_subject);
        services.fail_on = Some("status");

        assert!(matches!(
            executor.reconcile_locked_body_for_reproposal(
                tag(1),
                replacement,
                &mut services,
            ),
            Err(EffectExecutorError::Service(reason)) if reason.contains("status failed")
        ));
        assert_eq!(executor.protected_lock, Some(replacement));
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.cancelled_fetches, vec![old_work_id]);
        assert!(executor.output_guard.restart_required());
        assert_eq!(services.closed.len(), 1);
        assert!(matches!(
            executor.reconcile_locked_body_for_reproposal(tag(1), replacement, &mut services,),
            Err(EffectExecutorError::FailClosed(_))
        ));
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn missing_merge_sidecar_retains_exact_validation_until_retry() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
        let round = pending.task.round();
        let subject = pending.task.subject();
        let task = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            round,
            subject,
        );
        let work_id = task.id();
        let durable = task.durable_receipt().clone();

        let completion = BodyValidationCompletion::DeferredMergeSidecar {
            work_id,
            reference: reference.clone(),
        };
        assert_eq!(
            executor
                .complete_body_validation(completion.clone(), &mut services)
                .expect("defer validation for exact merge sidecar"),
            CompletionDisposition::Deferred
        );
        assert_eq!(executor.pending_validations.len(), 1);
        let status = executor.status();
        assert_eq!(status.deferred_merge_work, 1);
        assert_eq!(status.deferred_validation_merge_work, 1);
        assert_eq!(status.deferred_application_merge_work, 0);
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
        let status = executor.status();
        assert_eq!(status.deferred_merge_work, 1);
        assert_eq!(status.deferred_validation_merge_work, 1);
        assert_eq!(status.deferred_application_merge_work, 0);
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
                        receipt: ValidatedBodyReceipt::for_test(durable),
                    },
                    &mut services,
                )
                .expect("complete exact retried validation"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_validations.is_empty());
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::ValidationSucceeded(
                completion_tag,
                completion_round,
                completion_subject,
                _
            )) if *completion_tag == tag(3)
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
        let round = pending.task.round();
        let subject = pending.task.subject();
        let work_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            round,
            subject,
        )
        .id();
        executor
            .complete_body_validation(
                BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
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
            )) if *completion_tag == tag(3)
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
        let first_round = first.task.round();
        let first_subject = first.task.subject();
        let second_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"conflicting second carrier")),
            ..first_subject
        };
        let mut second_reference = first_reference.clone();
        second_reference.encoded_len += 1;
        let retry_reference = first_reference.clone();
        let first_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            first_round,
            first_subject,
        )
        .id();
        let second_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            first_round,
            second_subject,
        )
        .id();

        for (work_id, reference) in [(first_id, first_reference), (second_id, second_reference)] {
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
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
            CompletionDisposition::Accepted
        );
        assert!(!executor.pending_validations.contains_key(&second_id));
        assert!(executor.pending_validations.contains_key(&first_id));
        assert_eq!(
            executor.deferred_merge_work.get(&first_id),
            Some(&entry_hash)
        );
        let status = executor.status();
        assert_eq!(status.deferred_merge_work, 1);
        assert_eq!(status.deferred_validation_merge_work, 1);
        assert_eq!(status.deferred_application_merge_work, 0);

        // A multi-waiter retry is transactional with respect to executor
        // ownership. The first external enqueue may acknowledge before the
        // second fails, but no deferred entry or pending task is committed
        // away until every callback succeeds.
        let third_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"retry third carrier")),
            ..first_subject
        };
        let third_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            first_round,
            third_subject,
        )
        .id();
        executor
            .complete_body_validation(
                BodyValidationCompletion::DeferredMergeSidecar {
                    work_id: third_id,
                    reference: retry_reference,
                },
                &mut services,
            )
            .expect("retain a second reachable retry waiter");
        let before = executor.body_ownership_projection();
        let validation_tasks_before = services.validation_tasks.len();
        let validation_calls = services
            .operation_calls
            .get("validation")
            .copied()
            .expect("production validation admissions were counted");
        services.fail_on_call = Some(("validation", validation_calls + 2));

        assert!(matches!(
            executor.retry_deferred_merge_sidecar(entry_hash, &mut services),
            Err(EffectExecutorError::Service(reason))
                if reason.contains("validation call")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.validation_tasks.len(), validation_tasks_before + 1);
        assert!(executor.deferred_merge_work.contains_key(&first_id));
        assert!(executor.deferred_merge_work.contains_key(&third_id));
        assert!(executor.status().fail_closed);

        // Terminal rejection preflights the complete matching set before the
        // first waiter is completed. A corrupt later owner therefore cannot
        // allow an earlier validation rejection or ownership removal.
        {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
            let round = pending.task.round();
            let first_subject = pending.task.subject();
            let second_subject = wire::BlockSubject {
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"corrupt later waiter")),
                ..first_subject
            };
            let first_id = begin_reachable_merge_validation(
                &fixture,
                &mut executor,
                &mut services,
                round,
                first_subject,
            )
            .id();
            let second_id = begin_reachable_merge_validation(
                &fixture,
                &mut executor,
                &mut services,
                round,
                second_subject,
            )
            .id();
            for work_id in [first_id, second_id] {
                executor
                    .complete_body_validation(
                        BodyValidationCompletion::DeferredMergeSidecar {
                            work_id,
                            reference: reference.clone(),
                        },
                        &mut services,
                    )
                    .expect("retain each reachable rejection waiter");
            }
            executor
                .body_pipeline_owners
                .get_mut(&(round, second_subject))
                .expect("second exact validation owner")
                .tag = EventTag::new(1, round.view, Generation::new(8));
            let before = executor.body_ownership_projection();

            assert!(matches!(
                executor.reject_deferred_merge_sidecar(
                    entry_hash,
                    "invalid shared merge entry",
                    &mut services,
                ),
                Err(EffectExecutorError::Contract(reason))
                    if reason.contains("immutable pipeline owner")
            ));
            assert_eq!(executor.body_ownership_projection(), before);
            assert!(services.rejected_validations.is_empty());
            assert!(executor.runtime.completions.is_empty());
        }

        // Runtime failure is one atomic batch failure, so a later admission
        // cannot leave an earlier ValidationFailed completion visible while
        // every executor waiter is still retained.
        {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
            let round = pending.task.round();
            let first_subject = pending.task.subject();
            let second_subject = wire::BlockSubject {
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime second waiter")),
                ..first_subject
            };
            let first_id = begin_reachable_merge_validation(
                &fixture,
                &mut executor,
                &mut services,
                round,
                first_subject,
            )
            .id();
            let second_id = begin_reachable_merge_validation(
                &fixture,
                &mut executor,
                &mut services,
                round,
                second_subject,
            )
            .id();
            for work_id in [first_id, second_id] {
                executor
                    .complete_body_validation(
                        BodyValidationCompletion::DeferredMergeSidecar {
                            work_id,
                            reference: reference.clone(),
                        },
                        &mut services,
                    )
                    .expect("retain each atomic rejection waiter");
            }
            let before = executor.body_ownership_projection();
            executor.runtime.fail_enqueue = true;

            assert!(
                executor
                    .reject_deferred_merge_sidecar(
                        entry_hash,
                        "invalid shared merge entry",
                        &mut services,
                    )
                    .is_err()
            );
            assert_eq!(executor.body_ownership_projection(), before);
            assert_eq!(executor.runtime.fail_enqueue_hits, 1);
            assert!(services.rejected_validations.is_empty());
        }
    }

    #[test]
    fn decided_apply_retries_after_exact_merge_sidecar_recovery() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending_validation, reference, entry_hash) = pending_merge_validation(&fixture);
        let mut certificate = fixture.qc(wire::GlobalPhase::Commit);
        certificate.round = pending_validation.task.round();
        certificate.subject = pending_validation.task.subject();
        let validated_receipt =
            ValidatedBodyReceipt::for_test(pending_validation.task.durable_receipt().clone());
        certificate.execution_commitment = validated_receipt.execution_commitment();
        executor.durable_bodies.insert(
            (certificate.round, certificate.subject),
            validated_receipt.durable().clone(),
        );
        executor
            .validated_bodies
            .insert((certificate.round, certificate.subject), validated_receipt);
        executor
            .begin_apply(tag(3), certificate.subject, certificate, &mut services)
            .expect("start Apply through the production admission path");
        let task = services.apply_tasks.pop().expect("production Apply task");
        let work_id = task.id();

        assert_eq!(
            executor
                .defer_application_for_merge_sidecar(work_id, &reference, &mut services)
                .expect("defer decided apply"),
            CompletionDisposition::Deferred
        );
        let status = executor.status();
        assert_eq!(status.deferred_merge_work, 1);
        assert_eq!(status.deferred_validation_merge_work, 0);
        assert_eq!(status.deferred_application_merge_work, 1);
        assert!(services.apply_tasks.is_empty());
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

        // Application deferral is also an ownership boundary. An internally
        // inconsistent decided task must fail before sidecar registration or
        // a recovery callback can treat it as legitimate pending work.
        executor
            .pending_applications
            .get_mut(&work_id)
            .expect("retained exact Apply owner")
            .task
            .certificate
            .subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"corrupt deferred apply")),
            ..task.subject
        };
        let deferred_callbacks = services.deferred_merge_sidecars.len();

        assert!(matches!(
            executor.defer_application_for_merge_sidecar(work_id, &reference, &mut services,),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("exact decided-body owner")
        ));
        assert!(executor.pending_applications.contains_key(&work_id));
        assert!(executor.deferred_merge_work.is_empty());
        assert_eq!(services.deferred_merge_sidecars.len(), deferred_callbacks);
    }

    #[test]
    fn deferred_merge_sidecar_must_match_carrier_height_parent_and_round_ceiling() {
        for mismatch in 0..3 {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let (pending, mut reference, _) = pending_merge_validation(&fixture);
            let round = pending.task.round();
            let subject = pending.task.subject();
            let work_id = begin_reachable_merge_validation(
                &fixture,
                &mut executor,
                &mut services,
                round,
                subject,
            )
            .id();
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
                    reference.merge_qc.view = round.view.saturating_add(1);
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                executor.complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
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
            let durable = pending.task.durable_receipt().clone();
            let manifest = wire::PayloadManifest::derive(
                &fixture.context,
                round,
                subject,
                u64::try_from(fixture.body.len()).expect("body length"),
                std::slice::from_ref(&fixture.body),
            )
            .expect("protected manifest");
            executor
                .recovered_bodies
                .insert((round, subject), (manifest, durable.clone()));
            executor.durable_bodies.insert((round, subject), durable);
            executor.body_pipeline_owners.insert(
                (round, subject),
                BodyPipelineOwner {
                    tag: tag(round.view),
                    manifest_hash: Some(pending.task.durable_receipt().manifest_hash()),
                },
            );
            executor.pending_validations.insert(work_id, pending);
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
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
            let protected_body = protected.then_some((round, subject));
            executor
                .install_view(tag(round.view + 1), timeout, protected_body, &mut services)
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
            assert!(
                !executor
                    .body_pipeline_owners
                    .contains_key(&(round, subject))
            );
            if protected {
                assert!(executor.pending_validations[&work_id].consumer.is_none());
            }
            assert!(
                services.cancelled_validations.is_empty(),
                "a completed sidecar-deferred validation has no live I/O owner to cancel"
            );
        }
    }

    #[test]
    fn certified_view_protects_only_the_exact_high_qc_round_for_one_subject() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (first, reference, entry_hash) = pending_merge_validation(&fixture);
        let subject = first.task.subject();
        let first_round = first.task.round();
        let second_round = round(&fixture.context, first_round.view + 1);
        let first_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            first_round,
            subject,
        )
        .id();
        let second_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            second_round,
            subject,
        )
        .id();

        for work_id in [first_id, second_id] {
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id,
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
            .install_view(
                tag(second_round.view + 1),
                timeout,
                Some((second_round, subject)),
                &mut services,
            )
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
        assert!(executor.pending_validations[&second_id].consumer.is_none());
    }

    #[test]
    fn protected_deferred_validation_rebinds_across_view_churn_before_retry() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
        let work_id = pending.task.id();
        let round = pending.task.round();
        let subject = pending.task.subject();
        let durable = pending.task.durable_receipt().clone();
        let manifest = wire::PayloadManifest::derive(
            &fixture.context,
            round,
            subject,
            u64::try_from(fixture.body.len()).expect("body length"),
            std::slice::from_ref(&fixture.body),
        )
        .expect("protected manifest");
        executor
            .recovered_bodies
            .insert((round, subject), (manifest.clone(), durable.clone()));
        executor
            .durable_bodies
            .insert((round, subject), durable.clone());
        executor.body_pipeline_owners.insert(
            (round, subject),
            BodyPipelineOwner {
                tag: tag(round.view),
                manifest_hash: Some(durable.manifest_hash()),
            },
        );
        executor.pending_validations.insert(work_id, pending);
        assert_eq!(
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
                    &mut services,
                )
                .expect("defer protected validation"),
            CompletionDisposition::Deferred
        );

        let mut high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        high_prepare.round = round;
        high_prepare.subject = subject;
        let sources = high_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();

        for entering_view in [round.view + 1, round.view + 2] {
            let mut timeout = timeout_at_view(&fixture, entering_view - 1);
            timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
            executor
                .consume_effects(
                    vec![
                        AdapterEffect::EnterView {
                            tag: tag(entering_view),
                            certificate: timeout,
                            protected_body: Some((round, subject)),
                        },
                        AdapterEffect::FetchBody {
                            tag: tag(entering_view),
                            round,
                            subject,
                            manifest: Some(manifest.clone()),
                            certified_sources: sources.clone(),
                            certificate: Some(high_prepare.clone()),
                        },
                    ],
                    &mut services,
                )
                .expect("replay protected body in current view");
            assert!(executor.pending_validations[&work_id].consumer.is_none());

            executor
                .consume_effects(
                    vec![
                        AdapterEffect::StoreBody {
                            tag: tag(entering_view),
                            round,
                            subject,
                        },
                        AdapterEffect::ValidateBody {
                            tag: tag(entering_view),
                            round,
                            subject,
                        },
                    ],
                    &mut services,
                )
                .expect("adopt deferred validation in current view");
            assert_eq!(
                executor.pending_validations[&work_id].consumer,
                Some(ValidationConsumer::Reducer {
                    tag: tag(entering_view)
                })
            );
            assert_eq!(
                executor.deferred_merge_work.get(&work_id),
                Some(&entry_hash)
            );
        }

        assert_eq!(
            executor
                .retry_deferred_merge_sidecar(entry_hash, &mut services)
                .expect("retry protected validation after sidecar recovery"),
            1
        );
        assert_eq!(services.validation_tasks.len(), 1);
        assert_eq!(services.validation_tasks[0].id(), work_id);
        assert_eq!(
            executor
                .complete_body_validation(
                    BodyValidationCompletion::Validated {
                        work_id,
                        receipt: ValidatedBodyReceipt::for_test(durable),
                    },
                    &mut services,
                )
                .expect("route retried validation to latest consumer"),
            CompletionDisposition::Accepted
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::ValidationSucceeded(
                completion_tag,
                completion_round,
                completion_subject,
                _
            )) if *completion_tag == tag(round.view + 2)
                && *completion_round == round
                && *completion_subject == subject
        ));
        assert!(executor.pending_validations.is_empty());
        assert!(executor.deferred_merge_work.is_empty());
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn certified_view_rebinds_inflight_high_qc_validation_through_current_fifo() {
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
            .expect("admit old-view body");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("start old-view validation");
        let work_id = services.validation_tasks[0].id();

        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        let mut timeout = timeout_certificate(&fixture);
        timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
        executor
            .consume_effects(
                vec![
                    AdapterEffect::EnterView {
                        tag: tag(1),
                        certificate: timeout,
                        protected_body: Some((fixture.manifest.round, fixture.manifest.subject)),
                    },
                    AdapterEffect::FetchBody {
                        tag: tag(1),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: sources,
                        certificate: Some(prepare),
                    },
                ],
                &mut services,
            )
            .expect("install view and replay locked-body acquisition");

        assert_eq!(executor.pending_validations.len(), 1);
        assert!(executor.pending_validations[&work_id].consumer.is_none());
        assert_eq!(
            executor
                .body_pipeline_owners
                .get(&(fixture.manifest.round, fixture.manifest.subject))
                .map(|owner| owner.tag),
            Some(tag(1))
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(1) && manifest == &fixture.manifest
        ));

        executor
            .consume_effects(
                vec![
                    AdapterEffect::StoreBody {
                        tag: tag(1),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    },
                    AdapterEffect::ValidateBody {
                        tag: tag(1),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    },
                ],
                &mut services,
            )
            .expect("current view adopts retained immutable validation");
        assert_eq!(
            executor.pending_validations[&work_id].consumer,
            Some(ValidationConsumer::Reducer { tag: tag(1) })
        );
        assert_eq!(services.validation_tasks.len(), 2);
        assert!(
            services
                .validation_tasks
                .iter()
                .all(|task| task.id() == work_id)
        );

        let completed = services.execute_validation(work_id);
        assert_eq!(
            executor
                .complete_body_validation(completed, &mut services)
                .expect("route retained completion to current consumer"),
            CompletionDisposition::Accepted
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::ValidationSucceeded(
                completion_tag,
                completion_round,
                completion_subject,
                _
            )) if *completion_tag == tag(1)
                && *completion_round == fixture.manifest.round
                && *completion_subject == fixture.manifest.subject
        ));
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn detached_validation_outcomes_replay_only_after_current_consumer_attaches() {
        for reject in [false, true] {
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
                .expect("admit old-view body");
            let store_id = services.store_tasks[0].id();
            let stored = services.execute_store(store_id);
            executor
                .complete_body_store(stored, &mut services)
                .expect("start old-view validation");
            let work_id = services.validation_tasks[0].id();

            let prepare = fixture.qc(wire::GlobalPhase::Prepare);
            let mut timeout = timeout_certificate(&fixture);
            timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: tag(1),
                        certificate: timeout,
                        protected_body: Some((fixture.manifest.round, fixture.manifest.subject)),
                    }],
                    &mut services,
                )
                .expect("detach protected validation");
            assert!(executor.pending_validations[&work_id].consumer.is_none());

            executor.runtime.completions.clear();
            if reject {
                services.validation_error = Some("detached rejection".to_owned());
            }
            let completion = services.execute_validation(work_id);
            assert_eq!(
                executor
                    .complete_body_validation(completion, &mut services)
                    .expect("cache detached terminal outcome"),
                CompletionDisposition::Accepted
            );
            assert!(executor.runtime.completions.is_empty());
            assert!(executor.pending_validations.is_empty());

            let sources = prepare
                .signers
                .iter()
                .map(|index| fixture.context.roster[*index as usize].validator.clone())
                .collect();
            executor
                .consume_effects(
                    vec![
                        AdapterEffect::FetchBody {
                            tag: tag(1),
                            round: fixture.manifest.round,
                            subject: fixture.manifest.subject,
                            manifest: Some(fixture.manifest.clone()),
                            certified_sources: sources,
                            certificate: Some(prepare),
                        },
                        AdapterEffect::StoreBody {
                            tag: tag(1),
                            round: fixture.manifest.round,
                            subject: fixture.manifest.subject,
                        },
                        AdapterEffect::ValidateBody {
                            tag: tag(1),
                            round: fixture.manifest.round,
                            subject: fixture.manifest.subject,
                        },
                    ],
                    &mut services,
                )
                .expect("replay cached outcome through current FIFO");
            if reject {
                assert!(matches!(
                    executor.runtime.completions.last(),
                    Some(RuntimeCompletion::ValidationFailed(
                        completion_tag,
                        completion_round,
                        completion_subject
                    )) if *completion_tag == tag(1)
                        && *completion_round == fixture.manifest.round
                        && *completion_subject == fixture.manifest.subject
                ));
                assert!(
                    executor
                        .rejected_bodies
                        .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
                );
            } else {
                assert!(matches!(
                    executor.runtime.completions.last(),
                    Some(RuntimeCompletion::ValidationSucceeded(
                        completion_tag,
                        completion_round,
                        completion_subject,
                        _
                    )) if *completion_tag == tag(1)
                        && *completion_round == fixture.manifest.round
                        && *completion_subject == fixture.manifest.subject
                ));
            }
            assert_eq!(services.validation_tasks.len(), 1);
            assert!(!executor.status().fail_closed);
        }
    }

    #[test]
    fn contradictory_terminal_validation_catalogues_fail_closed() {
        for conflicting_receipt in [false, true] {
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
                .expect("start exact-body pipeline");
            let store_id = services.store_tasks[0].id();
            let stored = services.execute_store(store_id);
            executor
                .complete_body_store(stored, &mut services)
                .expect("start deterministic validation");
            let work_id = services.validation_tasks[0].id();
            let durable = executor.pending_validations[&work_id]
                .task
                .durable_receipt()
                .clone();

            let error = if conflicting_receipt {
                let first = services.execute_validation(work_id);
                let first_receipt = first
                    .validated_receipt()
                    .expect("validated completion")
                    .clone();
                executor
                    .complete_body_validation(first, &mut services)
                    .expect("record first validation receipt");
                let conflicting = ValidatedBodyReceipt::for_test(durable);
                assert_ne!(conflicting, first_receipt);
                executor
                    .complete_body_validation(
                        BodyValidationCompletion::Validated {
                            work_id,
                            receipt: conflicting,
                        },
                        &mut services,
                    )
                    .expect_err("conflicting validation receipts must fail closed")
            } else {
                executor
                    .complete_body_validation(
                        BodyValidationCompletion::Rejected {
                            work_id,
                            reason: "deterministic rejection".to_owned(),
                        },
                        &mut services,
                    )
                    .expect("record deterministic rejection");
                executor
                    .complete_body_validation(
                        BodyValidationCompletion::Validated {
                            work_id,
                            receipt: ValidatedBodyReceipt::for_test(durable),
                        },
                        &mut services,
                    )
                    .expect_err("validated and rejected outcomes must fail closed")
            };

            assert!(matches!(
                error,
                EffectExecutorError::Contract(_) | EffectExecutorError::BodyStore(_)
            ));
            assert!(executor.status().fail_closed);
            assert_eq!(services.closed.len(), 1);
        }
    }

    #[test]
    fn queued_protected_store_keeps_one_work_id_across_repeated_tcs() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                EventTag::new(1, 0, Generation::new(60)),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("queue the exact locked-body store");
        let original_task = services.store_tasks[0].clone();
        let protected = (fixture.manifest.round, fixture.manifest.subject);
        let mut high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        high_prepare.round = protected.0;
        high_prepare.subject = protected.1;

        for (view, generation) in [(1, 61), (2, 62)] {
            let mut timeout = timeout_at_view(&fixture, view - 1);
            timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: EventTag::new(1, view, Generation::new(generation)),
                        certificate: timeout,
                        protected_body: Some(protected),
                    }],
                    &mut services,
                )
                .expect("preserve queued protected storage across the TC");
            assert_eq!(executor.pending_stores.len(), 1);
            assert_eq!(
                executor.pending_stores[&original_task.id()].task,
                original_task
            );
            assert!(
                executor.pending_stores[&original_task.id()]
                    .consumer
                    .is_none()
            );
            assert!(services.cancelled_stores.is_empty());
            assert_eq!(services.store_tasks.len(), 1);
        }

        let current_tag = EventTag::new(1, 2, Generation::new(62));
        let sources = high_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        executor
            .consume_effects(
                vec![
                    AdapterEffect::FetchBody {
                        tag: current_tag,
                        round: protected.0,
                        subject: protected.1,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: sources,
                        certificate: Some(high_prepare),
                    },
                    AdapterEffect::StoreBody {
                        tag: current_tag,
                        round: protected.0,
                        subject: protected.1,
                    },
                ],
                &mut services,
            )
            .expect("the current reducer consumer adopts the immutable queued store");
        assert_eq!(services.store_tasks.len(), 1);
        assert_eq!(
            executor.pending_stores[&original_task.id()].consumer,
            Some(StoreConsumer::Reducer { tag: current_tag })
        );

        let completion = services.execute_store(original_task.id());
        assert_eq!(completion.tag(), original_task.tag());
        assert_eq!(
            executor
                .complete_body_store(completion, &mut services)
                .expect("the original immutable task routes to the latest consumer"),
            CompletionDisposition::Accepted
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyStored(completion_tag, _, _, _))
                if *completion_tag == current_tag
        ));
        assert_eq!(executor.pending_store_bytes, 0);
    }

    #[test]
    fn active_old_view_store_rebinds_current_consumer_before_late_completion() {
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
            .expect("start old-view body store");
        let store_id = services.store_tasks[0].id();
        services.inflight_stores.insert(store_id);

        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        let mut timeout = timeout_certificate(&fixture);
        timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout,
                    protected_body: Some((fixture.manifest.round, fixture.manifest.subject)),
                }],
                &mut services,
            )
            .expect("detach active old-view store consumer");

        assert!(
            services.cancelled_stores.is_empty(),
            "the effective durable lock owns immutable store work across the TC"
        );
        assert_eq!(executor.pending_stores.len(), 1);
        assert!(executor.pending_stores[&store_id].consumer.is_none());
        assert_eq!(
            executor.pending_store_bytes,
            u64::try_from(fixture.body.len()).expect("body length")
        );
        assert!(executor.body_pipeline_owners.is_empty());

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("current view adopts retained store through FetchBody");
        assert!(services.fetch_tasks.is_empty());
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(1) && manifest == &fixture.manifest
        ));
        assert_eq!(
            executor
                .body_pipeline_owners
                .get(&(fixture.manifest.round, fixture.manifest.subject))
                .map(|owner| owner.tag),
            Some(tag(1))
        );

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("attach current reducer consumer without duplicate I/O");
        assert_eq!(
            executor.pending_stores[&store_id].consumer,
            Some(StoreConsumer::Reducer { tag: tag(1) })
        );
        assert_eq!(services.store_tasks.len(), 1);

        let late_completion = services.execute_store(store_id);
        assert_eq!(late_completion.tag(), tag(0));
        assert_eq!(
            executor
                .complete_body_store(late_completion, &mut services)
                .expect("route late immutable completion to current consumer"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_stores.is_empty());
        assert_eq!(executor.pending_store_bytes, 0);
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyStored(
                completion_tag,
                completion_round,
                completion_subject,
                _
            )) if *completion_tag == tag(1)
                && *completion_round == fixture.manifest.round
                && *completion_subject == fixture.manifest.subject
        ));
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn active_old_view_store_completes_between_current_fetch_and_store() {
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
            .expect("start old-view body store");
        let store_id = services.store_tasks[0].id();
        services.inflight_stores.insert(store_id);

        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        let mut timeout = timeout_certificate(&fixture);
        timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout,
                    protected_body: Some((fixture.manifest.round, fixture.manifest.subject)),
                }],
                &mut services,
            )
            .expect("detach active old-view store consumer");
        assert!(executor.pending_stores[&store_id].consumer.is_none());

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("current FetchBody adopts detached store");
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(1) && manifest == &fixture.manifest
        ));
        assert!(executor.pending_stores[&store_id].consumer.is_none());
        assert_eq!(services.store_tasks.len(), 1);

        let late_completion = services.execute_store(store_id);
        assert_eq!(late_completion.tag(), tag(0));
        let expected_receipt = late_completion.receipt().clone();
        assert_eq!(
            executor
                .complete_body_store(late_completion, &mut services)
                .expect("catalog detached store before current StoreBody"),
            CompletionDisposition::Accepted
        );
        let key = (fixture.manifest.round, fixture.manifest.subject);
        assert!(executor.pending_stores.is_empty());
        assert_eq!(executor.pending_store_bytes, 0);
        assert_eq!(executor.durable_bodies.get(&key), Some(&expected_receipt));
        assert_eq!(
            executor.recovered_bodies.get(&key),
            Some(&(fixture.manifest.clone(), expected_receipt.clone()))
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(1) && manifest == &fixture.manifest
        ));

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("current StoreBody uses catalogued durable receipt");
        assert_eq!(services.store_tasks.len(), 1);
        assert!(executor.pending_stores.is_empty());
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyStored(
                completion_tag,
                completion_round,
                completion_subject,
                receipt
            )) if *completion_tag == tag(1)
                && *completion_round == fixture.manifest.round
                && *completion_subject == fixture.manifest.subject
                && receipt == &expected_receipt
        ));
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn retired_store_and_current_fetch_completion_are_order_independent() {
        for store_finishes_first in [true, false] {
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
                .expect("start old-view body store");
            let store_id = services.store_tasks[0].id();
            services.inflight_stores.insert(store_id);
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: tag(1),
                        certificate: timeout_certificate(&fixture),
                        protected_body: None,
                    }],
                    &mut services,
                )
                .expect("retire unprotected active store consumer");
            assert!(executor.pending_stores.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());

            let prepare = fixture.qc(wire::GlobalPhase::Prepare);
            let sources = prepare
                .signers
                .iter()
                .map(|index| fixture.context.roster[*index as usize].validator.clone())
                .collect();
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(1),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: sources,
                        certificate: Some(prepare),
                    }],
                    &mut services,
                )
                .expect("start current fetch without a detached store");
            let fetch_task = services.fetch_tasks.last().expect("current fetch").clone();
            let request = fetch_task
                .certified_request()
                .expect("signed certified request")
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

            let late_completion = services.execute_store(store_id);
            let durable = late_completion.receipt().clone();
            if store_finishes_first {
                assert_eq!(
                    executor
                        .complete_body_store(late_completion.clone(), &mut services)
                        .expect("catalog old store while current fetch is pending"),
                    CompletionDisposition::Stale
                );
            }

            assert_eq!(
                executor
                    .accept_certified_body_response(
                        response,
                        &fixture.context.roster[0].validator,
                        &mut services,
                    )
                    .expect("matching durable or empty state accepts current response"),
                CompletionDisposition::Accepted
            );
            let key = (fixture.manifest.round, fixture.manifest.subject);
            if store_finishes_first {
                assert!(executor.ready_bodies.is_empty());
                assert_eq!(executor.ready_body_bytes, 0);
            } else {
                assert_eq!(executor.ready_bodies.len(), 1);
                assert_eq!(
                    executor
                        .complete_body_store(late_completion, &mut services)
                        .expect("catalog old store after current fetch completion"),
                    CompletionDisposition::Stale
                );
                assert_eq!(executor.ready_bodies.len(), 1);
            }
            assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
            assert_eq!(
                executor.recovered_bodies.get(&key),
                Some(&(fixture.manifest.clone(), durable.clone()))
            );
            assert!(matches!(
                executor.runtime.completions.last(),
                Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                    if *completion_tag == tag(1) && manifest == &fixture.manifest
            ));

            executor
                .consume_effects(
                    vec![AdapterEffect::StoreBody {
                        tag: tag(1),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    }],
                    &mut services,
                )
                .expect("current StoreBody reuses exact durable receipt");
            assert_eq!(services.store_tasks.len(), 1);
            assert!(executor.ready_bodies.is_empty());
            assert_eq!(executor.ready_body_bytes, 0);
            assert!(matches!(
                executor.runtime.completions.last(),
                Some(RuntimeCompletion::BodyStored(
                    completion_tag,
                    completion_round,
                    completion_subject,
                    receipt
                )) if *completion_tag == tag(1)
                    && *completion_round == fixture.manifest.round
                    && *completion_subject == fixture.manifest.subject
                    && receipt == &durable
            ));
            assert!(executor.pending_fetches.is_empty());
            assert!(executor.certified_work.is_empty());
            assert!(executor.outstanding_requests.is_empty());
            assert_eq!(services.completed_certified_fetches, vec![fetch_task.id()]);
            assert!(!executor.status().fail_closed);
            assert!(services.closed.is_empty());
        }
    }

    #[test]
    fn current_fetch_fails_closed_on_conflicting_retired_store_receipt() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let mut alternate_chunk = fixture.body.clone();
        alternate_chunk[0] ^= 1;
        let alternate_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            fixture.manifest.subject,
            u64::try_from(fixture.body.len()).expect("body length"),
            std::slice::from_ref(&alternate_chunk),
        )
        .expect("structurally valid alternate manifest");
        assert_ne!(alternate_manifest, fixture.manifest);
        executor
            .admit_local_proposal(
                tag(0),
                alternate_manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("start old-view alternate store");
        let store_id = services.store_tasks[0].id();
        services.inflight_stores.insert(store_id);
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_certificate(&fixture),
                    protected_body: None,
                }],
                &mut services,
            )
            .expect("retire unprotected alternate store");

        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("start current canonical fetch");
        let request = services
            .fetch_tasks
            .last()
            .and_then(BodyFetchTask::certified_request)
            .expect("signed certified request")
            .clone();
        let late_completion = services.execute_store(store_id);
        let alternate_receipt = late_completion.receipt().clone();
        assert_eq!(
            executor
                .complete_body_store(late_completion, &mut services)
                .expect("catalog retired alternate store"),
            CompletionDisposition::Stale
        );

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
        assert!(matches!(
            executor.accept_certified_body_response(
                response,
                &fixture.context.roster[0].validator,
                &mut services,
            ),
            Err(EffectTransportError::FailClosed(reason))
                if reason.contains("retained durable body identity")
        ));
        let key = (fixture.manifest.round, fixture.manifest.subject);
        assert_eq!(
            executor.recovered_bodies.get(&key),
            Some(&(alternate_manifest, alternate_receipt.clone()))
        );
        assert_eq!(executor.durable_bodies.get(&key), Some(&alternate_receipt));
        assert!(executor.ready_bodies.is_empty());
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.status().fail_closed);
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn matching_ready_body_winner_makes_fetch_completion_idempotent() {
        let fixture = Fixture::new();
        let body_len = u64::try_from(fixture.body.len()).expect("body length");
        let mut executor = fixture.executor(EffectQueueConfig::new(8, 1, body_len, 4));
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
            .expect("start one exact fetch");
        let task = services.fetch_tasks[0].clone();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(
            &fixture.context,
            fixture.manifest.round,
            fixture.manifest.subject,
            fixture.body.clone(),
        )
        .expect("derive exact ready body");
        assert_eq!(ready.manifest, fixture.manifest);
        executor.ready_bodies.insert(key, ready);
        executor.ready_body_bytes = body_len;

        assert_eq!(
            executor
                .complete_body_reconstruction(
                    &task,
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("matching ready winner is idempotent at full capacity"),
            CompletionDisposition::Accepted
        );
        assert_eq!(executor.ready_bodies.len(), 1);
        assert_eq!(executor.ready_body_bytes, body_len);
        assert!(executor.pending_fetches.is_empty());
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(0) && manifest == &fixture.manifest
        ));
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn late_retired_store_cannot_overwrite_current_pending_manifest() {
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
            .expect("start old-view body store");
        let retired_id = services.store_tasks[0].id();
        services.inflight_stores.insert(retired_id);
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_certificate(&fixture),
                    protected_body: None,
                }],
                &mut services,
            )
            .expect("retire unprotected active store consumer");
        assert!(executor.pending_stores.is_empty());

        let mut alternate_chunk = fixture.body.clone();
        alternate_chunk[0] ^= 1;
        let alternate_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            fixture.manifest.subject,
            u64::try_from(fixture.body.len()).expect("body length"),
            std::slice::from_ref(&alternate_chunk),
        )
        .expect("structurally valid alternate manifest");
        assert_ne!(alternate_manifest, fixture.manifest);
        executor
            .admit_local_proposal(
                tag(1),
                alternate_manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("current view owns alternate exact manifest");
        let current_id = services.store_tasks.last().expect("current store").id();
        assert_ne!(current_id, retired_id);
        assert_eq!(executor.pending_stores.len(), 1);

        let late_completion = services.execute_store(retired_id);
        assert!(matches!(
            executor.complete_body_store(late_completion, &mut services),
            Err(EffectExecutorError::BodyStore(reason))
                if reason.contains("conflicts with retained exact-body ownership")
        ));
        assert_eq!(
            executor.pending_stores[&current_id].task.manifest(),
            &alternate_manifest
        );
        assert!(executor.recovered_bodies.is_empty());
        assert!(executor.durable_bodies.is_empty());
        assert!(executor.status().fail_closed);
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn active_losing_store_releases_capacity_for_high_qc_fetch() {
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
            .expect("start losing old-view body store");
        let store_id = services.store_tasks[0].id();
        services.inflight_stores.insert(store_id);

        let high_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"high-QC block")),
            ..fixture.manifest.subject
        };
        let mut high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        high_prepare.subject = high_subject;
        let sources = high_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        let mut timeout = timeout_certificate(&fixture);
        timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout,
                    protected_body: Some((high_prepare.round, high_prepare.subject)),
                }],
                &mut services,
            )
            .expect("release active losing-store ownership");

        assert_eq!(services.cancelled_stores, vec![store_id]);
        assert!(executor.pending_stores.is_empty());
        assert_eq!(executor.pending_store_bytes, 0);
        assert!(executor.body_pipeline_owners.is_empty());

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(1),
                    round: high_prepare.round,
                    subject: high_subject,
                    manifest: None,
                    certified_sources: sources,
                    certificate: Some(high_prepare),
                }],
                &mut services,
            )
            .expect("high-QC fetch uses the released bounded slot");
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.pending_work(), 1);

        let late_completion = services.execute_store(store_id);
        assert_eq!(late_completion.tag(), tag(0));
        assert_eq!(
            executor
                .complete_body_store(late_completion, &mut services)
                .expect("catalogue late losing-store completion"),
            CompletionDisposition::Stale
        );
        assert_eq!(executor.pending_fetches.len(), 1);
        assert!(executor.runtime.completions.is_empty());
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn view_change_cancels_non_durable_store_and_unprotected_validation() {
        for corrupt_class in ["store", "ready"] {
            for corruption in ["low", "high"] {
                let fixture = Fixture::new();
                let mut executor = fixture.executor(EffectQueueConfig::default());
                let mut services = fixture.services();
                match corrupt_class {
                    "store" => {
                        executor
                            .admit_local_proposal(
                                tag(0),
                                fixture.manifest.clone(),
                                fixture.body.clone(),
                                &mut services,
                            )
                            .expect("queue stale store");
                        executor.pending_store_bytes = match corruption {
                            "low" => 0,
                            "high" => executor
                                .pending_store_bytes
                                .checked_add(1)
                                .expect("small test counter"),
                            _ => unreachable!("the test enumerates low and high corruption"),
                        };
                    }
                    "ready" => {
                        executor
                            .admit_ready_body_for_test(&fixture, &mut services)
                            .expect("queue stale BodyAvailable completion");
                        executor.ready_body_bytes = match corruption {
                            "low" => 0,
                            "high" => executor
                                .ready_body_bytes
                                .checked_add(1)
                                .expect("small test counter"),
                            _ => unreachable!("the test enumerates low and high corruption"),
                        };
                    }
                    _ => unreachable!("the test enumerates both byte-owner classes"),
                }
                let before = executor.body_ownership_projection();

                assert!(matches!(
                    executor.consume_effects(
                        vec![AdapterEffect::EnterView {
                            tag: EventTag::new(1, 1, Generation::new(7)),
                            certificate: timeout_at_view(&fixture, 0),
                            protected_body: None,
                        }],
                        &mut services,
                    ),
                    Err(EffectExecutorError::Contract(reason))
                        if reason.contains("body byte accounting")
                ));
                assert_eq!(
                    executor.body_ownership_projection(),
                    before,
                    "{corrupt_class}/{corruption} accounting corruption must be rejected before ownership mutation"
                );
                assert!(services.cancelled_stores.is_empty());
                assert!(services.cancelled_fetches.is_empty());
                assert!(services.cancelled_validations.is_empty());
            }
        }

        // The counter covers the first ready body only. Without the global
        // preflight, lock reconciliation could retire that exact subset and
        // commit a zero residual before stale-view cleanup discovers the
        // second body's underflow.
        {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            for (view, generation) in [(0, 30), (2, 32)] {
                let manifest = manifest_at_view(&fixture, view);
                let key = (manifest.round, manifest.subject);
                let ready = ReadyBody::derive(
                    &fixture.context,
                    manifest.round,
                    manifest.subject,
                    fixture.body.clone(),
                )
                .expect("derive staged body at the selected view");
                let owner_tag = EventTag::new(1, view, Generation::new(generation));
                executor.body_pipeline_owners.insert(
                    key,
                    BodyPipelineOwner {
                        tag: owner_tag,
                        manifest_hash: Some(HashOf::new(&ready.manifest)),
                    },
                );
                executor
                    .runtime
                    .completions
                    .push(RuntimeCompletion::BodyAvailable(
                        owner_tag,
                        ready.manifest.clone(),
                    ));
                executor.ready_bodies.insert(key, ready);
            }
            executor.ready_body_bytes = u64::try_from(fixture.body.len()).expect("one body length");
            let before = executor.body_ownership_projection();
            let mut replacement = fixture.qc(wire::GlobalPhase::Prepare);
            replacement.round = manifest_at_view(&fixture, 1).round;
            let mut timeout = timeout_at_view(&fixture, 2);
            timeout.groups[0].highest_prepare_qc = Some(replacement.clone());

            assert!(matches!(
                executor.consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: EventTag::new(1, 3, Generation::new(33)),
                        certificate: timeout,
                        protected_body: Some((replacement.round, replacement.subject)),
                    }],
                    &mut services,
                ),
                Err(EffectExecutorError::Contract(reason))
                    if reason.contains("body byte accounting")
            ));
            assert_eq!(executor.body_ownership_projection(), before);
            assert!(executor.protected_lock.is_none());
            assert!(services.cancelled_stores.is_empty());
            assert!(services.cancelled_fetches.is_empty());
            assert!(services.cancelled_validations.is_empty());
        }

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
                    protected_body: None,
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
        let validation_id = services.validation_tasks[0].id();
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, 1, Generation::new(7)),
                    certificate: timeout_at_view(&fixture, 0),
                    protected_body: None,
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
        assert_eq!(services.cancelled_validations, vec![validation_id]);
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
            Hash::new(b"drifted effects fixture executed block wire"),
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
            CompletionDisposition::Accepted
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
                        protected_body: None,
                    },
                    AdapterEffect::ReportEquivocation {
                        offender: fixture.context.roster[1].validator.clone(),
                        round: fixture.manifest.round,
                        kind: EquivocationKind::Vote,
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
    fn authenticated_chunk_reconstruction_rejection_retires_fetch_nonfatally() {
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
        services.reject_authenticated_chunks = true;
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

        assert!(matches!(
            executor.accept_payload_chunk(
                work_id,
                chunk,
                &fixture.context.roster[0].validator,
                &mut services,
            ),
            Err(EffectTransportError::BodyMismatch(
                "authenticated chunks reconstructed invalid or noncanonical body data"
            ))
        ));
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert_eq!(services.chunks, vec![work_id]);
        assert!(services.closed.is_empty());
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn failed_view_cleanup_keeps_stale_fetch_and_requires_restart() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let certified_sources = prepare
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
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("admit prior-view body recovery");
        let before = executor.body_ownership_projection();
        services.fail_on = Some("cancel-fetch");

        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_at_view(&fixture, 0),
                    protected_body: None,
                }],
                &mut services,
            ),
            Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.entered_views.is_empty());
        assert!(executor.output_guard.restart_required());
        assert!(executor.status().fail_closed);
        assert_eq!(services.closed.len(), 1);
        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_at_view(&fixture, 0),
                    protected_body: None,
                }],
                &mut services,
            ),
            Err(EffectExecutorError::FailClosed(_))
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn view_cleanup_rejects_inconsistent_protected_request_before_lock_mutation() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let certified_sources = prepare
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
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("admit certified prior-view recovery");
        let request_hash = *executor
            .certified_work
            .keys()
            .next()
            .expect("certified request index");
        assert!(executor.outstanding_requests.cancel(request_hash));
        let before = executor.body_ownership_projection();

        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_at_view(&fixture, 0),
                    protected_body: Some((fixture.manifest.round, fixture.manifest.subject,)),
                }],
                &mut services,
            ),
            Err(EffectExecutorError::Contract(_))
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.protected_lock, None);
        assert!(services.cancelled_fetches.is_empty());
        assert!(services.entered_views.is_empty());
        assert!(executor.output_guard.restart_required());
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn view_cleanup_second_cancellation_failure_commits_no_fetch_retirement() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let first_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let first_sources = first_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        let (second_subject, second_body) = distinct_body(&fixture);
        let second_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            second_subject,
            u64::try_from(second_body.len()).expect("second body length"),
            std::slice::from_ref(&second_body),
        )
        .expect("second manifest");
        let mut second_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        second_prepare.subject = second_manifest.subject;
        let second_sources = second_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![
                    AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: first_sources,
                        certificate: Some(first_prepare),
                    },
                    AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: second_manifest.round,
                        subject: second_manifest.subject,
                        manifest: Some(second_manifest),
                        certified_sources: second_sources,
                        certificate: Some(second_prepare),
                    },
                ],
                &mut services,
            )
            .expect("admit two stale certified recoveries");
        assert_eq!(executor.pending_fetches.len(), 2);
        let first_work_id = services.fetch_tasks[0].id();
        let before = executor.body_ownership_projection();
        services.fail_on_call = Some(("cancel-fetch", 2));

        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_at_view(&fixture, 0),
                    protected_body: None,
                }],
                &mut services,
            ),
            Err(EffectExecutorError::Service(reason))
                if reason.contains("cancel-fetch call 2 failed")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.cancelled_fetches, vec![first_work_id]);
        assert!(services.entered_views.is_empty());
        assert!(executor.output_guard.restart_required());
        assert_eq!(services.closed.len(), 1);
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
        let fetch_task = services.fetch_tasks[0].clone();
        let work_id = fetch_task.id();
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
                &fetch_task,
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
    fn proposal_reconstruction_rejects_noncanonical_manifest_without_fail_close() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let mut alternate_chunk = fixture.body.clone();
        alternate_chunk[0] ^= 1;
        let alternate_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            fixture.manifest.subject,
            u64::try_from(fixture.body.len()).expect("body length"),
            std::slice::from_ref(&alternate_chunk),
        )
        .expect("structurally valid alternate manifest");

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(alternate_manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("proposal starts body acquisition");
        let fetch_task = services.fetch_tasks[0].clone();

        assert_eq!(
            executor
                .complete_body_reconstruction(
                    &fetch_task,
                    alternate_manifest,
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("noncanonical proposal data is a recoverable remote rejection"),
            CompletionDisposition::Rejected
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.ready_bodies.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert!(executor.runtime.completions.is_empty());
        assert!(services.closed.is_empty());
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
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("certified fetch");
        let work_id = services.fetch_tasks[0].id();
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
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.completed_certified_fetches, vec![work_id]);
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
    fn certified_manifest_mismatch_is_recoverable_in_both_authority_orders() {
        for proposal_first in [true, false] {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let prepare = fixture.qc(wire::GlobalPhase::Prepare);
            let sources = prepare
                .signers
                .iter()
                .map(|index| fixture.context.roster[*index as usize].validator.clone())
                .collect::<Vec<_>>();

            if proposal_first {
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
                    .expect("proposal starts body acquisition");
            }
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: proposal_first.then(|| fixture.manifest.clone()),
                        certified_sources: sources.clone(),
                        certificate: Some(prepare.clone()),
                    }],
                    &mut services,
                )
                .expect("certificate adds body authority");
            if !proposal_first {
                executor
                    .consume_effects(
                        vec![AdapterEffect::FetchBody {
                            tag: tag(0),
                            round: fixture.manifest.round,
                            subject: fixture.manifest.subject,
                            manifest: Some(fixture.manifest.clone()),
                            certified_sources: sources,
                            certificate: Some(prepare),
                        }],
                        &mut services,
                    )
                    .expect("proposal adds manifest authority");
            }

            let work_id = services.fetch_tasks[0].id();
            let request = services
                .fetch_tasks
                .last()
                .and_then(BodyFetchTask::certified_request)
                .expect("signed certified request")
                .clone();
            let mut alternate_chunk = fixture.body.clone();
            alternate_chunk[0] ^= 1;
            let alternate_manifest = wire::PayloadManifest::derive(
                &fixture.context,
                fixture.manifest.round,
                fixture.manifest.subject,
                u64::try_from(fixture.body.len()).expect("body length"),
                std::slice::from_ref(&alternate_chunk),
            )
            .expect("structurally valid alternate manifest");
            assert_ne!(alternate_manifest, fixture.manifest);

            let signed_response =
                |manifest: wire::PayloadManifest, responder: wire::ValidatorIndex| {
                    let mut response = wire::CertifiedBodyResponse {
                        request_hash: HashOf::new(&request),
                        manifest,
                        body: fixture.body.clone(),
                        responder,
                        signature: Vec::new(),
                    };
                    response.signature = Signature::new(
                        fixture.validator_keys[responder as usize].private_key(),
                        &response.signature_preimage(),
                    )
                    .payload()
                    .to_vec();
                    response
                };

            let mismatched = signed_response(alternate_manifest, 0);
            mismatched
                .validate_against(
                    &fixture.context,
                    &request,
                    &fixture.context.roster[0].validator,
                )
                .expect("alternate manifest passes request-level authentication");
            assert!(matches!(
                executor.accept_certified_body_response(
                    mismatched,
                    &fixture.context.roster[0].validator,
                    &mut services,
                ),
                Err(EffectTransportError::BodyMismatch(
                    "certified response manifest differs from proposal authority"
                ))
            ));
            assert_eq!(executor.pending_fetches.len(), 1);
            assert_eq!(executor.certified_work.len(), 1);
            assert_eq!(executor.outstanding_requests.len(), 1);
            assert!(services.completed_certified_fetches.is_empty());
            assert!(services.closed.is_empty());
            assert!(!executor.status().fail_closed);
            assert!(executor.runtime.completions.is_empty());

            let correct = signed_response(fixture.manifest.clone(), 1);
            assert_eq!(
                executor
                    .accept_certified_body_response(
                        correct,
                        &fixture.context.roster[1].validator,
                        &mut services,
                    )
                    .expect("another certified signer supplies the authoritative manifest"),
                CompletionDisposition::Accepted
            );
            assert!(executor.pending_fetches.is_empty());
            assert!(executor.certified_work.is_empty());
            assert!(executor.outstanding_requests.is_empty());
            assert_eq!(services.completed_certified_fetches, vec![work_id]);
            assert!(matches!(
                executor.runtime.completions.last(),
                Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                    if *completion_tag == tag(0) && manifest == &fixture.manifest
            ));
        }
    }

    #[test]
    fn certificate_first_response_rederives_manifest_before_proposal_authority() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
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
            .expect("certificate starts acquisition before proposal authority");

        let work_id = services.fetch_tasks[0].id();
        let request = services.fetch_tasks[0]
            .certified_request()
            .expect("signed certified request")
            .clone();
        let mut alternate_chunk = fixture.body.clone();
        alternate_chunk[0] ^= 1;
        let alternate_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            fixture.manifest.subject,
            u64::try_from(fixture.body.len()).expect("body length"),
            std::slice::from_ref(&alternate_chunk),
        )
        .expect("structurally valid alternate manifest");

        let signed_response = |manifest: wire::PayloadManifest, responder: wire::ValidatorIndex| {
            let mut response = wire::CertifiedBodyResponse {
                request_hash: HashOf::new(&request),
                manifest,
                body: fixture.body.clone(),
                responder,
                signature: Vec::new(),
            };
            response.signature = Signature::new(
                fixture.validator_keys[responder as usize].private_key(),
                &response.signature_preimage(),
            )
            .payload()
            .to_vec();
            response
        };

        let mismatched = signed_response(alternate_manifest, 0);
        mismatched
            .validate_against(
                &fixture.context,
                &request,
                &fixture.context.roster[0].validator,
            )
            .expect("alternate manifest passes request-level authentication");
        assert!(matches!(
            executor.accept_certified_body_response(
                mismatched,
                &fixture.context.roster[0].validator,
                &mut services,
            ),
            Err(EffectTransportError::BodyMismatch(
                "certified response manifest is not canonical for its body"
            ))
        ));
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.certified_work.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);
        assert!(services.completed_certified_fetches.is_empty());
        assert!(services.closed.is_empty());
        assert!(!executor.status().fail_closed);

        assert_eq!(
            executor
                .accept_certified_body_response(
                    signed_response(fixture.manifest.clone(), 1),
                    &fixture.context.roster[1].validator,
                    &mut services,
                )
                .expect("another certified signer supplies the canonical manifest"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.completed_certified_fetches, vec![work_id]);
    }

    #[test]
    fn certified_response_retirement_failure_is_fail_closed_after_authentication() {
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
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("hybrid fetch");
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
        let before = executor.body_ownership_projection();
        services.fail_on = Some("complete-certified-fetch");

        assert!(matches!(
            executor.accept_certified_body_response(
                response,
                &fixture.context.roster[0].validator,
                &mut services,
            ),
            Err(EffectTransportError::FailClosed(_))
        ));
        assert!(executor.status().fail_closed);
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn decision_installation_frees_losing_capacity_before_fetch() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 2));
        let mut services = fixture.services();
        let (losing_subject, losing_body) = distinct_body(&fixture);
        let losing_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            losing_subject,
            u64::try_from(losing_body.len()).expect("losing body length"),
            std::slice::from_ref(&losing_body),
        )
        .expect("losing manifest");
        let mut losing_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        losing_prepare.subject = losing_manifest.subject;
        let certified_sources = losing_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: losing_manifest.round,
                    subject: losing_manifest.subject,
                    manifest: Some(losing_manifest),
                    certified_sources,
                    certificate: Some(losing_prepare),
                }],
                &mut services,
            )
            .expect("fill the only pending-work slot with a losing fetch");
        let losing_id = services.fetch_tasks[0].id();

        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decided_body =
            Some((commit.round, commit.subject, commit.execution_commitment));
        let certified_sources = commit
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: commit.round,
                    subject: commit.subject,
                    manifest: None,
                    certified_sources,
                    certificate: Some(commit.clone()),
                }],
                &mut services,
            )
            .expect("Decision cleanup frees capacity before decided-body recovery");

        assert_eq!(
            executor.protected_decision,
            Some((commit.round, commit.subject))
        );
        assert_eq!(executor.pending_fetches.len(), 1);
        assert!(executor.pending_fetches.values().all(|pending| {
            pending.task.round == commit.round && pending.task.subject == commit.subject
        }));
        assert_eq!(services.cancelled_fetches, vec![losing_id]);
        assert_eq!(services.retired_all_outbound, 1);
        assert_eq!(services.retired_candidate_work, 1);
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn decision_installed_by_same_runtime_step_retires_stale_terminal_effects() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decision_on_next_step =
            Some((commit.round, commit.subject, commit.execution_commitment));
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![
                AdapterEffect::Broadcast(proposal(&fixture)),
                AdapterEffect::Sign {
                    tag: tag(0),
                    request: SignRequest::Vote(vote(&fixture)),
                },
            ])));
        services.fail_on = Some("broadcast");

        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("durable Decision retires stale in-flight effects"),
            EffectExecutorStep::Advanced { effects: 0 }
        );
        assert_eq!(services.fail_on, Some("broadcast"));
        assert!(services.broadcasts.is_empty());
        assert!(services.sign_tasks.is_empty());
        assert_eq!(services.retired_all_outbound, 1);
        assert_eq!(services.retired_candidate_work, 1);
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn decision_installed_by_same_runtime_step_keeps_exact_commit_and_body_work() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        let decision = (commit.round, commit.subject, commit.execution_commitment);
        let exact_commit_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(commit.clone()),
        );
        let (losing_subject, _) = distinct_body(&fixture);
        let mut losing_commit = commit.clone();
        losing_commit.subject = losing_subject;
        let losing_commit_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(losing_commit),
        );
        let certified_sources = commit
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor.runtime.decision_on_next_step = Some(decision);
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![
                AdapterEffect::Broadcast(proposal(&fixture)),
                AdapterEffect::Broadcast(losing_commit_message),
                AdapterEffect::Broadcast(exact_commit_message.clone()),
                AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: commit.round,
                    subject: losing_subject,
                    manifest: None,
                    certified_sources: Vec::new(),
                    certificate: None,
                },
                AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: commit.round,
                    subject: commit.subject,
                    manifest: None,
                    certified_sources,
                    certificate: Some(commit.clone()),
                },
                AdapterEffect::Sign {
                    tag: tag(0),
                    request: SignRequest::Vote(vote(&fixture)),
                },
            ])));

        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("dispatch only exact post-Decision effects"),
            EffectExecutorStep::Advanced { effects: 2 }
        );
        assert_eq!(services.broadcasts, vec![exact_commit_message]);
        assert!(services.sign_tasks.is_empty());
        assert_eq!(services.fetch_tasks.len(), 1);
        assert_eq!(services.fetch_tasks[0].round, commit.round);
        assert_eq!(services.fetch_tasks[0].subject, commit.subject);
        assert_eq!(services.retired_all_outbound, 1);
        assert_eq!(services.retired_candidate_work, 1);
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }

    #[test]
    fn failed_decision_cleanup_keeps_losing_owner_and_requires_restart() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (losing_subject, losing_body) = distinct_body(&fixture);
        let losing_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            losing_subject,
            u64::try_from(losing_body.len()).expect("losing body length"),
            std::slice::from_ref(&losing_body),
        )
        .expect("losing manifest");
        let mut losing_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        losing_prepare.subject = losing_manifest.subject;
        let certified_sources = losing_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: losing_manifest.round,
                    subject: losing_manifest.subject,
                    manifest: Some(losing_manifest),
                    certified_sources,
                    certificate: Some(losing_prepare),
                }],
                &mut services,
            )
            .expect("admit losing body recovery");
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        let durable_decision = (commit.round, commit.subject, commit.execution_commitment);
        executor.runtime.decided_body = Some(durable_decision);
        let before = executor.body_ownership_projection();
        services.fail_on = Some("cancel-fetch");

        assert!(matches!(
            executor.consume_effects(Vec::new(), &mut services),
            Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.decided_body, Some(durable_decision));
        assert_eq!(executor.protected_decision, None);
        assert!(executor.output_guard.restart_required());
        assert!(executor.status().fail_closed);
        assert_eq!(services.closed.len(), 1);
        assert!(matches!(
            executor.consume_effects(Vec::new(), &mut services),
            Err(EffectExecutorError::FailClosed(_))
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn decision_cleanup_fetch_failure_preserves_exact_local_pipeline_consumer() {
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
            .expect("start exact decided local store");
        let (losing_subject, losing_body) = distinct_body(&fixture);
        let losing_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            losing_subject,
            u64::try_from(losing_body.len()).expect("losing body length"),
            std::slice::from_ref(&losing_body),
        )
        .expect("losing manifest");
        let mut losing_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        losing_prepare.subject = losing_manifest.subject;
        let certified_sources = losing_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: losing_manifest.round,
                    subject: losing_manifest.subject,
                    manifest: Some(losing_manifest),
                    certified_sources,
                    certificate: Some(losing_prepare),
                }],
                &mut services,
            )
            .expect("admit losing certified recovery beside decided local work");
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decided_body =
            Some((commit.round, commit.subject, commit.execution_commitment));
        let before = executor.body_ownership_projection();
        services.fail_on = Some("cancel-fetch");

        assert!(matches!(
            executor.consume_effects(Vec::new(), &mut services),
            Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.pending_stores.len(), 1);
        assert!(matches!(
            executor
                .pending_stores
                .values()
                .next()
                .and_then(|pending| pending.consumer.as_ref()),
            Some(StoreConsumer::LocalProposal { .. })
        ));
        assert!(executor.output_guard.restart_required());
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn decision_cleanup_rejects_inconsistent_certified_request_before_mutation() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (losing_subject, losing_body) = distinct_body(&fixture);
        let losing_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            fixture.manifest.round,
            losing_subject,
            u64::try_from(losing_body.len()).expect("losing body length"),
            std::slice::from_ref(&losing_body),
        )
        .expect("losing manifest");
        let mut losing_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        losing_prepare.subject = losing_manifest.subject;
        let certified_sources = losing_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: losing_manifest.round,
                    subject: losing_manifest.subject,
                    manifest: Some(losing_manifest),
                    certified_sources,
                    certificate: Some(losing_prepare),
                }],
                &mut services,
            )
            .expect("admit losing certified recovery");
        let request_hash = *executor
            .certified_work
            .keys()
            .next()
            .expect("certified request index");
        assert!(executor.outstanding_requests.cancel(request_hash));
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        let durable_decision = (commit.round, commit.subject, commit.execution_commitment);
        executor.runtime.decided_body = Some(durable_decision);
        let before = executor.body_ownership_projection();

        assert!(matches!(
            executor.consume_effects(Vec::new(), &mut services),
            Err(EffectExecutorError::Contract(_))
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.decided_body, Some(durable_decision));
        assert_eq!(executor.protected_decision, None);
        assert!(services.cancelled_fetches.is_empty());
        assert_eq!(services.retired_all_outbound, 0);
        assert_eq!(services.retired_candidate_work, 0);
        assert!(executor.output_guard.restart_required());
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn decision_preserves_current_tag_local_proposal_for_direct_apply() {
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
            .expect("start exact local proposal");
        complete_local_proposal_chain(&mut executor, &mut services);
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::LocalProposal(_, manifest, ..)]
                if manifest == &fixture.manifest
        ));

        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decided_body =
            Some((commit.round, commit.subject, commit.execution_commitment));
        let certified_sources = commit
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: commit.round,
                    subject: commit.subject,
                    manifest: None,
                    certified_sources,
                    certificate: Some(commit),
                }],
                &mut services,
            )
            .expect("preserve the exact local completion across Decision cleanup");

        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::LocalProposal(completion_tag, manifest, ..)]
                if *completion_tag == tag(0) && manifest == &fixture.manifest
        ));
        assert_eq!(executor.body_pipeline_owners.len(), 1);
        assert!(services.fetch_tasks.is_empty());
        assert_eq!(services.retired_all_outbound, 1);
        assert_eq!(services.retired_candidate_work, 1);

        executor
            .consume_effects(Vec::new(), &mut services)
            .expect("Decision reconciliation is idempotent");
        assert_eq!(services.retired_all_outbound, 1);
        assert_eq!(services.retired_candidate_work, 1);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn decision_commitment_mismatch_fails_closed_before_apply() {
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
            .expect("start exact local proposal");
        complete_local_proposal_chain(&mut executor, &mut services);
        let conflicting_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"Decision conflict parent state"),
            Hash::new(b"Decision conflict post state"),
            Hash::new(b"Decision conflict ordinary writes"),
            Hash::new(b"Decision conflict executed block"),
        );
        assert_ne!(conflicting_commitment, fixture_execution_commitment());
        executor.runtime.decided_body = Some((
            fixture.manifest.round,
            fixture.manifest.subject,
            conflicting_commitment,
        ));

        assert!(matches!(
            executor.consume_effects(Vec::new(), &mut services),
            Err(EffectExecutorError::Runtime(reason))
                if reason.contains("conflicts with the durable Decision")
        ));
        assert!(executor.status().fail_closed);
        assert!(services.apply_tasks.is_empty());
        assert!(services.fetch_tasks.is_empty());
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::LocalProposal(completion_tag, manifest, ..)]
                if *completion_tag == tag(0) && manifest == &fixture.manifest
        ));
    }

    #[test]
    fn stale_generation_local_completion_uses_durable_recovery() {
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
            .expect("start old-generation local proposal");
        complete_local_proposal_chain(&mut executor, &mut services);
        let current_tag = EventTag::new(1, 1, Generation::new(8));
        executor.runtime.round_tag = Some(current_tag);
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decided_body =
            Some((commit.round, commit.subject, commit.execution_commitment));
        let certified_sources = commit
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: current_tag,
                    round: commit.round,
                    subject: commit.subject,
                    manifest: None,
                    certified_sources,
                    certificate: Some(commit),
                }],
                &mut services,
            )
            .expect("stale completion falls back to durable body reconstruction");

        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
                if *completion_tag == current_tag && manifest == &fixture.manifest
        ));
        assert!(services.apply_tasks.is_empty());
        assert!(services.fetch_tasks.is_empty());
        assert_eq!(executor.body_pipeline_owners.len(), 1);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn decision_rebinds_exact_local_validation_to_reducer_progress() {
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
            .expect("start exact local proposal");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("advance exact local proposal to validation");
        let validation_id = services.validation_tasks[0].id();
        assert!(matches!(
            &executor.pending_validations[&validation_id].consumer,
            Some(ValidationConsumer::LocalProposal { .. })
        ));

        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor.runtime.decided_body =
            Some((commit.round, commit.subject, commit.execution_commitment));
        let certified_sources = commit
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: commit.round,
                    subject: commit.subject,
                    manifest: None,
                    certified_sources,
                    certificate: Some(commit),
                }],
                &mut services,
            )
            .expect("Decision detaches the exact local validation consumer");
        assert!(
            executor.pending_validations[&validation_id]
                .consumer
                .is_none()
        );
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(_, manifest)] if manifest == &fixture.manifest
        ));

        executor.runtime.completions.clear();
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("decided reducer adopts the exact durable body");
        executor.runtime.completions.clear();
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("decided reducer reattaches exact validation work");
        assert!(matches!(
            &executor.pending_validations[&validation_id].consumer,
            Some(ValidationConsumer::Reducer { tag: consumer }) if *consumer == tag(0)
        ));
        assert_eq!(
            services.validation_tasks.last().map(BodyValidationTask::id),
            Some(validation_id)
        );
        assert!(!executor.status().fail_closed);
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
            CompletionDisposition::Accepted
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

        let authenticated_genesis_context = verify_pending_kura_apply_parts(
            &fixture.context,
            decision,
            &recovered,
            &validations,
            expected,
        )
        .expect("exact replay binding")
        .expect("height-one replay mints a genesis projection capability");
        assert_eq!(
            authenticated_genesis_context.hash(),
            fixture.context.nexus_amx_context_hash
        );

        let mut wrong_context = fixture.context.clone();
        wrong_context.nexus_amx_context_hash = Hash::new(b"different frozen Nexus/AMX context");
        assert_ne!(
            wrong_context.id(),
            fixture.context.id(),
            "height-context identity must bind the Nexus/AMX projection"
        );
        assert!(matches!(
            verify_pending_kura_apply_parts(
                &wrong_context,
                decision,
                &recovered,
                &validations,
                expected,
            ),
            Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
                if reason.contains("different frozen height context")
        ));
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
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );

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
        assert!(
            status_services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn ready_body_backpressure_and_mismatches_are_recoverable_transport_rejections() {
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
        let fetch_task = services.fetch_tasks[0].clone();
        assert!(matches!(
            executor.complete_body_reconstruction(
                &fetch_task,
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
                &fetch_task,
                fixture.manifest.clone(),
                wrong,
                &mut services,
            ),
            Err(EffectTransportError::BodyMismatch(_))
        ));

        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("upgrade the retained fetch with certified authority");
        let request = services
            .fetch_tasks
            .last()
            .and_then(BodyFetchTask::certified_request)
            .expect("signed certified request");
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(request),
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

        assert!(matches!(
            executor.accept_certified_body_response(
                response.clone(),
                &fixture.context.roster[0].validator,
                &mut services,
            ),
            Err(EffectTransportError::Backpressure)
        ));
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.certified_work.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);
        assert!(!executor.status().fail_closed);

        executor.config.max_ready_body_bytes =
            u64::try_from(fixture.body.len()).expect("body length");
        assert_eq!(
            executor
                .accept_certified_body_response(
                    response,
                    &fixture.context.roster[0].validator,
                    &mut services,
                )
                .expect("same response succeeds after capacity is available"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
    }

    #[test]
    fn body_fetch_authority_upgrades_monotonically_in_both_orders() {
        let fixture = Fixture::new();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();

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
            .expect("proposal starts ordinary acquisition");
        let work_id = services.fetch_tasks[0].id();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources.clone(),
                    certificate: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("PrepareQC adds certified authority");
        let upgraded = services.fetch_tasks.last().expect("upgraded task");
        assert_eq!(upgraded.id(), work_id);
        assert_eq!(upgraded.manifest(), Some(&fixture.manifest));
        assert_eq!(
            upgraded
                .certified_request()
                .map(|request| &request.certificate),
            Some(&prepare)
        );
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);

        let first_request = upgraded
            .certified_request()
            .expect("first certified authority")
            .clone();
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources.clone(),
                    certificate: Some(commit),
                }],
                &mut services,
            )
            .expect("later same-subject QC retransmits first authority");
        assert_eq!(
            services
                .fetch_tasks
                .last()
                .and_then(BodyFetchTask::certified_request),
            Some(&first_request)
        );
        assert_eq!(executor.outstanding_requests.len(), 1);

        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: None,
                    certified_sources: sources.clone(),
                    certificate: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("PrepareQC starts certified acquisition");
        let work_id = services.fetch_tasks[0].id();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("proposal adds manifest authority");
        let upgraded = services.fetch_tasks.last().expect("upgraded task");
        assert_eq!(upgraded.id(), work_id);
        assert_eq!(upgraded.manifest(), Some(&fixture.manifest));
        assert_eq!(
            upgraded
                .certified_request()
                .map(|request| &request.certificate),
            Some(&prepare)
        );
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);
    }

    #[test]
    fn hybrid_reconstruction_wins_and_retires_certified_request() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("start hybrid acquisition");
        let task = services.fetch_tasks[0].clone();

        assert_eq!(
            executor
                .complete_body_reconstruction(
                    &task,
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("authenticated reconstruction wins"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert!(services.completed_certified_fetches.is_empty());
    }

    #[test]
    fn retained_exact_body_pipeline_prevents_reacquisition_at_every_stage() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let fetch = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        executor
            .consume_effects(vec![fetch.clone()], &mut services)
            .expect("start one exact acquisition");
        let task = services.fetch_tasks[0].clone();
        executor
            .complete_body_reconstruction(
                &task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("retain reconstructed body");
        assert_eq!(executor.runtime.queued_commands(), 1);

        executor
            .consume_effects(vec![fetch.clone()], &mut services)
            .expect("ready body makes FetchBody idempotent");
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.ready_bodies.len(), 1);
        assert_eq!(executor.runtime.queued_commands(), 1);

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("advance body into exact store ownership");
        executor
            .consume_effects(vec![fetch.clone()], &mut services)
            .expect("pending store makes FetchBody idempotent");
        assert_eq!(services.fetch_tasks.len(), 1);
        assert_eq!(executor.pending_stores.len(), 1);
        assert_eq!(executor.runtime.queued_commands(), 1);

        let store_id = services.store_tasks[0].id();
        let completion = services.execute_store(store_id);
        executor
            .complete_body_store(completion, &mut services)
            .expect("advance body into durable ownership");
        assert_eq!(executor.runtime.queued_commands(), 2);
        executor
            .consume_effects(vec![fetch], &mut services)
            .expect("durable receipt makes FetchBody idempotent");
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.durable_bodies.len(), 1);
        assert_eq!(executor.runtime.queued_commands(), 2);

        let mut conflicting_manifest = fixture.manifest.clone();
        conflicting_manifest.payload_size_bytes = conflicting_manifest
            .payload_size_bytes
            .checked_add(1)
            .expect("small fixture body");
        let conflicting_result = executor.consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(conflicting_manifest),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        );
        assert!(
            matches!(conflicting_result, Err(EffectExecutorError::Contract(_))),
            "conflicting retained manifest must fail closed: {conflicting_result:?}"
        );
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(executor.status().fail_closed);
    }

    #[test]
    fn uncertified_fetch_rejects_spurious_certified_sources() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();

        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: vec![fixture.context.roster[0].validator.clone()],
                    certificate: None,
                }],
                &mut services,
            ),
            Err(EffectExecutorError::Contract(_))
        ));
        assert!(services.fetch_tasks.is_empty());
        assert!(executor.status().fail_closed);
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
    fn tc_body_rebind_preserves_the_exact_fetch_until_reconstruction_completes() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let consumer_tag = |view| EventTag::new(1, view, Generation::new(7 + view));
        let sources = high_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        let fetch = |view| AdapterEffect::FetchBody {
            tag: consumer_tag(view),
            round: high_prepare.round,
            subject: high_prepare.subject,
            manifest: None,
            certified_sources: sources.clone(),
            certificate: Some(high_prepare.clone()),
        };

        executor
            .consume_effects(vec![fetch(0)], &mut services)
            .expect("begin exact high-QC fetch");
        let work_id = services.fetch_tasks[0].id();

        for view in 0..3 {
            let mut timeout = timeout_at_view(&fixture, view);
            timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: consumer_tag(view + 1),
                        certificate: timeout,
                        protected_body: Some((high_prepare.round, high_prepare.subject)),
                    }],
                    &mut services,
                )
                .expect("rebind protected fetch across certified view");
            assert_eq!(executor.pending_fetches.len(), 1);
            assert_eq!(
                executor.pending_fetches[&work_id].task.tag,
                consumer_tag(view + 1)
            );
            assert_eq!(
                services.fetch_tasks.last().map(BodyFetchTask::id),
                Some(work_id)
            );
            assert_eq!(
                services.fetch_tasks.last().map(|task| task.tag),
                Some(consumer_tag(view + 1))
            );
            assert!(services.cancelled_fetches.is_empty());

            executor
                .consume_effects(vec![fetch(view + 1)], &mut services)
                .expect("new reducer incarnation adopts the protected fetch");
            assert_eq!(executor.pending_fetches.len(), 1);
            assert_eq!(executor.pending_work(), 1);
        }

        let task = executor.pending_fetches[&work_id].task.clone();
        assert_eq!(task.tag, consumer_tag(3));
        assert_eq!(
            executor
                .complete_body_reconstruction(
                    &task,
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("complete once after repeated TC rebinding"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.ready_bodies.len(), 1);
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
                if *completion_tag == consumer_tag(3) && manifest == &fixture.manifest
        ));
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn tc_body_rebind_preserves_certified_request_ownership_through_signed_response() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let original_tag = EventTag::new(1, 0, Generation::new(70));
        let rebound_tag = EventTag::new(1, 1, Generation::new(71));
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: original_tag,
                    round: prepare.round,
                    subject: prepare.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources.clone(),
                    certificate: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("begin exact certified acquisition");
        let original = services.fetch_tasks[0].clone();
        let work_id = original.id();
        let request = original
            .certified_request()
            .expect("certified acquisition owns one signed request")
            .clone();
        let request_hash = HashOf::new(&request);
        assert_eq!(executor.certified_work.get(&request_hash), Some(&work_id));
        assert_eq!(executor.outstanding_requests.len(), 1);

        let mut timeout = timeout_at_view(&fixture, 0);
        timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: rebound_tag,
                    certificate: timeout,
                    protected_body: Some((prepare.round, prepare.subject)),
                }],
                &mut services,
            )
            .expect("rebind the certified acquisition across TC installation");
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: rebound_tag,
                    round: prepare.round,
                    subject: prepare.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("new reducer incarnation adopts the rebound acquisition");

        let rebound = executor.pending_fetches[&work_id].task.clone();
        assert!(rebound.rebinds_consumer_of(&original));
        assert_eq!(rebound.certified_request(), Some(&request));
        assert_eq!(executor.certified_work.get(&request_hash), Some(&work_id));
        assert_eq!(executor.outstanding_requests.len(), 1);

        let mut response = wire::CertifiedBodyResponse {
            request_hash,
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
                    response,
                    &fixture.context.roster[0].validator,
                    &mut services,
                )
                .expect("the original signed request authorizes the rebound response"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.completed_certified_fetches, vec![work_id]);
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(tag, manifest)]
                if *tag == rebound_tag && manifest == &fixture.manifest
        ));
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn tc_body_rebind_uses_the_effective_local_lock_when_the_tc_omits_or_lowers_it() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let manifest = manifest_at_view(&fixture, 1);
        let mut local_lock = fixture.qc(wire::GlobalPhase::Prepare);
        local_lock.round = manifest.round;
        local_lock.subject = manifest.subject;
        let sources = local_lock
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        let consumer_tag = |view| EventTag::new(1, view, Generation::new(20 + view));
        let fetch = |view| AdapterEffect::FetchBody {
            tag: consumer_tag(view),
            round: local_lock.round,
            subject: local_lock.subject,
            manifest: None,
            certified_sources: sources.clone(),
            certificate: Some(local_lock.clone()),
        };

        executor
            .consume_effects(vec![fetch(1)], &mut services)
            .expect("begin local-lock fetch");
        let work_id = services.fetch_tasks[0].id();

        let omitted = timeout_at_view(&fixture, 1);
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: consumer_tag(2),
                    certificate: omitted,
                    protected_body: Some((local_lock.round, local_lock.subject)),
                }],
                &mut services,
            )
            .expect("an omitted TC high cannot lower the effective local lock");
        executor
            .consume_effects(vec![fetch(2)], &mut services)
            .expect("the new reducer incarnation adopts the same fetch");

        let mut lowered = timeout_at_view(&fixture, 2);
        lowered.groups[0].highest_prepare_qc = Some(fixture.qc(wire::GlobalPhase::Prepare));
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: consumer_tag(3),
                    certificate: lowered,
                    protected_body: Some((local_lock.round, local_lock.subject)),
                }],
                &mut services,
            )
            .expect("a lower TC high cannot replace the effective local lock");
        executor
            .consume_effects(vec![fetch(3)], &mut services)
            .expect("the effective lock adopts the same immutable work again");

        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.pending_fetches[&work_id].task.tag, consumer_tag(3));
        assert!(services.fetch_tasks.iter().all(|task| task.id() == work_id));
        assert!(services.cancelled_fetches.is_empty());

        let task = executor.pending_fetches[&work_id].task.clone();
        assert_eq!(
            executor
                .complete_body_reconstruction(&task, manifest, fixture.body.clone(), &mut services,)
                .expect("the once-rebound local-lock work completes"),
            CompletionDisposition::Accepted
        );
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyAvailable(completion_tag, _)]
                if *completion_tag == consumer_tag(3)
        ));
    }

    #[test]
    fn enter_view_rejects_a_tc_high_without_an_effective_protected_body() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let mut timeout = timeout_at_view(&fixture, 0);
        timeout.groups[0].highest_prepare_qc = Some(fixture.qc(wire::GlobalPhase::Prepare));

        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout,
                    protected_body: None,
                }],
                &mut services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("omitted the body protected")
        ));
        assert!(executor.status().fail_closed);
    }

    #[test]
    fn tc_body_rebind_retags_a_queued_body_available_completion() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let consumer_tag = |view| EventTag::new(1, view, Generation::new(7 + view));
        let sources = high_prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        let fetch = |view| AdapterEffect::FetchBody {
            tag: consumer_tag(view),
            round: high_prepare.round,
            subject: high_prepare.subject,
            manifest: None,
            certified_sources: sources.clone(),
            certificate: Some(high_prepare.clone()),
        };

        executor
            .consume_effects(vec![fetch(0)], &mut services)
            .expect("begin exact high-QC fetch");
        let task = services.fetch_tasks[0].clone();
        executor
            .complete_body_reconstruction(
                &task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("queue old-view body completion");
        assert!(executor.pending_fetches.is_empty());
        assert_eq!(executor.ready_bodies.len(), 1);

        for view in 0..3 {
            let mut timeout = timeout_at_view(&fixture, view);
            timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: consumer_tag(view + 1),
                        certificate: timeout,
                        protected_body: Some((high_prepare.round, high_prepare.subject)),
                    }],
                    &mut services,
                )
                .expect("rebind protected terminal completion");
            executor
                .consume_effects(vec![fetch(view + 1)], &mut services)
                .expect("new reducer incarnation adopts the ready body");
            assert_eq!(executor.ready_bodies.len(), 1);
            assert!(executor.pending_fetches.is_empty());
            assert!(services.cancelled_fetches.is_empty());
            assert!(matches!(
                executor.runtime.completions.as_slice(),
                [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
                    if *completion_tag == consumer_tag(view + 1)
                        && manifest == &fixture.manifest
            ));
        }
        assert_eq!(executor.ready_body_bytes, fixture.body.len() as u64);
        assert_eq!(executor.pending_work(), 0);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn tc_body_rebind_retires_a_superseded_completion_and_releases_capacity() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 1, 1_048_576, 1));
        let mut services = fixture.services();
        let original = fixture.qc(wire::GlobalPhase::Prepare);
        let original_sources = original
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: EventTag::new(1, 0, Generation::new(30)),
                    round: original.round,
                    subject: original.subject,
                    manifest: None,
                    certified_sources: original_sources,
                    certificate: Some(original.clone()),
                }],
                &mut services,
            )
            .expect("start original fetch");
        let original_task = services.fetch_tasks[0].clone();
        executor
            .complete_body_reconstruction(
                &original_task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("queue original BodyAvailable");
        assert_eq!(executor.runtime.completions.len(), 1);
        assert_eq!(executor.ready_bodies.len(), 1);

        let replacement_manifest = manifest_at_view(&fixture, 1);
        let mut replacement = original;
        replacement.round = replacement_manifest.round;
        replacement.subject = replacement_manifest.subject;
        let mut timeout = timeout_at_view(&fixture, 1);
        timeout.groups[0].highest_prepare_qc = Some(replacement.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, 2, Generation::new(32)),
                    certificate: timeout,
                    protected_body: Some((replacement.round, replacement.subject)),
                }],
                &mut services,
            )
            .expect("supersede the old completion with a higher exact lock");

        assert!(executor.runtime.completions.is_empty());
        assert!(executor.ready_bodies.is_empty());
        assert_eq!(executor.ready_body_bytes, 0);
        assert!(executor.body_pipeline_owners.is_empty());

        let replacement_sources = replacement
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: EventTag::new(1, 2, Generation::new(32)),
                    round: replacement.round,
                    subject: replacement.subject,
                    manifest: None,
                    certified_sources: replacement_sources,
                    certificate: Some(replacement),
                }],
                &mut services,
            )
            .expect("the replacement claims the released one-item work capacity");
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.pending_work(), 1);
    }

    #[test]
    fn serialized_runtime_rebinds_busy_deferred_body_completion_before_service() {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS validator key")
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
            chain_id: "serialized-body-rebind-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"serialized rebind nexus context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1_048_576,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1_048_576,
                max_chunk_count: 1,
            },
            leader_seed: [0x44; 32],
        };
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified =
            VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified context");
        let directory = TempDir::new().expect("temporary runtime directory");
        let (mut adapter, startup) = SumeragiV2Adapter::open(
            directory.path().join("serialized-rebind-safety.wal"),
            verified,
            None,
            Generation::new(1),
            [0x55; 32],
            AdapterFingerprints {
                node: Hash::new(b"serialized rebind node"),
                build: Hash::new(b"serialized rebind build"),
                config: Hash::new(b"serialized rebind config"),
            },
        )
        .expect("open observing adapter");
        assert!(startup.is_empty());

        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            3_000,
            0,
        );
        let block_signature = SignatureOf::try_from_hash(keys[0].private_key(), header.hash())
            .expect("canonical body signature");
        let block =
            SignedBlock::presigned(BlockSignature::new(0, block_signature), header, Vec::new());
        let body = block.encode_wire().expect("canonical SignedBlockWire");
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
        .expect("canonical body manifest");
        let execution_commitment = fixture_execution_commitment();
        let prepare_preimage = wire::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let prepare_shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &prepare_preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let prepare_refs = prepare_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let prepare = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
                .expect("aggregate PrepareQC"),
        };
        let signed_timeout = |signer: wire::ValidatorIndex| {
            let mut vote = wire::TimeoutVote {
                round,
                highest_prepare_qc: Some(prepare.clone()),
                signer,
                signature: Vec::new(),
            };
            vote.signature = Signature::new(
                keys[usize::try_from(signer).expect("small signer")].private_key(),
                &vote.signature_preimage(),
            )
            .payload()
            .to_vec();
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(vote))
        };

        for signer in 0_u32..2 {
            let authenticated = adapter
                .authenticate(signed_timeout(signer))
                .expect("authenticate timeout vote");
            adapter
                .receive_authenticated(authenticated)
                .expect("admit timeout share before quorum");
        }
        let original_tag = adapter.current_tag();
        adapter
            .defer_body_available_for_test(original_tag, &manifest)
            .expect("stage Busy-deferred body completion");
        let authenticated = adapter
            .authenticate(signed_timeout(2))
            .expect("authenticate quorum timeout vote");
        let final_effects = adapter
            .receive_authenticated(authenticated)
            .expect("form and install TC before draining the old completion")
            .into_effects();
        let rebound_tag = final_effects
            .iter()
            .find_map(|effect| match effect {
                AdapterEffect::EnterView {
                    tag,
                    protected_body: Some(protected),
                    ..
                } if *protected == (round, subject) => Some(*tag),
                _ => None,
            })
            .expect("effective-lock EnterView effect");

        let started = Instant::now();
        let (runtime, startup_effects) = SerializedV2Runtime::new(
            adapter,
            final_effects.clone(),
            started,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
        )
        .expect("serialized production runtime");
        assert_eq!(startup_effects, final_effects);
        let mut executor = V2EffectExecutor::with_runtime(
            runtime,
            BTreeMap::new(),
            context,
            PeerId::new(keys[3].public_key().clone()),
            None,
            EffectQueueConfig::default(),
        )
        .expect("serialized production executor");
        executor.ready_body_bytes = u64::try_from(body.len()).expect("body length");
        executor.ready_bodies.insert(
            (round, subject),
            ReadyBody {
                manifest: manifest.clone(),
                bytes: body.into(),
            },
        );
        executor.body_pipeline_owners.insert(
            (round, subject),
            BodyPipelineOwner {
                tag: original_tag,
                manifest_hash: Some(HashOf::new(&manifest)),
            },
        );
        let mut services = FakeServices::default();
        executor
            .consume_effects(final_effects, &mut services)
            .expect("executor rebinds the deferred completion before later service");
        assert!(services.fetch_tasks.is_empty());
        assert_eq!(
            executor.body_pipeline_owners[&(round, subject)].tag,
            rebound_tag
        );

        executor
            .arm_live_clocks(started)
            .expect("arm clocks after startup effects");
        assert!(matches!(
            executor
                .step(started + Duration::from_secs(2), &mut services)
                .expect("periodic service drains the rebound completion"),
            EffectExecutorStep::Advanced { .. }
        ));
        assert_eq!(services.store_tasks.len(), 1);
        assert_eq!(services.store_tasks[0].tag(), rebound_tag);
        assert_eq!(services.store_tasks[0].manifest(), &manifest);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn tc_body_rebind_cancels_fetch_superseded_by_a_higher_different_qc() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let consumer_tag = |view| EventTag::new(1, view, Generation::new(7 + view));
        let original = fixture.qc(wire::GlobalPhase::Prepare);
        let original_sources = original
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer_tag(0),
                    round: original.round,
                    subject: original.subject,
                    manifest: None,
                    certified_sources: original_sources,
                    certificate: Some(original.clone()),
                }],
                &mut services,
            )
            .expect("begin original protected fetch");
        let original_id = services.fetch_tasks[0].id();

        let mut first_timeout = timeout_at_view(&fixture, 0);
        first_timeout.groups[0].highest_prepare_qc = Some(original.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: consumer_tag(1),
                    certificate: first_timeout,
                    protected_body: Some((original.round, original.subject)),
                }],
                &mut services,
            )
            .expect("retain original exact high-QC fetch");

        let replacement_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replacement high-QC block")),
            payload_hash: Hash::new(b"replacement high-QC payload"),
            ..original.subject
        };
        let replacement = wire::QuorumCertificate {
            round: round(&fixture.context, 1),
            phase: wire::GlobalPhase::Prepare,
            subject: replacement_subject,
            execution_commitment: fixture_execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let mut replacement_timeout = timeout_at_view(&fixture, 1);
        replacement_timeout.groups[0].highest_prepare_qc = Some(replacement.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: consumer_tag(2),
                    certificate: replacement_timeout,
                    protected_body: Some((replacement.round, replacement.subject)),
                }],
                &mut services,
            )
            .expect("higher different QC supersedes old acquisition");

        assert!(executor.pending_fetches.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.cancelled_fetches, vec![original_id]);

        let replacement_sources = replacement
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer_tag(2),
                    round: replacement.round,
                    subject: replacement.subject,
                    manifest: None,
                    certified_sources: replacement_sources,
                    certificate: Some(replacement),
                }],
                &mut services,
            )
            .expect("replacement high-QC fetch claims the released bounded slot");
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.pending_work(), 1);
        assert!(!executor.status().fail_closed);
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
                        protected_body: None,
                    }],
                    &mut services,
                )
                .expect("install next view");
            assert!(executor.pending_fetches.is_empty());
            assert!(executor.outstanding_requests.is_empty());
            assert!(executor.body_pipeline_owners.is_empty());
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
                        protected_body: None,
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
        let recovered_fetch = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        executor
            .consume_effects(vec![recovered_fetch.clone()], &mut services)
            .expect("recover local durable body");
        assert!(services.fetch_tasks.is_empty());
        assert_eq!(executor.runtime.queued_commands(), 1);
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(0) && manifest == &fixture.manifest
        ));
        executor
            .consume_effects(vec![recovered_fetch], &mut services)
            .expect("retransmitted recovery fetch remains idempotent");
        assert_eq!(executor.runtime.queued_commands(), 1);
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
    fn delayed_pending_tip_recovery_allows_only_local_apply_pipeline() {
        let fixture = Fixture::new();
        let directory = TempDir::new().expect("recovery directory");
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("open body store");
        let durable = store
            .store(fixture.manifest.clone(), fixture.body.clone())
            .expect("persist exact decided body");
        let _validated_receipt = store
            .validate(&durable, |_| {
                Ok::<_, &'static str>(fixture_execution_commitment())
            })
            .expect("persist exact deterministic-validation marker");
        drop(store);

        let reopened = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("reopen recovery body store");
        let recovered = reopened.recovery_catalog().expect("recovery catalog");
        let recovered_validations = reopened.validated_recovery_catalog();
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        let mut runtime = FakeRuntime::default();
        runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: None,
                certified_sources: commit
                    .signers
                    .iter()
                    .map(|index| fixture.context.roster[*index as usize].validator.clone())
                    .collect(),
                certificate: Some(commit.clone()),
            }])));
        runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::StoreBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }])));
        runtime.steps.push_back(Ok(RuntimeStep::Advanced(vec![
            AdapterEffect::ValidateBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            },
        ])));
        runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: commit,
            }])));

        let mut executor = V2EffectExecutor::with_runtime(
            runtime,
            recovered,
            fixture.context.clone(),
            PeerId::new(fixture.requester_key.public_key().clone()),
            Some(0),
            EffectQueueConfig::default(),
        )
        .expect("recovered executor");
        executor.validated_bodies = recovered_validations;
        let mut services = FakeServices {
            _body_directory: Some(directory),
            body_store: Some(reopened),
            requester_key: Some(fixture.requester_key.clone()),
            ..FakeServices::default()
        };

        for _ in 0..4 {
            assert!(matches!(
                executor
                    .step_pending_tip_recovery(Instant::now(), &mut services)
                    .expect("advance local-only recovery"),
                EffectExecutorStep::Advanced { effects: 1 }
            ));
        }
        assert_eq!(services.apply_tasks.len(), 1);
        assert!(services.fetch_tasks.is_empty());
        assert!(services.sign_tasks.is_empty());
        assert!(services.broadcasts.is_empty());
        assert!(services.entered_views.is_empty());
        assert!(services.equivocations.is_empty());
        assert!(services.invalid_bodies.is_empty());

        // Model a slow WSV/checkpoint/fsync completion. Repeated idle polling must remain silent,
        // and an accidental reducer broadcast is rejected before reaching the network adapter.
        for _ in 0..3 {
            assert_eq!(
                executor
                    .step_pending_tip_recovery(Instant::now(), &mut services)
                    .expect("wait for delayed local Apply"),
                EffectExecutorStep::Idle
            );
        }
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::Broadcast(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    fixture.qc(wire::GlobalPhase::Commit),
                )),
            )])));
        assert!(matches!(
            executor.step_pending_tip_recovery(Instant::now(), &mut services),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("non-local consensus effect")
        ));
        assert!(services.broadcasts.is_empty());
    }

    #[test]
    fn runtime_step_dispatches_entire_effect_batch_before_returning() {
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
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![
                AdapterEffect::Broadcast(message.clone()),
                AdapterEffect::ReportEquivocation {
                    offender: fixture.context.roster[1].validator.clone(),
                    round: fixture.manifest.round,
                    kind: EquivocationKind::Vote,
                },
                AdapterEffect::ReportInvalidCertifiedBody {
                    subject: fixture.manifest.subject,
                    certificate: fixture.qc(wire::GlobalPhase::Prepare),
                },
            ])));

        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("dispatch complete effect batch"),
            EffectExecutorStep::Advanced { effects: 3 }
        );
        assert_eq!(
            services.effect_service_order,
            vec!["broadcast", "equivocation", "invalid-body"]
        );
        assert_eq!(services.broadcasts, vec![message]);
        assert_eq!(services.equivocations.len(), 1);
        assert_eq!(services.invalid_bodies, vec![fixture.manifest.subject]);
        assert!(
            executor.runtime.steps.is_empty(),
            "the emitted effect batch must have no pending tail"
        );
        assert!(executor.runtime.completions.is_empty());
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("runtime is idle after complete batch dispatch"),
            EffectExecutorStep::Idle
        );
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

    #[test]
    fn restart_required_guard_stops_serialized_runtime_before_any_effect_work() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(Vec::new())));
        let queued_steps = executor.runtime.steps.len();
        executor.output_guard.activate_restart_required();

        assert!(matches!(
            executor.step(Instant::now(), &mut services),
            Err(EffectExecutorError::FailClosed(_))
        ));
        assert_eq!(
            executor.runtime.steps.len(),
            queued_steps,
            "post-latch runtime work must remain completely unconsumed"
        );
        assert!(services.statuses.is_empty());
        assert!(services.sign_tasks.is_empty());
        assert!(services.fetch_tasks.is_empty());
        assert!(services.store_tasks.is_empty());
        assert!(services.validation_tasks.is_empty());
        assert!(services.apply_tasks.is_empty());
    }

    #[test]
    fn failed_initial_local_store_admission_does_not_publish_pipeline_owner() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("store");

        assert!(
            executor
                .admit_local_proposal(
                    tag(0),
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.store_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_new_uncertified_fetch_admission_preserves_exact_projection() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
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
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.fetch_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_new_certified_fetch_admission_preserves_request_indexes() {
        let fixture = Fixture::new();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
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
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.fetch_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_existing_certified_fetch_retransmission_preserves_exact_projection() {
        let fixture = Fixture::new();
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
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(vec![effect.clone()], &mut services)
            .expect("admit initial certified fetch");
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
            executor
                .consume_effects(vec![effect], &mut services)
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_existing_fetch_certificate_upgrade_preserves_request_indexes() {
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
            .expect("admit ordinary fetch");
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: sources,
                        certificate: Some(prepare),
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_staged_exact_body_runtime_admission_preserves_ready_owner() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(&fixture.context, key.0, key.1, fixture.body.clone())
            .expect("derive ready body");
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.ready_body_bytes = u64::try_from(ready.bytes.len()).expect("body length");
        executor.ready_bodies.insert(key, ready);
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn failed_staged_conflict_replacement_preserves_ready_bytes() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(&fixture.context, key.0, key.1, fixture.body.clone())
            .expect("derive staged body");
        let mut conflicting = fixture.manifest.clone();
        conflicting.payload_size_bytes = conflicting
            .payload_size_bytes
            .checked_add(1)
            .expect("small body");
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.ready_body_bytes = u64::try_from(ready.bytes.len()).expect("body length");
        executor.ready_bodies.insert(key, ready);
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(conflicting),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.fetch_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_retained_locked_body_runtime_admission_preserves_exact_projection() {
        let fixture = Fixture::new();
        let retained: Arc<[u8]> = fixture.body.clone().into();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.retained_locked_body = Some((fixture.manifest.subject, retained));
        executor.ready_body_bytes =
            u64::try_from(fixture.body.len()).expect("retained body length");
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
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
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn late_fetch_conflict_does_not_fill_pipeline_owner_manifest() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(&fixture.context, key.0, key.1, fixture.body.clone())
            .expect("derive retained body");
        let mut conflicting = fixture.manifest.clone();
        conflicting.payload_size_bytes = conflicting
            .payload_size_bytes
            .checked_add(1)
            .expect("small body");
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.ready_body_bytes = u64::try_from(ready.bytes.len()).expect("body length");
        executor.ready_bodies.insert(key, ready);
        executor.body_pipeline_owners.insert(
            key,
            BodyPipelineOwner {
                tag: tag(0),
                manifest_hash: None,
            },
        );
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(conflicting),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
    }

    #[test]
    fn failed_detached_store_runtime_admission_preserves_exact_projection() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let id = executor.allocate_work_id().expect("allocate store work");
        let task = BodyStoreTask {
            id,
            tag: tag(0),
            manifest: fixture.manifest.clone(),
            canonical_wire: Arc::from(fixture.body.clone()),
        };
        executor.pending_store_bytes =
            u64::try_from(task.canonical_wire.len()).expect("body length");
        executor.pending_stores.insert(
            id,
            PendingStore {
                task,
                consumer: None,
            },
        );
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn failed_recovered_body_runtime_admission_preserves_durable_catalogue() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let receipt = services
            .body_store
            .as_mut()
            .expect("body store")
            .store(fixture.manifest.clone(), fixture.body.clone())
            .expect("persist recovery body");
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), receipt));
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn successful_new_certified_fetch_commits_exact_task_and_request_once() {
        let fixture = Fixture::new();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();

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
            .expect("admit certified fetch");
        let task = services.fetch_tasks.first().expect("fetch task");
        let request_hash = HashOf::new(task.certified_request().expect("certified request"));
        assert_eq!(executor.pending_fetches[&task.id()].task, *task);
        assert_eq!(
            executor.outstanding_requests.hashes(),
            BTreeSet::from([request_hash])
        );
        assert_eq!(
            executor.certified_work,
            BTreeMap::from([(request_hash, task.id())])
        );
    }

    #[test]
    fn successful_fetch_certificate_upgrade_commits_exact_delta_once() {
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
            .expect("admit ordinary fetch");
        let id = services.fetch_tasks[0].id();
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
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("upgrade fetch authority");
        let upgraded = services.fetch_tasks.last().expect("upgraded task");
        let request_hash = HashOf::new(upgraded.certified_request().expect("certified request"));
        assert_eq!(upgraded.id(), id);
        assert_eq!(executor.pending_fetches[&id].task, *upgraded);
        assert_eq!(
            executor.outstanding_requests.hashes(),
            BTreeSet::from([request_hash])
        );
        assert_eq!(
            executor.certified_work,
            BTreeMap::from([(request_hash, id)])
        );
    }

    #[test]
    fn successful_staged_conflict_retires_old_ready_only_after_fetch_admission() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(&fixture.context, key.0, key.1, fixture.body.clone())
            .expect("derive staged body");
        let old_ready_bytes = u64::try_from(ready.bytes.len()).expect("body length");
        let mut incoming = fixture.manifest.clone();
        incoming.payload_size_bytes = incoming
            .payload_size_bytes
            .checked_add(1)
            .expect("small body");
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.ready_body_bytes = old_ready_bytes;
        executor.ready_bodies.insert(key, ready);
        let mut services = fixture.services();

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: key.0,
                    subject: key.1,
                    manifest: Some(incoming.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("admit replacement fetch");
        assert!(!executor.ready_bodies.contains_key(&key));
        assert_eq!(executor.ready_body_bytes, 0);
        let task = services.fetch_tasks.first().expect("replacement fetch");
        assert_eq!(task.manifest(), Some(&incoming));
        assert_eq!(executor.pending_fetches[&task.id()].task, *task);
        assert_eq!(
            executor.body_pipeline_owners[&key].manifest_hash,
            Some(HashOf::new(&incoming))
        );
    }

    #[test]
    fn successful_ready_store_handoff_shares_exact_bytes_without_copy() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        let ready_bytes = Arc::clone(&executor.ready_bodies[&key].bytes);

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: key.0,
                    subject: key.1,
                }],
                &mut services,
            )
            .expect("admit store");
        let queued = services.store_tasks.first().expect("queued store");
        let pending = &executor.pending_stores[&queued.id()].task;
        assert!(Arc::ptr_eq(&ready_bytes, &queued.canonical_wire));
        assert!(Arc::ptr_eq(&ready_bytes, &pending.canonical_wire));
        assert_eq!(executor.ready_body_bytes, 0);
        assert_eq!(
            executor.pending_store_bytes,
            u64::try_from(ready_bytes.len()).expect("body length")
        );
    }

    #[test]
    fn runtime_wal_step_panic_latches_restart_required_before_callbacks() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.runtime.panic_step = true;
        let output_guard = Arc::clone(&executor.output_guard);
        let mut services = fixture.services();

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = executor.step(Instant::now(), &mut services);
        }));

        assert!(unwind.is_err());
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
        assert!(services.statuses.is_empty());
        assert!(services.sign_tasks.is_empty());
        assert!(services.fetch_tasks.is_empty());
        assert!(services.store_tasks.is_empty());
    }

    #[test]
    fn failed_body_available_admission_preserves_exact_body_owners() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (work_id, ready) = executor
            .stage_body_fetch_for_test(&fixture)
            .expect("stage exact fetch");
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert_eq!(
            executor.finish_fetch(work_id, ready, &mut services),
            Err(EffectTransportError::Backpressure)
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
        assert!(executor.fatal_reason.is_none());
    }

    #[test]
    fn failed_store_admission_preserves_ready_owner_and_accounting() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        let before = executor.body_ownership_projection();
        services.fail_on = Some("store");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::StoreBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.store_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_body_stored_runtime_admission_preserves_pending_store() {
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
            .expect("admit store");
        let store_id = services.store_tasks.last().expect("store task").id();
        let completion = services.execute_store(store_id);
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .complete_body_store(completion, &mut services)
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn failed_local_validation_admission_preserves_pending_store() {
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
            .expect("admit local store");
        let store_id = services.store_tasks.last().expect("store task").id();
        let completion = services.execute_store(store_id);
        let before = executor.body_ownership_projection();
        services.fail_on = Some("validation");

        assert!(
            executor
                .complete_body_store(completion, &mut services)
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.validation_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_validation_admission_preserves_durable_owner() {
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
            .expect("admit store");
        let store_id = services.store_tasks.last().expect("store task").id();
        let completion = services.execute_store(store_id);
        executor
            .complete_body_store(completion, &mut services)
            .expect("record durable body");
        let before = executor.body_ownership_projection();
        services.fail_on = Some("validation");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::ValidateBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.validation_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_validation_completion_admission_preserves_pending_validation() {
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
            .expect("admit store");
        let store_id = services.store_tasks.last().expect("store task").id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("record durable body");
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("admit validation");
        let validation_id = services
            .validation_tasks
            .last()
            .expect("validation task")
            .id();
        let completion = services.execute_validation(validation_id);
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .complete_body_validation(completion, &mut services)
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn successful_ready_store_validation_handoff_has_one_exact_owner() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        let ready_bytes = executor.ready_body_bytes;
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: key.0,
                    subject: key.1,
                }],
                &mut services,
            )
            .expect("admit store");
        assert!(!executor.ready_bodies.contains_key(&key));
        assert_eq!(executor.ready_body_bytes, 0);
        assert_eq!(executor.pending_stores.len(), 1);
        assert_eq!(executor.pending_store_bytes, ready_bytes);
        let store_id = services.store_tasks.last().expect("store task").id();
        assert_eq!(
            executor.pending_stores[&store_id].task,
            services.store_tasks[0]
        );

        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("complete store");
        assert!(executor.pending_stores.is_empty());
        assert_eq!(executor.pending_store_bytes, 0);
        assert!(executor.durable_bodies.contains_key(&key));
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyStored(completion_tag, round, subject, _))
                if *completion_tag == tag(0) && (*round, *subject) == key
        ));

        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: key.0,
                    subject: key.1,
                }],
                &mut services,
            )
            .expect("admit validation");
        assert_eq!(executor.pending_validations.len(), 1);
        let validation = services.validation_tasks.last().expect("validation task");
        assert_eq!(
            executor.pending_validations[&validation.id()].task,
            *validation
        );

        // Missing-sidecar completion is still a validation completion: it
        // cannot mutate deferred ownership or call recovery services before
        // the immutable consumer owner is checked. Build the state through
        // the production validation admission path, then corrupt only that
        // owner projection.
        for corruption in ["missing", "mismatched", "work-id", "orphan"] {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let (pending, reference, _) = pending_merge_validation(&fixture);
            let round = pending.task.round();
            let subject = pending.task.subject();
            let task = begin_reachable_merge_validation(
                &fixture,
                &mut executor,
                &mut services,
                round,
                subject,
            );
            let key = (round, subject);
            match corruption {
                "missing" => {
                    executor.body_pipeline_owners.remove(&key);
                }
                "mismatched" => {
                    executor
                        .body_pipeline_owners
                        .get_mut(&key)
                        .expect("reachable validation owner")
                        .tag = EventTag::new(1, round.view, Generation::new(8));
                }
                "work-id" => {
                    executor
                        .pending_validations
                        .get_mut(&task.id())
                        .expect("reachable pending validation")
                        .task
                        .id = EffectWorkId(999);
                }
                "orphan" => {
                    executor.durable_bodies.remove(&key);
                }
                _ => unreachable!("the test enumerates exact owner corruptions"),
            }
            let before = executor.body_ownership_projection();

            let error = executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id: task.id(),
                        reference,
                    },
                    &mut services,
                )
                .expect_err("corrupt validation owner must fail closed");
            assert!(matches!(
                error,
                EffectExecutorError::Contract(_) | EffectExecutorError::BodyStore(_)
            ));
            assert_eq!(executor.body_ownership_projection(), before);
            assert!(executor.deferred_merge_work.is_empty());
            assert!(services.deferred_merge_sidecars.is_empty());
        }
    }

    impl V2EffectExecutor<FakeRuntime> {
        fn stage_body_fetch_for_test(
            &mut self,
            fixture: &Fixture,
        ) -> Result<(EffectWorkId, ReadyBody), EffectExecutorError> {
            let id = self.allocate_work_id()?;
            self.bind_body_pipeline_owner(tag(0), &fixture.manifest)?;
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
            let ready_body = ReadyBody::derive(
                &self.context,
                fixture.manifest.round,
                fixture.manifest.subject,
                fixture.body.clone(),
            )
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
            Ok((id, ready_body))
        }

        fn admit_ready_body_for_test(
            &mut self,
            fixture: &Fixture,
            services: &mut FakeServices,
        ) -> Result<(), EffectExecutorError> {
            let (id, ready_body) = self.stage_body_fetch_for_test(fixture)?;
            self.finish_fetch(id, ready_body, services)
                .map(|_| ())
                .map_err(|error| EffectExecutorError::Contract(error.to_string()))
        }
    }
}
