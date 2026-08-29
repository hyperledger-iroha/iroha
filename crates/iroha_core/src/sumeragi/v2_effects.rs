//! Fail-closed execution boundary for Sumeragi v2 reducer effects.
//!
//! [`SerializedV2Runtime`] is the only owner of consensus state. This module
//! does not select leaders, count votes, form certificates, change views, or
//! decide blocks. It turns each [`AdapterEffect`] into explicit work at the
//! networking, signing, exact-body, lifecycle-owned validation, application,
//! status, and evidence boundaries. View-specific consumers retain their exact
//! [`EventTag`], while immutable persistence and validation work can be rebound
//! after a certified view transition.
//! The caller must explicitly select the exact-body signature policy: the
//! configured genesis authority at height one or the context's rotating leader
//! thereafter. The executor forwards that policy to the body store and still
//! routes full semantic block validation through the deterministic validator;
//! it never invents a second block-authorization rule.
//! Exact-body fsync executes as a tagged asynchronous task, but its immutable
//! storage operation is separate from the current reducer consumer. Canonical
//! decoding and deterministic validation execute against an immutable durable
//! receipt through the lifecycle registry. Only [`V2BodyStore`] can mint
//! durability and validation evidence, so networking code cannot acknowledge
//! either boundary.
//!
//! # Worker integration contract
//!
//! 1. Production opens [`V2BodyStore`] first, validates its recovery catalog
//!    against the durable ingress gate, constructs the adapter/runtime, then
//!    calls [`V2EffectExecutor::open_with_body_store`]. At height one, retain
//!    the already-authenticated staged genesis with
//!    [`V2EffectExecutor::install_authenticated_genesis_body`] before
//!    dispatching startup effects. Move the returned [`V2BodyStore`] to the
//!    storage/validation service thread. If
//!    recovery reported an interrupted canonical Kura tip, call
//!    [`V2EffectExecutor::verify_pending_kura_apply_replay`] before dispatching
//!    startup effects or opening ingress. Drain that local replay only through
//!    [`V2EffectExecutor::step_pending_tip_recovery`] while live clocks remain
//!    unarmed; the finalized runtime is then consumed. For a normal height,
//!    call [`V2EffectExecutor::arm_live_clocks`] exactly once after every
//!    constructor and startup effect and immediately before opening ingress.
//! 2. Route control envelopes through
//!    [`V2EffectExecutor::enqueue_network_with_ingress_ownership`]
//!    and payload traffic through the authenticated chunk/certified-response
//!    methods in this module.
//! 3. Repeatedly call [`V2EffectExecutor::step`] and execute every task handed
//!    to [`V2EffectServices`].
//! 4. Execute [`BodyStoreTask`] through [`V2BodyStore::execute_store_task`]
//!    and route every durable Validate owner through the lifecycle registry.
//!    The production validation callback is
//!    `ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block`; it admits
//!    only bodies which pass the shared deterministic
//!    transaction/internal/time-trigger work gate and is not interchangeable.
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
#[cfg(test)]
use super::v2_body_store::BlockSignaturePolicy;
use super::v2_core::{
    CanonicalIdentityProjection, CheckedProductionTransition, EFFECTIVE_LOCK_TRACE_OWNER,
    EFFECTIVE_LOCK_TRACE_RETIRE, EffectiveLockTraceProjection, EventTag, ExactBodyOwnerProjection,
    ExactBodyRetirementAccounting, IDENTITY_DOMAIN_CONTEXT, IDENTITY_DOMAIN_DURABLE_ARTIFACT,
    IDENTITY_DOMAIN_PAYLOAD, IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_BLOCK_HEADER,
    IDENTITY_KIND_CANONICAL_PAYLOAD, IDENTITY_KIND_CONSENSUS_MESSAGE,
    IDENTITY_KIND_DURABLE_BODY_FRAME, IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
    IDENTITY_KIND_EXECUTION_COMMITMENT, IDENTITY_KIND_PAYLOAD_MANIFEST,
    IDENTITY_KIND_QUORUM_CERTIFICATE, IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
    IDENTITY_KIND_WIRE_HEIGHT_CONTEXT, MAX_EFFECTS_PER_STEP, ProductionDecisionIdentityProjection,
    ProductionDecisionRecoveryTraceProjection, ProductionDurableBodyIdentityProjection,
    ProductionQuorumCertificateIdentityProjection, SERVICE_CLASS_PROGRESS, TagProjection,
    check_production_body_capacity_retirement_effective_lock_transition,
    check_production_body_ownership_effective_lock_transition,
    check_production_decision_recovery_transition, check_production_effect_to_candidate_transition,
    exact_body_stage_is_owned, plan_exact_body_owner_binding, plan_exact_body_owner_rebind,
    plan_exact_body_retirement_accounting,
};
#[cfg(test)]
use super::v2_runtime::{RuntimeSelectedOwnerKind, bind_adapter_effect_batch_ownership};
#[cfg(test)]
use super::v2_transport::authenticate_certified_body_request;
use super::{
    FairV2IngressLeaderWireToken, FairV2IngressOwnershipEvidence,
    message::BlockMessage,
    output_guard::ConsensusOutputGuard,
    v2::{
        AdapterEffect, AdapterError, LifecycleDecisionApplyAdapterCompletionAuthorityV1,
        LifecycleDecisionApplyAdapterFinalityV1, LiveProposalIntentWalSignHandoffV1,
        PreparedLifecycleDecisionApplyAdapterCompletionV1,
        RecoveredLifecycleNextVoteBodyAuthorityV1, RecoveredLifecycleNextVoteBodyLookupV1,
        SignRequest, VerifiedHeightContext,
    },
    v2_body_store::{
        BodyStoreCompletion, DurableBodyReceipt, V2BodyStore, V2BodyStoreInstanceIdentity,
        ValidatedBodyReceipt,
    },
    v2_chunks::{V2ChunkError, encode_payload},
    v2_first_release_recovery::{
        LocalProposalIntentReplayEvidenceV1, LocalProposalReadyReplayEvidenceV1,
    },
    v2_lifecycle_coordinator::{
        AdmissionDecision, AttestedLifecycleDecisionApplySuccessorOutputsV1,
        DurableStoreTerminalRetrySealV1, InstalledAuthenticatedGenesisReplayAuthorityV1,
        LifecycleContext, LifecycleDecisionApplyDispatchKeyV1, LifecycleDecisionApplyLineageV1,
        LifecycleOutputAdmissionKeyV1, LifecycleOutputServiceDispositionV1,
        LifecycleValidateDispatchKeyV1, LiveLifecycleDecisionApplyReconciliationAuthorityV1,
        PendingDurableValidateAdmissionV1, PendingLifecycleOutputAdmissionV1,
        PendingLiveWalSignAdmissionV1, PreparedAuthenticatedGenesisFetchReplayPreAdmission,
        PreparedAuthenticatedGenesisStoreReplayPreAdmission,
        PreparedAuthenticatedGenesisStoredReplayPreAdmission,
        PreparedLifecycleDecisionApplyDispatchV1, PreparedLocalBodyValidateReplayPreAdmission,
        PreparedRemoteProposalFetchReplayPreAdmission,
        PreparedRemoteProposalStoreReplayPreAdmission,
        PreparedRemoteProposalStoredReplayPreAdmission,
        ProductionDurableValidateAdmissionSettlementV1,
        ProductionLifecycleLiveClockActivationPermitV1,
        ProductionLifecycleOutputAdmissionFailureV1,
        ProductionLifecycleOutputAdmissionSettlementV1, ProductionLifecycleOwnerV1,
        ProductionLiveWalSignAdmissionFailureV1, ProductionLiveWalSignAdmissionSettlementV1,
        RecoveredDurableValidateRetryCensusV1, RecoveredDurableValidateRetryOwnerV1,
    },
    v2_recovery::PendingKuraApply,
    v2_runtime::{
        BodyAvailableReservation, DecisionProposalRetirement, EnqueueError,
        LeaderWireRuntimeTerminal, LocalProposalEffectOwnership, LocalProposalReadyCommandIdentity,
        NetworkIngressError, PendingRuntimeEffectBinding, PendingRuntimeEffectFingerprintV1,
        PreTimeoutLockedPrepareQcCutV1, RecoveredDurableValidateRetryFrontierV1,
        RetiredBodyPipelineCompletions, RuntimeCandidateAdmissionDisposition,
        RuntimeCandidateSemanticStatement, RuntimeClockError, RuntimeEffectOwnership,
        RuntimeFetchAuthorityRelation, RuntimeLifecycleOwner, RuntimeQueueLaneSnapshot,
        RuntimeQueueSnapshot, RuntimeStep, SerializedV2Runtime,
        production_adapter_effect_candidate_admission_disposition,
        production_adapter_effect_candidate_semantic_identity,
        production_adapter_effect_candidate_trace_projection,
    },
    v2_transport::{
        AuthenticatedCertifiedBodyRequest, AuthenticatedCertifiedBodyResponse,
        AuthenticatedPayloadChunk, CertifiedBodyRequestRegistrationPlan,
        CertifiedBodyRequestRetirementPlan, CertifiedBodyResponseClaimPreflight,
        OutstandingCertifiedBodyRequests, V2TransportError,
        authenticate_certified_body_request_with_live_adapter, authenticate_payload_chunk,
    },
    v2_worker::RecoveredDecisionFetchRequestOwnerV1,
};
use crate::kura::KuraV2CommitReceipt;
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    block::{BlockHeader, CertifiedMergeLedgerReference, consensus_v2 as wire},
    merge::MergeLedgerEntry,
    peer::PeerId,
};
#[cfg(test)]
use norito::codec::Encode as _;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry},
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

/// Whether one exact fair-ingress occurrence may cross retained reducer debt.
///
/// This pure predicate is shared by ordinary checked dequeue and lifecycle
/// queue selection. Current-height certified-Serve preparation remains a
/// separate, stateful runner/service transaction after this common gate.
pub(crate) fn v2_ingress_head_can_drain<R: EffectRuntime>(
    inbound: &super::InboundBlockMessage,
    executor: &V2EffectExecutor<R>,
    terminal_subject: Option<wire::BlockSubject>,
) -> bool {
    let BlockMessage::V2(message) = inbound.message() else {
        return true;
    };
    if message.validate_version().is_err() {
        return true;
    }
    if terminal_subject.is_some() && v2_payload_is_terminal_reducer_control(&message.payload) {
        return true;
    }
    if let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = &message.payload
        && certified_body_request_is_superseded_after_decision(
            request,
            terminal_subject,
            executor.context().height,
        )
    {
        return true;
    }
    let Some(ingress_ownership) = inbound.ingress_ownership() else {
        return true;
    };
    if let wire::ConsensusMessageV2Payload::PayloadChunk(chunk) = &message.payload {
        if terminal_subject.is_some() {
            // The post-Decision consumer terminalizes unmatched chunks
            // without opening a new body owner.
            return true;
        }
        if !executor.payload_chunk_ingress_can_drain(chunk.manifest_hash, ingress_ownership) {
            return false;
        }
    }
    executor.can_admit_network_message_with_ingress_ownership(message, ingress_ownership)
}

/// Return whether finality makes one competing same-height body request obsolete.
pub(crate) fn certified_body_request_is_superseded_after_decision(
    request: &wire::CertifiedBodyRequest,
    terminal_subject: Option<wire::BlockSubject>,
    active_height: wire::Height,
) -> bool {
    terminal_subject
        .is_some_and(|decided| request.round.height == active_height && request.subject != decided)
}

/// Return whether one payload can directly advance or close reducer finality.
pub(crate) const fn v2_payload_is_terminal_reducer_control(
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    matches!(
        payload,
        wire::ConsensusMessageV2Payload::Proposal(_)
            | wire::ConsensusMessageV2Payload::Vote(_)
            | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutVote(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
    )
}

/// Return whether one authenticated envelope can retire a hung signing fence.
///
/// Only a TC or a CommitQC changes the reducer incarnation strongly enough to
/// supersede an outstanding local signature. A PrepareQC can use the separate
/// protected pacemaker Progress turn, but is not a certified signing-fence
/// escape. Ordinary ingress remains behind retained reducer-effect debt. A
/// discovery response carries the same fence authority only when its embedded
/// certificate is a CommitQC.
pub(crate) const fn network_ingress_is_certified_fence_escape(
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    match payload {
        wire::ConsensusMessageV2Payload::TimeoutCertificate(_) => true,
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
            matches!(certificate.phase, wire::GlobalPhase::Commit)
        }
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
            matches!(response.certificate.phase, wire::GlobalPhase::Commit)
        }
        wire::ConsensusMessageV2Payload::Proposal(_)
        | wire::ConsensusMessageV2Payload::Vote(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        | wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => false,
    }
}
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
/// One-shot proof that an exact discovered CommitQC entered serialized reducer ingress.
///
/// Fields and the production constructor remain private to this module. Block-sync discovery can
/// therefore retire an authenticated request only after the real effect executor accepted the
/// exact message; a generic callback returning `Ok(())` cannot claim reducer admission.
#[derive(Debug)]
#[must_use]
pub(crate) struct CommitCertificateReducerAdmission {
    message_hash: HashOf<wire::ConsensusMessageV2>,
}
impl CommitCertificateReducerAdmission {
    /// Return whether this admission was minted for the complete canonical message.
    pub(crate) fn matches(&self, message: &wire::ConsensusMessageV2) -> bool {
        self.message_hash == HashOf::new(message)
    }
    /// Losslessly project the admitted canonical envelope for the shared
    /// historical-certificate refinement gate.
    pub(crate) fn refinement_projection(&self) -> CanonicalIdentityProjection {
        canonical_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CONSENSUS_MESSAGE,
            self.message_hash,
        )
    }
    /// Construct deterministic ownership evidence for block-sync boundary tests.
    #[cfg(test)]
    pub(crate) fn for_test(message: &wire::ConsensusMessageV2) -> Self {
        Self {
            message_hash: HashOf::new(message),
        }
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
    #[cfg(test)]
    pub(crate) const fn hash(self) -> Hash {
        self.hash
    }
}
/// Exact closed-ingress stage of an interrupted canonical-tip recovery.
///
/// Each variant names the sole reducer effect which may be dispatched next.
/// `ApplicationDispatched` and `Completed` are terminal with respect to
/// reducer effects: only the matching typed Kura completion may cross the
/// former boundary, and no height-local work may cross the latter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) enum PendingKuraApplyRecoveryStage {
    /// The authenticated replay must dispatch its certified body fetch.
    CertifiedFetch,
    /// The recovered exact body must cross the reducer's durable-store stage.
    DurableStore,
    /// The durable validation marker must cross deterministic validation.
    DeterministicValidation,
    /// The exact replayed CommitQC must dispatch application.
    Apply,
    /// The unique matching Apply is owned by the durable I/O worker.
    ApplicationDispatched,
    /// Kura returned and the executor accepted the exact finality completion.
    Completed,
}
/// Result of the latest bounded interrupted-tip recovery scheduler attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) enum PendingTipRecoveryAttemptResult {
    /// The serialized reducer or retained effect suffix advanced.
    Advanced,
    /// No reducer transition was ready while asynchronous local work remained.
    Waiting,
    /// The exact durable application completion crossed the effect boundary.
    Completed,
    /// The runner exhausted its closed-ingress recovery deadline.
    DeadlineExceeded,
}
fn canonical_typed_identity<T>(
    domain: u8,
    kind: u8,
    hash: HashOf<T>,
) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}
fn canonical_hash_identity(domain: u8, kind: u8, hash: Hash) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}
fn recovery_decision_projection(
    certificate: &wire::QuorumCertificate,
) -> ProductionDecisionIdentityProjection {
    ProductionDecisionIdentityProjection {
        context_id: canonical_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            certificate.round.context_id.0,
        ),
        height: certificate.round.height,
        view: certificate.round.view,
        proposal_height: certificate.proposal_round.height,
        proposal_view: certificate.proposal_round.view,
        phase: certificate.phase as u8,
        subject: canonical_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
            HashOf::new(&certificate.subject),
        ),
        block_hash: canonical_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            certificate.subject.block_hash,
        ),
        payload_hash: canonical_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            certificate.subject.payload_hash,
        ),
        execution_commitment: canonical_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTION_COMMITMENT,
            HashOf::new(&certificate.execution_commitment),
        ),
        executed_block_wire_hash: canonical_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
            certificate.execution_commitment.executed_block_wire_hash,
        ),
    }
}
fn recovery_certificate_projection(
    certificate: &wire::QuorumCertificate,
) -> Option<ProductionQuorumCertificateIdentityProjection> {
    Some(ProductionQuorumCertificateIdentityProjection {
        decision: recovery_decision_projection(certificate),
        certificate: canonical_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_QUORUM_CERTIFICATE,
            HashOf::new(certificate),
        ),
        signer_count: u64::try_from(certificate.signers.len()).ok()?,
        aggregate_signature_len: u64::try_from(certificate.aggregate_signature.len()).ok()?,
    })
}
fn recovery_body_projection(
    durable: &DurableBodyReceipt,
) -> ProductionDurableBodyIdentityProjection {
    let subject = durable.subject();
    ProductionDurableBodyIdentityProjection {
        context_id: canonical_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            durable.context_id().0,
        ),
        height: durable.round().height,
        view: durable.round().view,
        subject: canonical_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
            HashOf::new(&subject),
        ),
        block_hash: canonical_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            subject.block_hash,
        ),
        payload_hash: canonical_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            subject.payload_hash,
        ),
        manifest: canonical_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_PAYLOAD_MANIFEST,
            durable.manifest_hash(),
        ),
        frame: canonical_hash_identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_DURABLE_BODY_FRAME,
            durable.frame_hash(),
        ),
    }
}
/// Lossless native evidence binding one interrupted Kura tip to replay.
///
/// This process-local type is deliberately not serializable. It retains the
/// complete wire certificate (including canonical signer order and aggregate
/// signature), complete manifest, and both typed body receipts. Hash-mediated
/// links retain the repository's native 256-bit values unchanged and rely on
/// the reviewed collision-resistance contract rather than truncation or a
/// synthetic numeric projection.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
#[must_use]
pub(crate) struct PendingKuraApplyRecoveryEvidence {
    expected: PendingKuraApply,
    frozen_context_id: wire::HeightContextId,
    frozen_height: wire::Height,
    replay_tag: EventTag,
    owner_tag: EventTag,
    replay_generation: u64,
    commit_qc: wire::QuorumCertificate,
    manifest: wire::PayloadManifest,
    manifest_hash: HashOf<wire::PayloadManifest>,
    durable_receipt: DurableBodyReceipt,
    durable_frame_hash: Hash,
    validated_receipt: ValidatedBodyReceipt,
    deferred_validated_marker: Option<super::v2::DeferredPendingKuraValidatedMarkerV1>,
    stage: PendingKuraApplyRecoveryStage,
}

/// Executor-private permit for consuming one committed pending-Kura Apply child.
pub(in crate::sumeragi) struct PendingKuraApplySuccessorExecutorPermitV1 {
    _private: (),
}
impl PendingKuraApplySuccessorExecutorPermitV1 {
    fn new() -> Self {
        Self { _private: () }
    }
}

/// Private mint permit for one released lifecycle validation marker.
pub(in crate::sumeragi) struct ReleasedLifecycleValidatedMarkerSealPermitV1 {
    _private: (),
}

/// Durable resolution of one lifecycle-owned Validate row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum LifecycleValidateRetryResolutionV1 {
    /// A certified newer view retired an unprotected missing-sidecar row.
    Cancelled,
    /// The row terminalized without a successor and may authenticate a later
    /// current-Decision standalone Apply.
    AdvancedNoSuccessor,
    /// The row published an adjacent Sign, report, or Apply successor.
    AdvancedToSuccessor,
}
impl ReleasedLifecycleValidatedMarkerSealPermitV1 {
    fn new() -> Self {
        Self { _private: () }
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test() -> Self {
        Self { _private: () }
    }
}

/// Closed direct-validation successors accepted by executor dispatch.
enum DirectValidatedApplySuccessorV1 {
    PendingKura(super::v2::PendingKuraValidatedApplySuccessorV1),
}
impl PendingKuraApplyRecoveryEvidence {
    /// Canonical Kura tip selected by startup recovery.
    pub(crate) const fn expected(&self) -> PendingKuraApply {
        self.expected
    }
    /// Frozen height-context identifier independently reconstructed at startup.
    pub(crate) const fn frozen_context_id(&self) -> wire::HeightContextId {
        self.frozen_context_id
    }
    /// Frozen consensus height independently reconstructed at startup.
    pub(crate) const fn frozen_height(&self) -> wire::Height {
        self.frozen_height
    }
    /// Reducer incarnation which owns every recovery effect.
    pub(crate) const fn replay_tag(&self) -> EventTag {
        self.replay_tag
    }
    /// Independently observed reducer incarnation which authorized replay.
    pub(crate) const fn owner_tag(&self) -> EventTag {
        self.owner_tag
    }
    /// Actor-local replay generation, kept separate from consensus view.
    pub(crate) const fn replay_generation(&self) -> u64 {
        self.replay_generation
    }
    /// Complete authenticated CommitQC, including signers and aggregate evidence.
    pub(crate) const fn commit_qc(&self) -> &wire::QuorumCertificate {
        &self.commit_qc
    }
    /// Exact round certified by the CommitQC.
    pub(crate) const fn commit_round(&self) -> wire::ConsensusRound {
        self.commit_qc.round
    }
    /// Immutable proposal-body origin authenticated by the CommitQC.
    pub(crate) const fn proposal_round(&self) -> wire::ConsensusRound {
        self.commit_qc.proposal_round
    }
    /// Exact phase certified by the CommitQC.
    pub(crate) const fn commit_phase(&self) -> wire::GlobalPhase {
        self.commit_qc.phase
    }
    /// Exact subject certified by the CommitQC.
    pub(crate) const fn commit_subject(&self) -> wire::BlockSubject {
        self.commit_qc.subject
    }
    /// Exact deterministic execution result certified by the CommitQC.
    pub(crate) const fn execution_commitment(&self) -> wire::ExecutionCommitment {
        self.commit_qc.execution_commitment
    }
    /// Canonically ordered validator indices carried by the CommitQC.
    pub(crate) fn commit_signers(&self) -> &[wire::ValidatorIndex] {
        &self.commit_qc.signers
    }
    /// Complete aggregate-signature evidence carried by the CommitQC.
    pub(crate) fn commit_aggregate_signature(&self) -> &[u8] {
        &self.commit_qc.aggregate_signature
    }
    /// Complete canonical manifest recovered beside the decided body.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }
    /// Native typed hash of the complete canonical manifest.
    pub(crate) const fn manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.manifest_hash
    }
    /// Receipt for the complete checksummed body frame.
    pub(crate) const fn durable_receipt(&self) -> &DurableBodyReceipt {
        &self.durable_receipt
    }
    /// Frozen context carried by the durable body receipt.
    pub(crate) const fn durable_context_id(&self) -> wire::HeightContextId {
        self.durable_receipt.context_id()
    }
    /// Exact proposal round carried by the durable body receipt.
    pub(crate) const fn durable_round(&self) -> wire::ConsensusRound {
        self.durable_receipt.round()
    }
    /// Exact proposal subject carried by the durable body receipt.
    pub(crate) const fn durable_subject(&self) -> wire::BlockSubject {
        self.durable_receipt.subject()
    }
    /// Manifest identity carried by the durable body receipt.
    pub(crate) const fn durable_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.durable_receipt.manifest_hash()
    }
    /// Complete checksummed frame identity carried by the durable body receipt.
    pub(crate) const fn durable_frame_hash(&self) -> Hash {
        self.durable_frame_hash
    }
    /// Receipt binding deterministic validation to the same exact body frame.
    pub(crate) const fn validated_receipt(&self) -> &ValidatedBodyReceipt {
        &self.validated_receipt
    }
    /// Deterministic execution result carried by the validation marker.
    pub(crate) const fn validated_execution_commitment(&self) -> wire::ExecutionCommitment {
        self.validated_receipt.execution_commitment()
    }
    /// Sole closed-ingress recovery stage currently authorized.
    pub(crate) const fn stage(&self) -> PendingKuraApplyRecoveryStage {
        self.stage
    }
    /// Project every independently retained recovery identity into the pure
    /// shared production/Verus kernel without truncating canonical digests.
    pub(crate) fn recovery_refinement_projection(
        &self,
    ) -> Option<ProductionDecisionRecoveryTraceProjection> {
        let manifest_subject = self.manifest.subject;
        Some(ProductionDecisionRecoveryTraceProjection {
            state_height: self.expected().state_height(),
            expected_context_id: canonical_typed_identity(
                IDENTITY_DOMAIN_CONTEXT,
                IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
                self.expected().context_id().0,
            ),
            expected_height: self.expected().height(),
            expected_block_hash: canonical_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER,
                self.expected().block_hash(),
            ),
            frozen_context_id: canonical_typed_identity(
                IDENTITY_DOMAIN_CONTEXT,
                IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
                self.frozen_context_id().0,
            ),
            frozen_height: self.frozen_height(),
            replay_tag: TagProjection {
                height: self.replay_tag().height(),
                view: self.replay_tag().view(),
                generation: self.replay_tag().generation().get(),
            },
            owner_tag: TagProjection {
                height: self.owner_tag().height(),
                view: self.owner_tag().view(),
                generation: self.owner_tag().generation().get(),
            },
            replay_generation: self.replay_generation(),
            commit_qc: recovery_certificate_projection(self.commit_qc())?,
            manifest_round: TagProjection {
                height: self.manifest.round.height,
                view: self.manifest.round.view,
                generation: 0,
            },
            manifest_subject: canonical_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
                HashOf::new(&manifest_subject),
            ),
            manifest: canonical_typed_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_PAYLOAD_MANIFEST,
                self.manifest_hash(),
            ),
            durable_body: recovery_body_projection(self.durable_receipt()),
            validated_body: recovery_body_projection(self.validated_receipt().durable()),
            validated_execution_commitment: canonical_typed_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_EXECUTION_COMMITMENT,
                HashOf::new(&self.validated_execution_commitment()),
            ),
            stage: match self.stage() {
                PendingKuraApplyRecoveryStage::CertifiedFetch => 1,
                PendingKuraApplyRecoveryStage::DurableStore => 2,
                PendingKuraApplyRecoveryStage::DeterministicValidation => 3,
                PendingKuraApplyRecoveryStage::Apply => 4,
                PendingKuraApplyRecoveryStage::ApplicationDispatched => 5,
                PendingKuraApplyRecoveryStage::Completed => 6,
            },
        })
    }
    /// Check every redundant native identity link against the frozen context.
    pub(crate) fn is_exact(&self, context: &wire::HeightContext) -> bool {
        let deferred_marker_is_exact = match self.stage() {
            PendingKuraApplyRecoveryStage::CertifiedFetch
            | PendingKuraApplyRecoveryStage::DurableStore
            | PendingKuraApplyRecoveryStage::DeterministicValidation => self
                .deferred_validated_marker
                .as_ref()
                .is_some_and(|marker| {
                    marker.exactly_matches_recovery(
                        context,
                        self.expected(),
                        self.replay_tag(),
                        self.manifest(),
                        self.durable_receipt(),
                        self.validated_receipt(),
                        self.commit_qc(),
                    )
                }),
            PendingKuraApplyRecoveryStage::Apply
            | PendingKuraApplyRecoveryStage::ApplicationDispatched
            | PendingKuraApplyRecoveryStage::Completed => self.deferred_validated_marker.is_none(),
        };
        context.id() == self.frozen_context_id()
            && context.height == self.frozen_height()
            && self.expected().context_id() == self.frozen_context_id()
            && self.expected().height() == self.frozen_height()
            && self.expected().state_height() <= self.frozen_height()
            && self
                .frozen_height()
                .saturating_sub(self.expected().state_height())
                <= 1
            && self.expected().block_hash() == self.commit_subject().block_hash
            && self.replay_tag().height() == self.frozen_height()
            && self.replay_tag() == self.owner_tag()
            && self.replay_tag().generation().get() == self.replay_generation()
            && self.commit_phase() == wire::GlobalPhase::Commit
            && self.commit_qc().validate(context).is_ok()
            && !self.commit_signers().is_empty()
            && !self.commit_aggregate_signature().is_empty()
            && self.commit_round().context_id == self.frozen_context_id()
            && self.commit_round().height == self.frozen_height()
            && self.execution_commitment() == self.validated_execution_commitment()
            && self.manifest().validate(context).is_ok()
            && self.manifest().round.context_id == self.frozen_context_id()
            && self.manifest().round.height == self.frozen_height()
            && self.manifest().round == self.proposal_round()
            && self.manifest().subject == self.commit_subject()
            && HashOf::new(self.manifest()) == self.manifest_hash()
            && self.manifest_hash() == self.durable_manifest_hash()
            && self.durable_context_id() == self.frozen_context_id()
            && self.durable_round() == self.manifest().round
            && self.durable_subject() == self.commit_subject()
            && self.durable_receipt().frame_hash() == self.durable_frame_hash()
            && self.validated_receipt().durable() == self.durable_receipt()
            && deferred_marker_is_exact
    }

    fn take_deferred_validated_marker(
        &mut self,
    ) -> Result<super::v2::DeferredPendingKuraValidatedMarkerV1, EffectExecutorError> {
        self.deferred_validated_marker.take().ok_or_else(|| {
            EffectExecutorError::Contract(
                "pending Kura validation lost its move-only deferred marker".to_owned(),
            )
        })
    }

    fn restore_deferred_validated_marker(
        &mut self,
        marker: super::v2::DeferredPendingKuraValidatedMarkerV1,
    ) {
        debug_assert!(self.deferred_validated_marker.is_none());
        self.deferred_validated_marker = Some(marker);
    }

    #[cfg(test)]
    fn enter_apply_stage_for_test(&mut self) {
        assert_eq!(
            self.stage,
            PendingKuraApplyRecoveryStage::CertifiedFetch,
            "test-only Apply projection starts from sealed startup evidence",
        );
        let _marker = self
            .deferred_validated_marker
            .take()
            .expect("test-only Apply projection consumes its deferred marker");
        self.stage = PendingKuraApplyRecoveryStage::Apply;
    }

    #[cfg(test)]
    fn advance_stage_for_test(&mut self, effect: &AdapterEffect) {
        let next = self
            .transition_for_effect(effect)
            .expect("test-only recovery effect advances its exact stage");
        if self.stage == PendingKuraApplyRecoveryStage::DeterministicValidation
            && next == PendingKuraApplyRecoveryStage::Apply
        {
            let _marker = self
                .deferred_validated_marker
                .take()
                .expect("test-only Validate transition consumes its deferred marker");
        }
        self.stage = next;
    }
    fn transition_for_effect(
        &self,
        effect: &AdapterEffect,
    ) -> Result<PendingKuraApplyRecoveryStage, EffectExecutorError> {
        let exact_tag = |tag: EventTag| tag == self.replay_tag;
        let exact_body_key = |round: wire::ConsensusRound, subject: wire::BlockSubject| {
            round == self.manifest.round && subject == self.commit_qc.subject
        };
        let next = match (self.stage, effect) {
            (
                PendingKuraApplyRecoveryStage::CertifiedFetch,
                AdapterEffect::FetchBody {
                    tag,
                    round,
                    subject,
                    manifest,
                    certificate: Some(certificate),
                    ..
                },
            ) if exact_tag(*tag)
                && exact_body_key(*round, *subject)
                && manifest
                    .as_ref()
                    .is_none_or(|manifest| manifest == &self.manifest)
                && certificate == &self.commit_qc =>
            {
                Some(PendingKuraApplyRecoveryStage::DurableStore)
            }
            (
                PendingKuraApplyRecoveryStage::DurableStore,
                AdapterEffect::StoreBody {
                    tag,
                    round,
                    subject,
                },
            ) if exact_tag(*tag) && exact_body_key(*round, *subject) => {
                Some(PendingKuraApplyRecoveryStage::DeterministicValidation)
            }
            (
                PendingKuraApplyRecoveryStage::DeterministicValidation,
                AdapterEffect::ValidateBody {
                    tag,
                    round,
                    subject,
                },
            ) if exact_tag(*tag) && exact_body_key(*round, *subject) => {
                Some(PendingKuraApplyRecoveryStage::Apply)
            }
            (
                PendingKuraApplyRecoveryStage::Apply,
                AdapterEffect::Apply {
                    tag,
                    subject,
                    certificate,
                },
            ) if exact_tag(*tag)
                && *subject == self.commit_qc.subject
                && certificate == &self.commit_qc =>
            {
                Some(PendingKuraApplyRecoveryStage::ApplicationDispatched)
            }
            _ => None,
        };
        next.ok_or_else(|| {
            EffectExecutorError::Contract(
                "interrupted-tip recovery effect does not match its exact authenticated stage"
                    .to_owned(),
            )
        })
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
    ownership: RuntimeEffectOwnership,
}
impl ConsensusSignTask {
    /// Construct an exact signing task for service-boundary unit tests.
    #[cfg(test)]
    pub(crate) fn for_test(id: u64, tag: EventTag, request: SignRequest) -> Self {
        let effect = AdapterEffect::Sign {
            tag,
            request: request.clone(),
        };
        let ownership = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, u128::from(id))],
        )
        .expect("test signing task has one exact candidate")
        .pop()
        .expect("test signing binding contains one owner");
        Self {
            id: EffectWorkId(id),
            tag,
            request,
            ownership,
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
    /// Immutable actor-global lifecycle ordinal retained across asynchronous I/O.
    pub(crate) const fn lifecycle_ordinal(&self) -> u128 {
        self.ownership.owner().lifecycle_ordinal()
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
    ownership: RuntimeEffectOwnership,
}
impl BodyFetchTask {
    /// Construct ordinary chunk-reconstruction work for service-boundary unit tests.
    #[cfg(test)]
    pub(crate) fn ordinary_for_test(
        id: u64,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Self {
        let effect = AdapterEffect::FetchBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        let ownership = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, u128::from(id))],
        )
        .expect("ordinary test fetch has one exact candidate")
        .pop()
        .expect("ordinary test fetch binding contains one owner");
        Self {
            id: EffectWorkId(id),
            tag,
            round: manifest.round,
            subject: manifest.subject,
            manifest: Some(manifest),
            sources: Vec::new(),
            certified_request: None,
            ownership,
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
        let effect = AdapterEffect::FetchBody {
            tag,
            round: certified_request.round,
            subject: certified_request.subject,
            manifest: manifest.clone(),
            certified_sources: sources.clone(),
            certificate: Some(certified_request.certificate.clone()),
        };
        let ownership = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, u128::from(id))],
        )
        .expect("certified test fetch has one exact candidate")
        .pop()
        .expect("certified test fetch binding contains one owner");
        Self {
            id: EffectWorkId(id),
            tag,
            round: certified_request.round,
            subject: certified_request.subject,
            manifest,
            sources,
            certified_request: Some(certified_request),
            ownership,
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
            && self.ownership == other.ownership
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
    /// acquisition authority, proposal round, subject, and work identifier stay
    /// fixed; only the completion tag may advance.
    pub(crate) fn rebind_consumer(&self, tag: EventTag) -> Option<Self> {
        if !tag.strictly_advances(self.tag) {
            return None;
        }
        let previous_effect = self.adapter_effect();
        let mut rebound = self.clone();
        rebound.tag = tag;
        let rebound_effect = AdapterEffect::FetchBody {
            tag,
            round: rebound.round,
            subject: rebound.subject,
            manifest: rebound.manifest.clone(),
            certified_sources: rebound.sources.clone(),
            certificate: rebound
                .certified_request
                .as_ref()
                .map(|request| request.certificate.clone()),
        };
        rebound.ownership = if rebound
            .ownership
            .exact_remote_proposal_fetch_replay(&previous_effect)
            .is_some()
        {
            rebound
                .ownership
                .rebind_fetch_consumer(&previous_effect, &rebound_effect)
                .ok()?
        } else {
            rebound
                .ownership
                .rebind_same_adapter_effect(&rebound_effect)
                .ok()?
        };
        Some(rebound)
    }
    /// Whether `self` is the exact later-incarnation consumer binding of `previous`.
    pub(crate) fn rebinds_consumer_of(&self, previous: &Self) -> bool {
        self.tag.strictly_advances(previous.tag)
            && self.id == previous.id
            && self.round == previous.round
            && self.subject == previous.subject
            && self.manifest == previous.manifest
            && self.sources == previous.sources
            && self.certified_request == previous.certified_request
            && self.ownership == previous.ownership
    }
    /// Canonically ordered frozen-roster archive fetch sources.
    pub(crate) fn sources(&self) -> &[PeerId] {
        &self.sources
    }
    /// Exact signed request for a certified fetch.
    pub(crate) const fn certified_request(&self) -> Option<&wire::CertifiedBodyRequest> {
        self.certified_request.as_ref()
    }
    fn adapter_effect(&self) -> AdapterEffect {
        AdapterEffect::FetchBody {
            tag: self.tag,
            round: self.round,
            subject: self.subject,
            manifest: self.manifest.clone(),
            certified_sources: self.sources.clone(),
            certificate: self
                .certified_request
                .as_ref()
                .map(|request| request.certificate.clone()),
        }
    }
    /// Immutable actor-global lifecycle ordinal retained through reconstruction.
    #[cfg(test)]
    pub(crate) const fn lifecycle_ordinal(&self) -> u128 {
        self.ownership.owner().lifecycle_ordinal()
    }
    fn ownership(&self) -> &RuntimeEffectOwnership {
        &self.ownership
    }
}
/// Tagged exact-body persistence work executed outside the reducer owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BodyStoreTask {
    id: EffectWorkId,
    tag: EventTag,
    manifest: wire::PayloadManifest,
    canonical_wire: Arc<[u8]>,
    ownership: RuntimeEffectOwnership,
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
        let effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let ownership = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, u128::from(id))],
        )
        .expect("test body store has one exact candidate")
        .pop()
        .expect("test body-store binding contains one owner");
        Self {
            id: EffectWorkId::for_test(id),
            tag,
            manifest,
            canonical_wire: Arc::from(canonical_wire),
            ownership,
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
    /// Immutable actor-global lifecycle ordinal retained across asynchronous I/O.
    pub(crate) const fn lifecycle_ordinal(&self) -> u128 {
        self.ownership.owner().lifecycle_ordinal()
    }
    fn ownership(&self) -> &RuntimeEffectOwnership {
        &self.ownership
    }
}
/// Application request for an exact durable, validated decided block.
#[derive(Clone, Debug)]
pub(crate) struct ApplyTask {
    id: EffectWorkId,
    tag: EventTag,
    authorized_owner_tag: EventTag,
    subject: wire::BlockSubject,
    certificate: wire::QuorumCertificate,
    validated_receipt: ValidatedBodyReceipt,
    lifecycle_ordinal: u128,
}
impl ApplyTask {
    /// Construct an exact application task for crash-boundary unit tests.
    #[cfg(test)]
    pub(crate) fn for_test(
        id: u64,
        tag: EventTag,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Self {
        Self {
            id: EffectWorkId(id),
            tag,
            authorized_owner_tag: tag,
            subject,
            certificate,
            validated_receipt,
            lifecycle_ordinal: u128::from(id),
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
    /// Immutable actor-global lifecycle ordinal retained across asynchronous I/O.
    pub(crate) const fn lifecycle_ordinal(&self) -> u128 {
        self.lifecycle_ordinal
    }
    /// Reducer owner independently captured when the task was authorized.
    pub(crate) const fn authorized_owner_tag(&self) -> EventTag {
        self.authorized_owner_tag
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
    /// Typed Kura receipt for the exact committed block/finality pair.
    pub(crate) const fn receipt(&self) -> &KuraV2CommitReceipt {
        &self.receipt
    }
    /// Complete canonical finality artifact persisted by Kura.
    pub(crate) const fn artifact(&self) -> &wire::finality::V2FinalityArtifact {
        &self.artifact
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
    /// Exact interrupted-tip recovery stage, when startup owns that closed-ingress path.
    pub pending_tip_recovery_stage: Option<PendingKuraApplyRecoveryStage>,
    /// Number of serialized recovery scheduler attempts at this startup height.
    pub pending_tip_recovery_attempts: u64,
    /// Result of the latest serialized recovery scheduler attempt.
    pub pending_tip_recovery_last_result: Option<PendingTipRecoveryAttemptResult>,
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
    /// Signed/diagnostic outputs awaiting lifecycle-owned execution.
    pub pending_outputs: usize,
    /// Durable Apply operations waiting for an exact merge sidecar.
    pub deferred_application_merge_work: usize,
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
    /// Reducer effects retained until one bounded pending-work slot becomes
    /// available.
    ///
    /// This strict FIFO is attempted before runtime advancement and therefore
    /// never reports eligible scheduler-skip debt.
    pub effect_dispatch_queue: RuntimeQueueLaneSnapshot,
    /// Per-class serialized runtime ownership and fairness state.
    pub runtime_queues: RuntimeQueueSnapshot,
    /// View-aware no-progress threshold derived from the configured pacemaker.
    pub watchdog_threshold: Duration,
}

/// Opaque test snapshot of one recovered durable-Validate retry seal.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct RecoveredDurableValidateRetrySnapshotV1 {
    owner_identity: usize,
    causal_lifecycle_key: Hash,
    effect_tag: EventTag,
    phase: Option<wire::GlobalPhase>,
    commitment_ceiling: Option<wire::ExecutionCommitment>,
}

/// Closed observation of the raw reducer output selected by one test step.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct RuntimeStepObservationV1 {
    selected: Option<RuntimeSelectedOwnerKind>,
    effect_count: usize,
    validate_count: usize,
    non_validate_class: Option<RuntimeEffectClassV1>,
    broadcast_count: usize,
    canonical_prepare_qc_digest: Option<HashOf<wire::QuorumCertificate>>,
}

/// Closed class of the sole non-Validate sibling in one observed reducer step.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum RuntimeEffectClassV1 {
    /// Network broadcast.
    Broadcast,
    /// Certified or ordinary body fetch.
    FetchBody,
    /// Exact-body persistence.
    StoreBody,
    /// Deterministic decision application.
    Apply,
    /// Local consensus signature.
    Sign,
    /// Certified view transition.
    EnterView,
    /// Equivocation evidence.
    ReportEquivocation,
    /// Invalid certified-body evidence.
    ReportInvalidCertifiedBody,
    /// More than one non-Validate class occurred.
    Multiple,
}

#[cfg(test)]
impl RuntimeStepObservationV1 {
    /// Exact scheduler branch selected before its ownership was consumed.
    pub(in crate::sumeragi) const fn selected(&self) -> Option<RuntimeSelectedOwnerKind> {
        self.selected
    }

    /// Number of raw reducer effects before executor-side stutter handling.
    pub(in crate::sumeragi) const fn effect_count(&self) -> usize {
        self.effect_count
    }

    /// Number of raw Validate effects before executor-side stutter handling.
    pub(in crate::sumeragi) const fn validate_count(&self) -> usize {
        self.validate_count
    }

    /// Sole non-Validate class, or `Multiple` when several were emitted.
    pub(in crate::sumeragi) const fn non_validate_class(&self) -> Option<RuntimeEffectClassV1> {
        self.non_validate_class
    }

    /// Whether the sole Broadcast is the canonical envelope for this exact PrepareQC.
    pub(in crate::sumeragi) fn sole_broadcast_is_exact_prepare_qc(
        &self,
        expected: &wire::QuorumCertificate,
    ) -> bool {
        self.broadcast_count == 1 && self.canonical_prepare_qc_digest == Some(HashOf::new(expected))
    }
}

#[cfg(test)]
fn observed_non_validate_class(effects: &[AdapterEffect]) -> Option<RuntimeEffectClassV1> {
    let mut observed = None;
    for effect in effects {
        let class = match effect {
            AdapterEffect::ValidateBody { .. } => continue,
            AdapterEffect::Broadcast(_) => RuntimeEffectClassV1::Broadcast,
            AdapterEffect::FetchBody { .. } => RuntimeEffectClassV1::FetchBody,
            AdapterEffect::StoreBody { .. } => RuntimeEffectClassV1::StoreBody,
            AdapterEffect::Apply { .. } => RuntimeEffectClassV1::Apply,
            AdapterEffect::Sign { .. } => RuntimeEffectClassV1::Sign,
            AdapterEffect::EnterView { .. } => RuntimeEffectClassV1::EnterView,
            AdapterEffect::ReportEquivocation { .. } => RuntimeEffectClassV1::ReportEquivocation,
            AdapterEffect::ReportInvalidCertifiedBody { .. } => {
                RuntimeEffectClassV1::ReportInvalidCertifiedBody
            }
        };
        if observed.replace(class).is_some() {
            return Some(RuntimeEffectClassV1::Multiple);
        }
    }
    observed
}

#[cfg(test)]
fn observed_canonical_prepare_qc_digest(
    effects: &[AdapterEffect],
) -> Option<HashOf<wire::QuorumCertificate>> {
    let mut observed = None;
    for effect in effects {
        let AdapterEffect::Broadcast(message) = effect else {
            continue;
        };
        if observed.is_some() || message.protocol_version != wire::PROTOCOL_VERSION {
            return None;
        }
        let wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) = &message.payload
        else {
            return None;
        };
        if certificate.phase != wire::GlobalPhase::Prepare {
            return None;
        }
        observed = Some(HashOf::new(certificate));
    }
    observed
}

#[cfg(test)]
impl RecoveredDurableValidateRetrySnapshotV1 {
    /// Whether both snapshots retain the same exact recovered `Arc` owner.
    pub(in crate::sumeragi) fn same_owner(&self, other: &Self) -> bool {
        self.owner_identity == other.owner_identity
            && self.causal_lifecycle_key == other.causal_lifecycle_key
    }

    /// Immutable causal root recovered from the concrete registry carrier.
    pub(in crate::sumeragi) const fn causal_lifecycle_key(&self) -> Hash {
        self.causal_lifecycle_key
    }

    /// Monotonic Validate tag retained by the process-local frontier.
    pub(in crate::sumeragi) const fn effect_tag(&self) -> EventTag {
        self.effect_tag
    }

    /// Highest retained quorum phase, if any.
    pub(in crate::sumeragi) const fn phase(&self) -> Option<wire::GlobalPhase> {
        self.phase
    }

    /// First authenticated durable commitment ceiling, if any.
    pub(in crate::sumeragi) const fn commitment_ceiling(
        &self,
    ) -> Option<wire::ExecutionCommitment> {
        self.commitment_ceiling
    }
}
/// Ownership disposition returned by the exact consensus-output service.
///
/// This is deliberately distinct from `Result`: source retention is bounded
/// backpressure, not a service failure. The reducer must keep the semantic
/// source which can reproduce the control message until a later occurrence is
/// accepted by the exact-output service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConsensusBroadcastDisposition {
    /// This physical attempt was admitted by the network actor or its exact
    /// unadmitted suffix entered the bounded output corridor. Active reducer
    /// control remains eligible for a later periodic delivery attempt.
    ExactServiceAccepted,
    /// The bounded output corridor was full, so the semantic source remains
    /// responsible for a later retransmission.
    SourceRetained,
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
    /// Advance receiver-side leader-wire recovery after one runtime WAL step.
    ///
    /// `decided_subject` is `None` when the step did not install a Decision.
    /// A subject is monotone for the height.
    fn finish_runtime_step_reconciliation(
        &mut self,
        decided_subject: Option<wire::BlockSubject>,
    ) -> Result<(), Self::Error>;
    /// Retire one exact receiver-side leader-wire lifecycle after the runtime
    /// retained all of its causal successor ownership. A volatile terminal is
    /// process-local and reopens after crash; a producer terminal is backed by
    /// independently persisted continuation evidence.
    fn complete_leader_wire_runtime_terminal(
        &mut self,
        terminal: LeaderWireRuntimeTerminal,
    ) -> Result<(), Self::Error>;
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
    /// Route one canonical v2 consensus envelope through the frozen committee.
    ///
    /// Returning [`ConsensusBroadcastDisposition::ExactServiceAccepted`]
    /// completes this physical attempt. The reducer may still reproduce active
    /// control in a later periodic episode because actor admission is not a
    /// remote-delivery receipt. A [`ConsensusBroadcastDisposition::SourceRetained`]
    /// result leaves the current occurrence responsible for retry.
    fn broadcast_consensus(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<ConsensusBroadcastDisposition, Self::Error>;
    /// Sign a certified-body request with the requester's transport identity.
    fn sign_body_request(&mut self, preimage: &[u8]) -> Result<Vec<u8>, Self::Error>;
    /// Start or retransmit body reconstruction/fetch. Repeated tasks with the
    /// same work identifier are idempotent retransmission requests, not new
    /// work. Authenticated chunks are delivered separately through
    /// [`Self::accept_authenticated_chunk`].
    fn enqueue_body_fetch(&mut self, task: BodyFetchTask) -> Result<(), Self::Error>;
    /// Move the completion consumer for one unchanged protected fetch to a later lifecycle incarnation.
    ///
    /// Implementations must preserve live acquisition state and any already queued terminal
    /// completion. This is an ownership transfer, not cancellation followed by
    /// new work or a change to the proposal round.
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
    /// Hand one structurally, cryptographically, and outer-peer authenticated
    /// chunk to the bounded in-memory reconstruction adapter.
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
    /// Observe a reducer-authorized view installation and its authenticated
    /// durable-lock projection for timer/status and ingress recovery wiring.
    fn entered_view(
        &mut self,
        tag: EventTag,
        certificate: wire::TimeoutCertificate,
        protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    ) -> Result<(), Self::Error>;
    /// Validate and persist complete authenticated equivocation evidence.
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
    /// noncanonical, so that acquisition was reset for an exact retry without
    /// advancing the reducer.
    Rejected,
    /// The work identifier was already completed or belongs to an old owner.
    Stale,
}
/// Result of handing an authenticated chunk to the bounded reconstruction
/// service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthenticatedChunkDisposition {
    /// The chunk was retained or completed one canonical reconstruction.
    Accepted,
    /// The committed chunk set reconstructed invalid or noncanonical body data;
    /// the service reset that remote acquisition for retry without a local
    /// failure.
    Rejected,
}
/// Executor authority for retiring or retaining one exact leader-wire chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PayloadChunkLifecycleDisposition {
    /// Independently durable body bytes make the chunk restart-stably terminal.
    Durable(DurableBodyReceipt),
    /// Process-local body ownership or certified obsolescence makes the chunk
    /// terminal for this process; an exact retry reopens after a crash.
    Volatile,
    /// The exact bytes may still be needed by a current or protected fetch.
    Retain,
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
include!("v2_effects_recovered_fetch_and_pipeline_types.rs");
#[derive(Clone, Debug)]
struct OwnedAdapterEffect {
    effect: AdapterEffect,
    ownership: RuntimeEffectOwnership,
    /// Cleanup-only post-step high retained atomically with this EnterView.
    /// Non-EnterView effects must always carry `None`.
    highest_prepare_retention: Option<wire::QuorumCertificateRef>,
}
struct LocalProposalIntentProjection {
    command_identity: LocalProposalReadyCommandIdentity,
    effect: AdapterEffect,
    ownership: RuntimeEffectOwnership,
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
    /// Complete authenticated artifacts persisted under a canonical WSV key.
    DurableAccountabilityEvidence,
    /// Recovered view owns fresh process-local cleanup; old services no longer exist.
    RecoveredView,
    /// Non-progress diagnostic; losing it in a process crash cannot orphan work.
    DiagnosticOnly,
}
pub(crate) trait EffectRuntime {
    /// Commit one exact deferred pending-Kura marker through the serialized adapter.
    ///
    /// Synthetic runtimes cannot mint this authority and retain the default
    /// fail-closed result. Production returns one opaque predecessor-derived
    /// Apply child only after the real direct validation transition commits.
    fn commit_pending_kura_validated_apply(
        &mut self,
        marker: super::v2::DeferredPendingKuraValidatedMarkerV1,
        _predecessor: &AdapterEffect,
        _ownership: &RuntimeEffectOwnership,
    ) -> Result<
        super::v2::PendingKuraValidatedApplySuccessorV1,
        (super::v2::DeferredPendingKuraValidatedMarkerV1, String),
    > {
        Err((
            marker,
            "runtime cannot commit a deferred pending-Kura validation marker".to_owned(),
        ))
    }

    /// Return whether the exact source-only Decision WAL Apply seal remains
    /// available for a lifecycle Validate-to-Apply join. Synthetic runtimes
    /// cannot mint this affine authority and retain the closed default.
    fn has_exact_pending_live_decision_apply(
        &self,
        _tag: EventTag,
        _decision: DurableDecision,
    ) -> bool {
        false
    }

    /// Decide whether the runtime accepts one exact fair-ingress ownership carrier.
    fn can_admit_network_message_with_ingress_ownership(
        &self,
        _message: &wire::ConsensusMessageV2,
        _ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        false
    }
    /// Decide whether an exact TimeoutVote carrier may close retained restart debt.
    fn can_admit_timeout_vote_recovery_episode(
        &self,
        _message: &wire::ConsensusMessageV2,
        _ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        false
    }
    /// Return whether one pre-authentication wire has the closed shape of a
    /// pacemaker Progress root.
    ///
    /// The default covers the wire kinds which are unconditionally assigned
    /// to the protected Progress prefix. Production additionally recognizes
    /// an exact historical CommitVote for its durable lock and an exact
    /// current PrepareVote for a locally bound unchanged-lock reproposal. This
    /// is a scheduling hint; normal runtime authentication remains mandatory.
    fn wire_ingress_may_use_pacemaker_progress(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        matches!(
            payload,
            wire::ConsensusMessageV2Payload::QuorumCertificate(_)
                | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
                | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        )
    }
    /// Publish the first receiver-local physical ordinal not yet admitted.
    /// Synthetic runtimes have no outer ingress and may retain the default.
    fn set_ingress_physical_cut(&mut self, _physical_cut: u128) -> Result<(), String> {
        Ok(())
    }
    /// Freeze one already-due timeout owner for an exact fixed-cut
    /// unchanged-lock Prepare-progress scan.
    fn freeze_pre_timeout_locked_prepare_qc_cut(
        &mut self,
        _now: Instant,
    ) -> Result<Option<PreTimeoutLockedPrepareQcCutV1>, String> {
        Ok(None)
    }
    /// Deep-preview one fair-ingress payload against the frozen target.
    fn wire_previews_pre_timeout_locked_prepare_qc(
        &self,
        _cut: &PreTimeoutLockedPrepareQcCutV1,
        _payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        false
    }
    /// Dispatch at most one exact already-admitted pre-cut Prepare carrier.
    fn step_pre_timeout_locked_prepare_qc_effects(
        &mut self,
        _now: Instant,
        _cut: &PreTimeoutLockedPrepareQcCutV1,
    ) -> Result<Option<RuntimeStep<AdapterEffect>>, String> {
        Ok(None)
    }
    fn step_effects(&mut self, now: Instant) -> Result<RuntimeStep<AdapterEffect>, String>;
    /// Run at most one absolute-timeout or authenticated Progress-root turn.
    fn step_pacemaker_effects(
        &mut self,
        _now: Instant,
    ) -> Result<Option<RuntimeStep<AdapterEffect>>, String> {
        Ok(None)
    }
    fn step_recovery_effects(&mut self, now: Instant)
    -> Result<RuntimeStep<AdapterEffect>, String>;
    /// Consume the exact positional lifecycle sidecar for one returned batch.
    fn take_effect_ownership(
        &mut self,
        effects: &[AdapterEffect],
    ) -> Result<Vec<RuntimeEffectOwnership>, String>;
    /// Consume an exact live ProposalIntent WAL Sign sidecar, when the runtime
    /// produces one. Synthetic runtimes never mint production WAL authority.
    fn take_live_proposal_intent_wal_sign(
        &mut self,
        _effects: &[AdapterEffect],
    ) -> Result<Option<LiveProposalIntentWalSignHandoffV1>, String> {
        Ok(None)
    }
    /// Consume the exact receiver-side terminal sidecar emitted by the same
    /// serialized transition. A later runtime step cannot overtake it.
    fn take_leader_wire_runtime_terminals(
        &mut self,
    ) -> Result<Vec<LeaderWireRuntimeTerminal>, String>;
    /// Publish the bounded runnable owners retained outside runtime ingress
    /// before the next clock arbitration. Passive network fetches rejoin only
    /// through their exact completion owner.
    fn set_external_lifecycle_owners(
        &mut self,
        owners: Vec<RuntimeLifecycleOwner>,
    ) -> Result<(), String>;
    /// Bind the runtime's external-owner bound to this executor's configured
    /// asynchronous pending-work capacity.
    fn configure_external_lifecycle_owner_capacity(
        &mut self,
        max_pending_work: usize,
    ) -> Result<(), String>;
    /// Allocate the bounded `AssembleBody` root for a local proposal.
    fn mint_local_proposal_effect_ownership(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<LocalProposalEffectOwnership, String>;
    /// Reserve or release the scheduler-visible proposal producer for one
    /// authoritative view. Production retains it only when this node is the
    /// frozen-roster leader.
    fn reconcile_active_view_producer(&mut self, tag: EventTag, retain: bool)
    -> Result<(), String>;
    /// Retire the exact view producer only after its Proposal and chunks enter
    /// guarded remote fanout with the inherited lifecycle owner.
    #[allow(dead_code)]
    fn complete_active_view_producer_after_proposal_fanout(
        &mut self,
        proposal_round: wire::ConsensusRound,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), String>;
    /// Consume and validate the exact scheduler owner created by the preceding
    /// successful live or recovery step.
    fn take_scheduler_ownership(&mut self) -> Result<(), String>;
    /// Observe the scheduler branch before its move-only carrier is consumed.
    #[cfg(test)]
    fn last_scheduler_selection_for_test(&self) -> Option<RuntimeSelectedOwnerKind> {
        None
    }
    /// Whether any live reducer clock has been armed for this height.
    /// Synthetic runtimes model cold recovery unless they opt into live clocks.
    fn lifecycle_live_clocks_are_armed(&self) -> bool {
        false
    }
    /// Return the reducer incarnation which currently owns effects.
    fn authoritative_tag(&self) -> Option<EventTag>;
    /// Read the reducer tag, durable lock, and Decision from one serialized
    /// frontier. Synthetic runtimes which do not model locks inherit the
    /// conservative tag/Decision projection.
    fn reconciliation_frontier(&self) -> Result<RuntimeReconciliationFrontier, String> {
        Ok(RuntimeReconciliationFrontier {
            tag: self.authoritative_tag(),
            locked_body: None,
            highest_prepare: None,
            lock_is_authoritative: false,
            decision: self.decided_body()?,
        })
    }
    /// Return the exact durable Decision currently owned by the reducer.
    fn decided_body(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        String,
    >;
    /// Return the complete durable Prepare/Commit QC behind a refined body owner.
    ///
    /// Synthetic runtimes which never refine body authority may retain the
    /// default. Production returns the reducer-authenticated certificate so a
    /// lifecycle replay row never treats an opaque runtime statement as
    /// independently durable authority.
    fn durable_body_authority_certificate(
        &self,
    ) -> Result<Option<wire::QuorumCertificate>, String> {
        Ok(None)
    }
    /// Reserve an exact body completion without exposing it to the reducer.
    fn reserve_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError>;
    /// Reserve a body completion under the immutable Fetch lifecycle owner.
    fn reserve_body_available_with_owner(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        _ownership: &RuntimeEffectOwnership,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        self.reserve_body_available(tag, manifest)
    }
    /// Publish one previously reserved body completion.
    ///
    /// A mismatched token is an internal ownership violation and must fail
    /// closed instead of silently dropping the trusted completion.
    fn commit_body_available(
        &mut self,
        reservation: BodyAvailableReservation,
    ) -> Result<(), EnqueueError>;
    /// Retain one unpublished body-completion reservation for exact retry.
    /// This is not a terminal release and cannot mint a replacement owner;
    /// stale or mismatched abort tokens likewise cannot clear the exact owner.
    fn abort_body_available(&mut self, reservation: BodyAvailableReservation);
    /// Rebind one already queued exact-body completion to a later reducer incarnation.
    ///
    /// The manifest proposal round remains fixed. Returns `true` only when the
    /// runtime still owned that exact completion.
    fn rebind_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String>;
    /// Rebind the sole unpublished body completion for an exact protected
    /// pipeline, if one exists, without requiring the fetch task to carry its
    /// response-derived manifest.
    ///
    /// `false` means the fetch has not reserved a completion yet. A matching
    /// reservation must retain its physical admission and lifecycle owner.
    fn rebind_unpublished_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<bool, String>;
    /// Retire the sole unpublished body completion owned by an exact fetch
    /// pipeline, if one exists.
    ///
    /// `false` means the fetch never reached completion reservation. Any
    /// matching token must be removed with its physical capacity and restart
    /// backing before the fetch owner itself is retired.
    fn retire_unpublished_body_available(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<bool, String>;
    /// Retire a restart-dormant stage-7 fetch parent which became terminal
    /// before it could reserve a body-completion token.
    fn retire_restored_body_fetch_parent(
        &mut self,
        _effect: &AdapterEffect,
        _ownership: &RuntimeEffectOwnership,
    ) -> Result<bool, String> {
        Ok(false)
    }
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
    fn enqueue_body_stored_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
        _ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_body_stored(tag, round, subject, receipt)
    }
    fn enqueue_signature(&mut self, tag: EventTag, signature: Vec<u8>) -> Result<(), EnqueueError>;
    fn enqueue_signature_with_owner(
        &mut self,
        tag: EventTag,
        signature: Vec<u8>,
        _ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_signature(tag, signature)
    }
    fn enqueue_application_completed(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError>;
    fn enqueue_application_completed_with_owner(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
        _ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_application_completed(tag, subject)
    }
    fn verify_certificate(
        &self,
        context: &wire::HeightContext,
        certificate: &wire::QuorumCertificate,
    ) -> Result<(), String>;
    /// Authenticate one complete signed body request through this runtime's
    /// fixed certificate authority. Implementors cannot mint the opaque return
    /// type without using a verifier-backed transport entry point.
    fn authenticate_certified_body_request(
        &self,
        context: &wire::HeightContext,
        request: wire::CertifiedBodyRequest,
        authenticated_requester: &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError>;
    /// Plan an exact Store/Validate terminal retry under the runtime's
    /// immutable incumbent owner without committing authority refinement.
    fn plan_body_pipeline_candidate_terminal(
        &mut self,
        _effect: &AdapterEffect,
        _ownership: &RuntimeEffectOwnership,
    ) -> Result<Option<RuntimeEffectOwnership>, String> {
        Ok(None)
    }
    /// Commit previously planned terminal authority refinements after the
    /// executor has discharged the complete macro-step positional gate.
    fn commit_body_pipeline_candidate_terminals(
        &mut self,
        _terminals: &[(&AdapterEffect, &RuntimeEffectOwnership)],
    ) -> Result<(), String> {
        Ok(())
    }
    fn queued_commands(&self) -> usize;
    fn remaining_completion_capacity(&self) -> usize;
    fn queue_snapshot(&self, now: Instant) -> RuntimeQueueSnapshot;
    fn watchdog_threshold(&self) -> Duration;
}
impl EffectRuntime for SerializedV2Runtime {
    fn commit_pending_kura_validated_apply(
        &mut self,
        marker: super::v2::DeferredPendingKuraValidatedMarkerV1,
        predecessor: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<
        super::v2::PendingKuraValidatedApplySuccessorV1,
        (super::v2::DeferredPendingKuraValidatedMarkerV1, String),
    > {
        match self.prepare_pending_kura_validated_apply(marker, predecessor, ownership) {
            Ok(prepared) => Ok(prepared.commit()),
            Err((marker, error)) => Err((marker, error.to_string())),
        }
    }

    fn has_exact_pending_live_decision_apply(
        &self,
        tag: EventTag,
        decision: DurableDecision,
    ) -> bool {
        SerializedV2Runtime::has_exact_pending_live_decision_apply(
            self, tag, decision.0, decision.1, decision.2, decision.3,
        )
    }

    fn can_admit_network_message_with_ingress_ownership(
        &self,
        message: &wire::ConsensusMessageV2,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        SerializedV2Runtime::can_admit_network_message_with_ingress_ownership(
            self,
            message,
            ingress_ownership,
        )
    }

    fn can_admit_timeout_vote_recovery_episode(
        &self,
        message: &wire::ConsensusMessageV2,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        SerializedV2Runtime::can_admit_timeout_vote_recovery_episode(
            self,
            message,
            ingress_ownership,
        )
    }

    fn wire_ingress_may_use_pacemaker_progress(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        SerializedV2Runtime::wire_ingress_may_use_pacemaker_progress(self, payload)
    }

    fn set_ingress_physical_cut(&mut self, physical_cut: u128) -> Result<(), String> {
        SerializedV2Runtime::set_ingress_physical_cut(self, physical_cut)
    }
    fn freeze_pre_timeout_locked_prepare_qc_cut(
        &mut self,
        now: Instant,
    ) -> Result<Option<PreTimeoutLockedPrepareQcCutV1>, String> {
        SerializedV2Runtime::freeze_pre_timeout_locked_prepare_qc_cut(self, now)
    }
    fn wire_previews_pre_timeout_locked_prepare_qc(
        &self,
        cut: &PreTimeoutLockedPrepareQcCutV1,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        SerializedV2Runtime::wire_previews_pre_timeout_locked_prepare_qc(self, cut, payload)
    }
    fn step_pre_timeout_locked_prepare_qc_effects(
        &mut self,
        now: Instant,
        cut: &PreTimeoutLockedPrepareQcCutV1,
    ) -> Result<Option<RuntimeStep<AdapterEffect>>, String> {
        self.try_step_pre_timeout_locked_prepare_qc(now, cut)
            .map_err(|error| error.to_string())
    }
    fn lifecycle_live_clocks_are_armed(&self) -> bool {
        SerializedV2Runtime::lifecycle_live_clocks_are_armed(self)
    }
    fn step_effects(&mut self, now: Instant) -> Result<RuntimeStep<AdapterEffect>, String> {
        self.step(now).map_err(|error| error.to_string())
    }
    fn step_pacemaker_effects(
        &mut self,
        now: Instant,
    ) -> Result<Option<RuntimeStep<AdapterEffect>>, String> {
        self.try_step_pacemaker_escape(now)
            .map_err(|error| error.to_string())
    }
    fn step_recovery_effects(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<AdapterEffect>, String> {
        self.step_recovery(now).map_err(|error| error.to_string())
    }
    fn take_effect_ownership(
        &mut self,
        effects: &[AdapterEffect],
    ) -> Result<Vec<RuntimeEffectOwnership>, String> {
        SerializedV2Runtime::take_effect_ownership(self, effects.len())
    }
    fn take_live_proposal_intent_wal_sign(
        &mut self,
        effects: &[AdapterEffect],
    ) -> Result<Option<LiveProposalIntentWalSignHandoffV1>, String> {
        SerializedV2Runtime::take_live_proposal_intent_wal_sign(self, effects)
            .map_err(|error| error.to_string())
    }
    fn take_leader_wire_runtime_terminals(
        &mut self,
    ) -> Result<Vec<LeaderWireRuntimeTerminal>, String> {
        Ok(SerializedV2Runtime::take_leader_wire_runtime_terminals(
            self,
        ))
    }
    fn set_external_lifecycle_owners(
        &mut self,
        owners: Vec<RuntimeLifecycleOwner>,
    ) -> Result<(), String> {
        SerializedV2Runtime::set_external_lifecycle_owners(self, owners)
    }
    fn configure_external_lifecycle_owner_capacity(
        &mut self,
        max_pending_work: usize,
    ) -> Result<(), String> {
        SerializedV2Runtime::configure_external_lifecycle_owner_capacity(self, max_pending_work)
    }
    fn mint_local_proposal_effect_ownership(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<LocalProposalEffectOwnership, String> {
        SerializedV2Runtime::mint_local_proposal_effect_ownership(self, tag, manifest)
    }
    fn reconcile_active_view_producer(
        &mut self,
        tag: EventTag,
        retain: bool,
    ) -> Result<(), String> {
        SerializedV2Runtime::reconcile_active_view_producer(self, tag, retain)
    }
    fn complete_active_view_producer_after_proposal_fanout(
        &mut self,
        proposal_round: wire::ConsensusRound,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), String> {
        SerializedV2Runtime::complete_active_view_producer_after_proposal_fanout(
            self,
            proposal_round,
            ownership,
        )
    }
    fn take_scheduler_ownership(&mut self) -> Result<(), String> {
        let evidence =
            SerializedV2Runtime::take_last_scheduler_ownership(self).ok_or_else(|| {
                "successful Sumeragi v2 runtime step omitted its exact scheduler owner".to_owned()
            })?;
        evidence.validate_exact().map_err(|error| {
            format!("Sumeragi v2 runtime scheduler ownership was invalid: {error:?}")
        })
    }
    #[cfg(test)]
    fn last_scheduler_selection_for_test(&self) -> Option<RuntimeSelectedOwnerKind> {
        self.last_scheduler_ownership()
            .map(|evidence| evidence.selected)
    }
    fn authoritative_tag(&self) -> Option<EventTag> {
        Some(self.round_tag())
    }
    fn reconciliation_frontier(&self) -> Result<RuntimeReconciliationFrontier, String> {
        let directive = self
            .local_proposal_directive()
            .map_err(|error| error.to_string())?;
        let decision = self
            .replayed_decision_key()
            .map_err(|error| error.to_string())?;
        if directive.decided_subject() != decision.map(|(_, _, subject, _)| subject) {
            return Err(
                "runtime proposal directive disagreed with its durable Decision".to_owned(),
            );
        }
        Ok(RuntimeReconciliationFrontier {
            tag: Some(directive.tag()),
            locked_body: directive.locked_body(),
            highest_prepare: self
                .replayed_highest_prepare_certificate_ref()
                .map_err(|error| error.to_string())?,
            lock_is_authoritative: true,
            decision,
        })
    }
    fn decided_body(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        String,
    > {
        self.replayed_decision_key()
            .map_err(|error| error.to_string())
    }
    fn durable_body_authority_certificate(
        &self,
    ) -> Result<Option<wire::QuorumCertificate>, String> {
        self.replayed_body_authority_certificate()
            .map_err(|error| error.to_string())
    }
    fn reserve_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        SerializedV2Runtime::reserve_body_available(self, tag, manifest)
    }
    fn reserve_body_available_with_owner(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        SerializedV2Runtime::reserve_body_available_with_owner(self, tag, manifest, ownership)
    }
    fn commit_body_available(
        &mut self,
        reservation: BodyAvailableReservation,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::commit_body_available(self, reservation)
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
    fn rebind_unpublished_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<bool, String> {
        SerializedV2Runtime::rebind_unpublished_body_available(
            self, previous, rebound, round, subject,
        )
        .map_err(|error| error.to_string())
    }
    fn retire_unpublished_body_available(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<bool, String> {
        SerializedV2Runtime::retire_unpublished_body_available(self, tag, round, subject)
            .map_err(|error| error.to_string())
    }
    fn retire_restored_body_fetch_parent(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<bool, String> {
        SerializedV2Runtime::retire_restored_body_fetch_parent(self, effect, ownership)
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
    fn enqueue_body_stored_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_body_stored_with_owner(
            self, tag, round, subject, receipt, ownership,
        )
    }
    fn enqueue_signature(&mut self, tag: EventTag, signature: Vec<u8>) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_signature(self, tag, signature)
    }
    fn enqueue_signature_with_owner(
        &mut self,
        tag: EventTag,
        signature: Vec<u8>,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_signature_with_owner(self, tag, signature, ownership)
    }
    fn enqueue_application_completed(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_application_completed(self, tag, subject)
    }
    fn enqueue_application_completed_with_owner(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_application_completed_with_owner(self, tag, subject, ownership)
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
    fn authenticate_certified_body_request(
        &self,
        context: &wire::HeightContext,
        request: wire::CertifiedBodyRequest,
        authenticated_requester: &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError> {
        authenticate_certified_body_request_with_live_adapter(
            context,
            request,
            authenticated_requester,
            self.driver(),
        )
    }
    fn plan_body_pipeline_candidate_terminal(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<Option<RuntimeEffectOwnership>, String> {
        SerializedV2Runtime::plan_body_pipeline_candidate_terminal(self, effect, ownership)
    }
    fn commit_body_pipeline_candidate_terminals(
        &mut self,
        terminals: &[(&AdapterEffect, &RuntimeEffectOwnership)],
    ) -> Result<(), String> {
        SerializedV2Runtime::commit_body_pipeline_candidate_terminals(self, terminals)
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
///
/// The body-store instance marker is captured before the exact store moves to
/// the production worker. It is comparison-only and lets lifecycle services
/// prove that a body lookup is resolved by the same launched store instance,
/// rather than another open of the same path or context.
pub(crate) struct V2EffectExecutor<R = SerializedV2Runtime> {
    runtime: R,
    output_guard: Arc<ConsensusOutputGuard>,
    lifecycle_body_store_identity: Option<V2BodyStoreInstanceIdentity>,
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
    /// Move-only pre-intent authority beside exact local Store work.
    local_store_replay: BTreeMap<EffectWorkId, LocalProposalEffectOwnership>,
    /// Signed-Proposal authority advancing with the exact ordinary body stage.
    remote_proposal_replay:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), RemoteProposalReplayStageV1>,
    /// Authenticated height-one genesis authority advancing with its certified body stage.
    authenticated_genesis_replay:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), AuthenticatedGenesisReplayStageV1>,
    /// Durable local/remote Validate owners awaiting the sole lifecycle cut.
    pending_durable_validate_admissions:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), PendingDurableValidateAdmissionV1>,
    /// Inert exact owners retained after lifecycle consumes a Validate pre-admission; only proven retries can stutter.
    durable_validate_retry_seals:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableValidateRetrySealV1>,
    /// Inert exact markers for Store rows published directly by the lifecycle body pipeline.
    published_lifecycle_store_retry_markers: BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        PublishedLifecycleStoreTerminalRetrySealV1,
    >,
    /// Inert exact markers for Validate rows published directly by the lifecycle body pipeline.
    published_lifecycle_validate_retry_markers: BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        PublishedLifecycleValidateRetryMarkerV1,
    >,
    /// One current-Decision Validate retry whose cached receipt must publish
    /// a standalone lifecycle-owned Apply row.
    pending_released_lifecycle_validate_apply:
        Option<super::v2::DeferredReleasedLifecycleValidatedMarkerV1>,
    #[cfg(test)]
    last_recovered_validate_retry_trace_root: Option<Hash>,
    #[cfg(test)]
    last_recovered_validate_retry_trace_ordinal: Option<u128>,
    #[cfg(test)]
    last_runtime_step_observation: Option<RuntimeStepObservationV1>,
    /// One fsynced ProposalIntent Sign waiting for lifecycle admission.
    pending_live_wal_sign_admission: Option<PendingLiveWalSignAdmissionV1>,
    /// Signed/diagnostic outputs awaiting exact lifecycle-row execution.
    pending_lifecycle_output_admissions:
        BTreeMap<LifecycleOutputAdmissionKeyV1, PendingLifecycleOutputAdmissionV1>,
    /// Exact direct-Broadcast census which must remain pending until its
    /// globally earlier lifecycle Decision Apply terminalizes.
    lifecycle_decision_apply_successor_outputs:
        Option<AttestedLifecycleDecisionApplySuccessorOutputsV1>,
    /// Non-Clone authority beside exact cloneable `LocalProposalReady` commands.
    local_proposal_ready_replay:
        BTreeMap<LocalProposalReadyCommandIdentity, LocalProposalReadyReplayEvidenceV1>,
    /// Inseparable local body plus ProposalIntent authority retained after FIFO emission.
    local_proposal_intent_replay:
        BTreeMap<LocalProposalReadyCommandIdentity, LocalProposalIntentReplayEvidenceV1>,
    deferred_merge_work: BTreeMap<EffectWorkId, HashOf<MergeLedgerEntry>>,
    pending_applications: BTreeMap<EffectWorkId, PendingApply>,
    body_pipeline_owners: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), BodyPipelineOwner>,
    certified_work: BTreeMap<HashOf<wire::CertifiedBodyRequest>, EffectWorkId>,
    outstanding_requests: OutstandingCertifiedBodyRequests,
    /// Lifecycle-owned recovered Decision Fetch requests never enter ordinary
    /// effect work or tracker indexes.
    recovered_decision_fetches: BTreeMap<
        super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
        RecoveredDecisionFetchRequestOwnerV1,
    >,
    /// Exact signed-request reverse edge for the dedicated recovered owner.
    recovered_decision_fetch_by_request: BTreeMap<
        HashOf<wire::CertifiedBodyRequest>,
        super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
    >,
    ready_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ReadyBody>,
    /// Last reducer incarnation whose executor-side view transition completed.
    reconciled_tag: Option<EventTag>,
    protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    protected_decision: Option<DurableDecision>,
    /// Newly installed live Decision whose exact Apply suffix cannot enter the
    /// worker until the runner retires process-local proposal and lane owners.
    pending_runner_decision_cleanup: Option<PendingRunnerDecisionCleanup>,
    live_lifecycle_decision_apply: Option<LiveLifecycleDecisionApplyOwnerV1>,
    live_lifecycle_validate_successor: Option<LiveLifecycleValidateSuccessorOwnerV1>,
    pending_tip_recovery: Option<PendingKuraApplyRecoveryEvidence>,
    pending_tip_recovery_attempts: u64,
    pending_tip_recovery_last_result: Option<PendingTipRecoveryAttemptResult>,
    decision_body_drained: bool,
    authenticated_genesis_body: Option<InstalledAuthenticatedGenesisReplayAuthorityV1>,
    retained_locked_body: Option<(wire::BlockSubject, Arc<[u8]>)>,
    ready_body_bytes: u64,
    pending_store_bytes: u64,
    durable_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    validated_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    rejected_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    /// Structurally authenticated but not semantically replayed rejection
    /// markers. They deny only exact cold local-proposal adoption and never
    /// authorize a reducer validation failure.
    retired_rejected_bodies:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    finality_completion: Option<FinalityCompletion>,
    retained_effect_batch: Option<RetainedEffectBatch>,
    /// Ordinary dispatch debt parked behind one bounded typed control turn.
    parked_effect_batch: Option<RetainedEffectBatch>,
    fatal_reason: Option<String>,
}
/// Executor-private one-shot permit for minting an exact next-Vote body owner.
///
/// The opaque adapter authority accepts this permit only after the executor
/// has rejoined its exact launched body-store marker and all three retained
/// body catalogs. Sibling modules can name but cannot construct the permit.
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyAuthorityMintPermitV1 {
    _linearity: RecoveredLifecycleNextVoteBodyAuthorityMintPermitLinearityV1,
}
struct RecoveredLifecycleNextVoteBodyAuthorityMintPermitLinearityV1;
impl Drop for RecoveredLifecycleNextVoteBodyAuthorityMintPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextVoteBodyAuthorityMintPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleNextVoteBodyAuthorityMintPermitLinearityV1,
        }
    }
}
/// Executor-private one-shot permit for binding one preview to its store owner.
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyPreviewBindPermitV1 {
    _linearity: RecoveredLifecycleNextVoteBodyPreviewBindPermitLinearityV1,
}
struct RecoveredLifecycleNextVoteBodyPreviewBindPermitLinearityV1;
impl Drop for RecoveredLifecycleNextVoteBodyPreviewBindPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextVoteBodyPreviewBindPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleNextVoteBodyPreviewBindPermitLinearityV1,
        }
    }
}
fn authenticate_recovered_lifecycle_next_vote_body_catalogs(
    lookup: RecoveredLifecycleNextVoteBodyLookupV1,
    body_store_identity: V2BodyStoreInstanceIdentity,
    recovered_bodies: &BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    durable_bodies: &BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    validated_bodies: &BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
) -> Result<RecoveredLifecycleNextVoteBodyAuthorityV1, EffectExecutorError> {
    let mut exact = validated_bodies
        .values()
        .filter(|validated| lookup.matches_validated_body(validated));
    let validated = exact.next().cloned().ok_or_else(|| {
        EffectExecutorError::BodyStore(
            "recovered next-Vote body lookup has no exact validated receipt".to_owned(),
        )
    })?;
    if exact.next().is_some() {
        return Err(EffectExecutorError::BodyStore(
            "recovered next-Vote body lookup matched multiple validated receipts".to_owned(),
        ));
    }
    let durable = validated.durable();
    let key = (durable.round(), durable.subject());
    let recovered = recovered_bodies.get(&key);
    if validated_bodies.get(&key) != Some(&validated)
        || durable_bodies.get(&key) != Some(durable)
        || recovered.is_none_or(|(manifest, recovered_durable)| {
            recovered_durable != durable
                || HashOf::new(manifest) != durable.manifest_hash()
                || !lookup.matches_recovered_body(manifest, recovered_durable)
        })
    {
        return Err(EffectExecutorError::BodyStore(
            "recovered next-Vote body lookup changed its exact body catalogs".to_owned(),
        ));
    }
    RecoveredLifecycleNextVoteBodyAuthorityV1::from_exact_executor(
        RecoveredLifecycleNextVoteBodyAuthorityMintPermitV1::new(),
        lookup,
        validated,
        body_store_identity,
    )
    .ok_or_else(|| {
        EffectExecutorError::Contract(
            "recovered next-Vote body authority failed its final exact join".to_owned(),
        )
    })
}

impl<R: EffectRuntime> V2EffectExecutor<R> {
    /// Borrow the sole response-free recovered Decision Fetch owner for one
    /// periodic retransmission.
    ///
    /// Network-actor admission is not a remote-delivery receipt.  The executor
    /// therefore remains the semantic source of the exact signed request until
    /// an authenticated response claims it.  This projection neither changes
    /// the external lifecycle wait nor creates a second request identity.
    pub(in crate::sumeragi) fn recovered_decision_fetch_retransmission_owner(
        &self,
    ) -> Result<Option<&RecoveredDecisionFetchRequestOwnerV1>, EffectExecutorError> {
        self.ensure_open()?;
        if self.recovered_decision_fetches.len() > 1
            || !self.recovered_decision_fetch_request_index_is_exact()
        {
            return Err(EffectExecutorError::Contract(
                "recovered Decision Fetch retransmission indexes are not exact".to_owned(),
            ));
        }
        let Some(owner) = self.recovered_decision_fetches.values().next() else {
            return Ok(None);
        };
        if !owner.validates_exact_executor_context(&self.context, &self.requester) {
            return Err(EffectExecutorError::Contract(
                "recovered Decision Fetch retransmission owner changed context".to_owned(),
            ));
        }
        Ok(owner
            .candidate_projection()
            .response_claim
            .is_none()
            .then_some(owner))
    }

    /// Project the sole exact recovered request indexes for lifecycle tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn recovered_decision_fetch_owner_for_test(
        &self,
    ) -> Option<(
        super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
        HashOf<wire::CertifiedBodyRequest>,
    )> {
        let (&key, owner) = self.recovered_decision_fetches.first_key_value()?;
        let request_hash = owner.request_hash();
        (self.recovered_decision_fetches.len() == 1
            && self.recovered_decision_fetch_by_request.len() == 1
            && self.recovered_decision_fetch_by_request.get(&request_hash) == Some(&key))
        .then_some((key, request_hash))
    }

    /// Preflight one dedicated recovered request without retaining an executor borrow.
    ///
    /// `Ok(false)` is reserved for the configured request-capacity bound. Every
    /// identity, index, coordinate, or existing dedicated-owner conflict stays
    /// a typed error so the scheduler cannot hide corruption as backpressure.
    pub(in crate::sumeragi) fn recovered_decision_fetch_registration_available(
        &self,
        owner: &RecoveredDecisionFetchRequestOwnerV1,
    ) -> Result<bool, RecoveredDecisionFetchRequestRegistrationErrorV1> {
        if self.output_guard.restart_required()
            || self.fatal_reason.is_some()
            || !owner.validates_exact_executor_context(&self.context, &self.requester)
        {
            return Err(RecoveredDecisionFetchRequestRegistrationErrorV1::ForeignExecutor);
        }
        if self.validated_certified_request_presence().is_err() {
            return Err(RecoveredDecisionFetchRequestRegistrationErrorV1::InvalidExistingCensus);
        }
        let key = owner.dispatch_key();
        let request_hash = owner.request_hash();
        if !self.recovered_decision_fetches.is_empty()
            || self
                .recovered_decision_fetch_by_request
                .contains_key(&request_hash)
        {
            return Err(RecoveredDecisionFetchRequestRegistrationErrorV1::Occupied);
        }
        if self.certified_work.contains_key(&request_hash)
            || self.outstanding_requests.contains(request_hash)
            || owner.conflicts_with_ordinary_tracker(&self.outstanding_requests)
            || self.pending_fetches.values().any(|pending| {
                owner.matches_body_coordinates(pending.task.round, pending.task.subject)
            })
            || self.recovered_decision_fetches.values().any(|existing| {
                let projection = owner.candidate_projection();
                existing.matches_body_coordinates(projection.round, projection.subject)
            })
            || self
                .recovered_decision_fetch_by_request
                .values()
                .any(|existing| *existing == key)
        {
            return Err(RecoveredDecisionFetchRequestRegistrationErrorV1::ConflictingOwner);
        }
        if self
            .outstanding_requests
            .len()
            .checked_add(self.recovered_decision_fetches.len())
            .is_none_or(|owned| owned >= self.config.max_certified_requests)
        {
            return Ok(false);
        }
        Ok(true)
    }

    /// Reserve the sole dedicated recovered Decision Fetch owner position.
    /// Exact hash, logical request identity, body coordinates, and both ordinary
    /// and recovered reverse indexes are checked while the executor is
    /// exclusively borrowed. No map changes until the returned reservation
    /// consumes the claimed registry carrier.
    pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_request_registration(
        &mut self,
        owner: RecoveredDecisionFetchRequestOwnerV1,
    ) -> Result<
        PreparedRecoveredDecisionFetchRequestRegistrationV1<'_, R>,
        RecoveredDecisionFetchRequestRegistrationErrorV1,
    > {
        if !self.recovered_decision_fetch_registration_available(&owner)? {
            return Err(RecoveredDecisionFetchRequestRegistrationErrorV1::Occupied);
        }
        Ok(PreparedRecoveredDecisionFetchRequestRegistrationV1 {
            executor: self,
            owner: Some(owner),
        })
    }
}

include!("v2_effects_recovered_lifecycle_output_service.rs");
include!("v2_effects_lifecycle_admission_settlement.rs");

impl V2EffectExecutor<SerializedV2Runtime> {
    /// Take ownership of an exact-body store opened during sealed preflight.
    ///
    /// Production uses this entry point after independently inspecting the
    /// store's recovery catalog to validate durable leader-wire terminals and
    /// before allowing the runtime to mint any later scheduler ordinal.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn open_with_body_store(
        mut runtime: SerializedV2Runtime,
        body_store: V2BodyStore,
        mut recovered_validate_retry_census: RecoveredDurableValidateRetryCensusV1,
        mut pending_kura_apply_replay: Option<
            &mut super::v2::PreparedRecoveredPendingKuraApplyReplayV1,
        >,
        context: wire::HeightContext,
        requester: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        output_guard: Arc<ConsensusOutputGuard>,
        config: EffectQueueConfig,
    ) -> Result<(Self, V2BodyStore), EffectExecutorError> {
        let executor_output_guard = Arc::clone(&output_guard);
        let lifecycle_body_store_identity = body_store.instance_identity();
        let construction = output_guard.begin_fail_stop_operation().ok_or_else(|| {
            EffectExecutorError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            )
        })?;
        if !body_store.matches_context(&context) {
            return Err(EffectExecutorError::BodyStore(
                "pre-opened Sumeragi v2 body store changed its height context".to_owned(),
            ));
        }
        body_store
            .ensure_recovered_markers_revalidated()
            .map_err(|error| EffectExecutorError::BodyStore(error.to_string()))?;
        let recovered_bodies = body_store
            .recovery_catalog()
            .map_err(|error| EffectExecutorError::BodyStore(error.to_string()))?;
        let recovered_validations = body_store.validated_recovery_catalog();
        let recovered_rejections = body_store.rejected_recovery_catalog();
        let retired_recovered_rejections = body_store.retired_rejected_recovery_catalog();
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
            let ready_validate_deferred = recovered_validate_retry_census
                .classify_and_bind_validated_marker(*key, validated_receipt)
                .map_err(|error| EffectExecutorError::BodyStore(error.to_owned()))?;
            let pending_kura_deferred = pending_kura_apply_replay
                .as_deref_mut()
                .map(|replay| {
                    replay.classify_and_defer_validated_marker(
                        *key,
                        manifest,
                        durable_receipt,
                        validated_receipt,
                    )
                })
                .transpose()
                .map_err(|error| EffectExecutorError::BodyStore(error.to_owned()))?
                .unwrap_or(false);
            match (ready_validate_deferred, pending_kura_deferred) {
                (false, false) => runtime
                    .recover_validated_body(manifest, validated_receipt)
                    .map_err(|error| EffectExecutorError::Runtime(error.to_string()))?,
                (true, true) => {
                    return Err(EffectExecutorError::BodyStore(
                        "validated marker retained two cold recovery owners".to_owned(),
                    ));
                }
                (true, false) | (false, true) => {}
            }
        }
        if pending_kura_apply_replay
            .as_deref()
            .is_some_and(|replay| !replay.validated_marker_was_deferred())
        {
            return Err(EffectExecutorError::BodyStore(
                "pending Kura replay has no exact validated marker to defer".to_owned(),
            ));
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
        executor.lifecycle_body_store_identity = Some(lifecycle_body_store_identity);
        executor.install_recovered_validation_catalog(
            recovered_validations,
            recovered_rejections,
            retired_recovered_rejections,
        )?;
        recovered_validate_retry_census.install_into_executor(&mut executor)?;
        construction.complete();
        Ok((executor, body_store))
    }
    /// Preview one recovered signature and authenticate its next body in one borrow.
    ///
    /// The runtime borrow retained by the adapter preview is disjoint from the
    /// body catalogs below. This single executor method therefore avoids a
    /// second preview and never requires a caller to reborrow the whole
    /// executor while the first preview is live.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body(
        &mut self,
        service: super::v2_worker::RecoveredLifecycleNextVoteBodyExecutorPermitV1,
        completion: super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1,
    ) -> Result<
        (
            super::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'_>,
            RecoveredLifecycleNextVoteBodyAuthorityV1,
        ),
        EffectExecutorError,
    > {
        let Self {
            runtime,
            output_guard,
            lifecycle_body_store_identity,
            recovered_bodies,
            context,
            requester,
            durable_bodies,
            validated_bodies,
            fatal_reason,
            ..
        } = self;
        if output_guard.restart_required() || fatal_reason.is_some() {
            return Err(EffectExecutorError::Contract(
                "recovered next-Vote body lookup belongs to a closed executor".to_owned(),
            ));
        }
        let executor_body_store_identity =
            lifecycle_body_store_identity.as_ref().ok_or_else(|| {
                EffectExecutorError::Contract(
                    "recovered next-Vote body lookup has no launched body-store owner".to_owned(),
                )
            })?;
        let body_store_identity = service
            .consume_for_executor(
                context,
                requester,
                output_guard,
                executor_body_store_identity,
            )
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "recovered next-Vote body service permit changed its executor owner".to_owned(),
                )
            })?;
        let mut preview = runtime
            .prepare_recovered_lifecycle_sign_completion(completion)
            .map_err(|error| EffectExecutorError::Runtime(error.to_string()))?;
        let lookup = match preview.shape() {
            super::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign => preview
                .project_broadcast_and_sign_body_lookup(
                    RecoveredLifecycleNextVoteBodyPreviewBindPermitV1::new(),
                    body_store_identity.clone(),
                    Arc::clone(output_guard),
                ),
            super::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal => preview
                .prepare_proposal_prepare_wal_body_lookup(
                    RecoveredLifecycleNextVoteBodyPreviewBindPermitV1::new(),
                    body_store_identity.clone(),
                    Arc::clone(output_guard),
                ),
            super::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast => {
                Err(super::v2::AdapterError::RecoveredLifecycleSignCompletionMismatch)
            }
        }
        .map_err(|error| EffectExecutorError::Runtime(error.to_string()))?;
        if !lookup.matches_height_context(context) {
            return Err(EffectExecutorError::Contract(
                "recovered next-Vote body lookup changed its height context".to_owned(),
            ));
        }
        let body = authenticate_recovered_lifecycle_next_vote_body_catalogs(
            lookup,
            body_store_identity,
            recovered_bodies,
            durable_bodies,
            validated_bodies,
        )?;
        Ok((preview, body))
    }
    /// Reserve exclusive mutation of the exact recovered response-family claim.
    pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_response_claim(
        &mut self,
        task: &super::v2_lifecycle_coordinator::RecoveredDecisionFetchBodyPersistenceTaskV1,
    ) -> Result<
        PreparedRecoveredDecisionFetchResponseClaimV1<'_>,
        RecoveredDecisionFetchResponseClaimErrorV1,
    > {
        let key = task.dispatch_key();
        let response_hash = task.response_hash();
        let preflight = task.claim_preflight();
        if !self.recovered_decision_fetch_request_index_is_exact() {
            return Err(RecoveredDecisionFetchResponseClaimErrorV1::InvalidOwnerIndex);
        }
        let Some(owner) = self.recovered_decision_fetches.get(&key) else {
            return Err(RecoveredDecisionFetchResponseClaimErrorV1::ForeignOwner);
        };
        if !key.matches_height_context(&self.context)
            || !owner.validates_exact_executor_context(&self.context, &self.requester)
        {
            return Err(RecoveredDecisionFetchResponseClaimErrorV1::ForeignOwner);
        }
        if !owner.matches_response_claim_preflight(response_hash, preflight) {
            return Err(RecoveredDecisionFetchResponseClaimErrorV1::ConflictingClaim);
        }
        Ok(PreparedRecoveredDecisionFetchResponseClaimV1 {
            executor: self,
            key,
            response_hash,
            preflight,
        })
    }
    /// Preview the recovered Store reducer transition through the dedicated runtime seam.
    pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_store_adapter(
        &mut self,
        authority: super::v2_lifecycle_coordinator::RecoveredDecisionFetchStoreAdapterAuthorityV1,
    ) -> Result<super::v2::PreparedRecoveredDecisionFetchStoreAdapterV1<'_>, super::v2::AdapterError>
    {
        self.runtime
            .prepare_recovered_decision_fetch_store(authority)
    }
    /// Seal the serialized adapter's current reducer-fence generation.
    pub(in crate::sumeragi) fn lifecycle_reducer_fence_observation(
        &self,
    ) -> super::v2::LifecycleReducerFenceObservationV1 {
        self.runtime.lifecycle_reducer_fence_observation()
    }
    /// Preview one registry-owned Ready Validate outcome through the serialized adapter.
    pub(in crate::sumeragi) fn prepare_ready_durable_validate_adapter_preview<'registry>(
        &mut self,
        execution: super::v2_lifecycle_coordinator::PreparedReadyDurableValidateExecution<
            'registry,
        >,
    ) -> Result<
        super::v2_lifecycle_coordinator::PreparedReadyDurableValidateAdapterPreview<'registry, '_>,
        super::v2_lifecycle_coordinator::ReadyDurableValidateAdapterPreviewError<'registry>,
    > {
        let mut execution = execution;
        let validated_catalog_authority = execution.take_validated_catalog_authority();
        if matches!(
            execution.outcome_kind(),
            super::v2_lifecycle_coordinator::ReadyDurableValidateOutcomeKind::Validated
        ) != validated_catalog_authority.is_some()
        {
            return Err(super::v2_lifecycle_coordinator::ReadyDurableValidateAdapterPreviewError::runtime_gate(
                execution,
                AdapterError::ReadyDurableValidatePublicationContractViolation,
            ));
        }
        if let Some(authority) = validated_catalog_authority
            && let Err(error) = self.record_lifecycle_validated_body(authority)
        {
            self.fatal_reason = Some(error.to_string());
            return Err(super::v2_lifecycle_coordinator::ReadyDurableValidateAdapterPreviewError::runtime_gate(
                execution,
                AdapterError::ReadyDurableValidatePublicationContractViolation,
            ));
        }
        let local_handoff = execution.project_local_proposal_ready();
        let local_publication = match local_handoff.as_ref() {
            Some(handoff) => {
                let Some(identity) = handoff.command_identity() else {
                    iroha_logger::error!(
                        "local Ready Validate handoff could not derive its exact runtime identity"
                    );
                    return Err(super::v2_lifecycle_coordinator::ReadyDurableValidateAdapterPreviewError::runtime_gate(
                        execution,
                        AdapterError::ReadyDurableValidatePublicationContractViolation,
                    ));
                };
                Some((identity, handoff.lifecycle_ordinal()))
            }
            None => None,
        };
        let preflight_kind = match self
            .runtime
            .preflight_ready_durable_validate_adapter_publication(&execution, local_publication)
        {
            Ok(kind) => kind,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    "Ready Validate adapter publication preflight failed"
                );
                return Err(super::v2_lifecycle_coordinator::ReadyDurableValidateAdapterPreviewError::runtime_gate(
                    execution,
                    error,
                ));
            }
        };
        use crate::sumeragi::v2::ReadyDurableValidateAdapterPublicationKind as Kind;
        if local_handoff.is_some()
            && !matches!(
                preflight_kind,
                Kind::ValidatedBusy | Kind::ValidatedInactive | Kind::ValidatedNoEffect
            )
        {
            self.fatal_reason = Some(
                "local lifecycle Validate produced a nonterminal reducer publication".to_owned(),
            );
            iroha_logger::error!(
                ?preflight_kind,
                "local Ready Validate preflight produced a nonterminal reducer publication"
            );
            return Err(super::v2_lifecycle_coordinator::ReadyDurableValidateAdapterPreviewError::runtime_gate(
                execution,
                AdapterError::ReadyDurableValidatePublicationContractViolation,
            ));
        }
        let local_published = match local_handoff {
            Some(handoff) => match handoff.publish_into_runtime(&mut self.runtime) {
                Ok(published) => Some(published),
                Err(_) => {
                    iroha_logger::error!(
                        "local Ready Validate handoff failed exact runtime publication"
                    );
                    return Err(super::v2_lifecycle_coordinator::ReadyDurableValidateAdapterPreviewError::runtime_gate(
                        execution,
                        AdapterError::ReadyDurableValidatePublicationContractViolation,
                    ));
                }
            },
            None => None,
        };
        let has_local_publication = local_published.is_some();
        if let Some(published) = local_published {
            let identity = published.command_identity();
            let ready_incumbent = self.local_proposal_ready_replay.get(&identity);
            let intent_incumbent = self.local_proposal_intent_replay.get(&identity);
            let command_was_coalesced = published.command_was_coalesced();
            if command_was_coalesced {
                // Runtime installed no FIFO owner. Drop this new linear replay
                // value and leave any older replay incumbent untouched;
                // semantic coalescence is terminal cancellation, not an owner
                // substitution or a source of fresh replay authority.
            } else if ready_incumbent.is_none() && intent_incumbent.is_none() {
                let (identity, replay) = published.into_entry();
                let previous = self.local_proposal_ready_replay.insert(identity, replay);
                debug_assert!(previous.is_none());
            } else {
                self.fatal_reason = Some(
                    "lifecycle local-proposal publication conflicted with retained replay authority"
                        .to_owned(),
                );
                iroha_logger::error!(
                    "local Ready Validate publication conflicted with retained replay authority"
                );
                return Err(super::v2_lifecycle_coordinator::ReadyDurableValidateAdapterPreviewError::runtime_gate(
                    execution,
                    AdapterError::ReadyDurableValidatePublicationContractViolation,
                ));
            }
        }
        let preview = match self
            .runtime
            .prepare_ready_durable_validate_adapter_preview(execution, local_publication)
        {
            Ok(preview) => preview,
            Err(error) => {
                iroha_logger::error!(
                    "Ready Validate adapter preview failed after local publication"
                );
                return Err(error);
            }
        };
        if preview.publication_kind() != preflight_kind
            || (has_local_publication
                && !matches!(
                    preview.publication_kind(),
                    Kind::ValidatedBusy | Kind::ValidatedInactive | Kind::ValidatedNoEffect
                ))
        {
            let published_kind = preview.publication_kind();
            let error = preview.into_runtime_gate_error(
                AdapterError::ReadyDurableValidatePublicationContractViolation,
            );
            self.fatal_reason = Some(
                "local lifecycle Validate produced a nonterminal reducer publication".to_owned(),
            );
            iroha_logger::error!(
                ?preflight_kind,
                ?published_kind,
                "Ready Validate adapter publication changed after preflight"
            );
            return Err(error);
        }
        Ok(preview)
    }

    /// Preview one exact lifecycle-owned signature on the serialized adapter.
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(
        &mut self,
        authority: super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1,
    ) -> Result<
        super::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'_>,
        super::v2::AdapterError,
    > {
        self.runtime
            .prepare_recovered_lifecycle_sign_completion(authority)
    }
    /// Arm the runtime pacemaker after all height startup work has completed.
    pub(in crate::sumeragi) fn arm_live_clocks(
        &mut self,
        _permit: ProductionLifecycleLiveClockActivationPermitV1,
        now: Instant,
    ) -> Result<(), RuntimeClockError> {
        if self.pending_tip_recovery.is_some() {
            return Err(RuntimeClockError::PendingKuraRecovery);
        }
        let tag = self.current_tag();
        let retain = self.local_validator == Some(self.context.leader(tag.view()));
        self.runtime
            .reconcile_active_view_producer(tag, retain)
            .map_err(|_| RuntimeClockError::ProducerReservation)?;
        self.runtime.arm_live_clocks(now)
    }
    /// Prove runner setup has not crossed the one-shot live-clock boundary.
    pub(in crate::sumeragi) fn lifecycle_live_clocks_are_unarmed(&self) -> bool {
        !self.runtime.lifecycle_live_clocks_are_armed()
    }

    /// Borrow the complete pending-output census only for lifecycle registry
    /// attestation. No output service or ownership transfer is exposed.
    pub(in crate::sumeragi) fn pending_lifecycle_output_admission_census(
        &self,
    ) -> impl ExactSizeIterator<Item = &PendingLifecycleOutputAdmissionV1> {
        self.pending_lifecycle_output_admissions.values()
    }

    /// Return whether one typed Decision Apply can enter its terminal worker barrier.
    ///
    /// Every admitted runtime command must drain before the worker can make
    /// Kura durable. This preserves ordinary serialized settlement for the
    /// finite pre-Apply FIFO while the Ready Apply retains its exact owner.
    /// The only retained output debt is a registry-attested direct successor;
    /// the runner Decision-cleanup handoff and all other mutation owners must
    /// already be empty.
    pub(in crate::sumeragi) fn lifecycle_decision_apply_dispatch_available(
        &self,
        successor_outputs: Option<&AttestedLifecycleDecisionApplySuccessorOutputsV1>,
    ) -> Result<bool, EffectExecutorError> {
        self.ensure_open()?;
        let successor_debt_is_exact = match successor_outputs {
            None => {
                self.pending_lifecycle_output_admissions.is_empty()
                    && self.retained_effect_batch.is_none()
            }
            Some(attestation) => {
                self.lifecycle_decision_apply_successor_outputs.is_none()
                    && attestation.pending_count() == self.pending_lifecycle_output_admissions.len()
                    && attestation.exactly_matches_pending_keys(
                        self.pending_lifecycle_output_admissions.keys(),
                    )
                    && self
                        .pending_lifecycle_output_admissions
                        .values()
                        .next()
                        .is_some_and(|pending_output| {
                            self.retained_effect_batch.as_ref().is_some_and(|batch| {
                                batch.effects.len() == 1
                                    && batch.effects.front().is_some_and(|owned| {
                                        attestation.exactly_matches_retransmit_apply(&owned.effect)
                                            && pending_output
                                                .exactly_precedes_periodic_retransmit_apply(
                                                    &owned.effect,
                                                    &owned.ownership,
                                                )
                                    })
                            })
                        })
            }
        };
        Ok(
            self.pending_work() == self.pending_lifecycle_output_admissions.len()
                && successor_debt_is_exact
                && self.pending_runner_decision_cleanup.is_none()
                && self.recovered_decision_fetch_request_index_is_exact_and_empty()
                && self.parked_effect_batch.is_none()
                && self.finality_completion.is_none()
                && self.runtime.queued_commands() == 0
                && self.runtime.lifecycle_decision_apply_dispatch_available(),
        )
    }

    /// Bind lifecycle Decision Apply queue publication to pending-Kura stage ownership.
    ///
    /// Ordinary live/recovered dispatch receives an inert token. When startup
    /// owns interrupted-tip evidence, the exact registry task remains borrowed
    /// until its worker reservation publishes the command; only then may the
    /// recovery stage become `ApplicationDispatched`.
    pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_executor_dispatch<'executor>(
        &'executor mut self,
        prepared: &PreparedLifecycleDecisionApplyDispatchV1<'_>,
        successor_outputs: Option<AttestedLifecycleDecisionApplySuccessorOutputsV1>,
    ) -> Result<PreparedLifecycleDecisionApplyExecutorDispatchV1<'executor>, EffectExecutorError>
    {
        self.ensure_open()?;
        if successor_outputs
            .as_ref()
            .is_some_and(|attestation| attestation.dispatch_key() != prepared.dispatch_key())
            || !self.lifecycle_decision_apply_dispatch_available(successor_outputs.as_ref())?
        {
            return Err(EffectExecutorError::Contract(
                "lifecycle Apply dispatch overtook retained executor work".to_owned(),
            ));
        }
        let Some(evidence) = self.pending_tip_recovery.as_ref() else {
            let successor_outputs = successor_outputs.map(|attestation| {
                PendingLifecycleDecisionApplySuccessorOutputsTransitionV1 {
                    installed: &mut self.lifecycle_decision_apply_successor_outputs,
                    retained_effect_batch: &mut self.retained_effect_batch,
                    attestation,
                }
            });
            return Ok(PreparedLifecycleDecisionApplyExecutorDispatchV1 {
                pending: None,
                successor_outputs,
            });
        };
        if successor_outputs.is_some() {
            return Err(EffectExecutorError::Contract(
                "recovered lifecycle Apply borrowed a live post-Apply output census".to_owned(),
            ));
        }
        let exact = evidence.stage() == PendingKuraApplyRecoveryStage::Apply
            && evidence.is_exact(&self.context)
            && evidence.replay_tag() == self.current_tag()
            && prepared.exactly_matches_pending_kura_recovery(
                &self.context,
                evidence.replay_tag(),
                evidence.commit_subject(),
                evidence.commit_qc(),
                evidence.validated_receipt(),
            );
        if !exact {
            return Err(EffectExecutorError::Contract(
                "pending-Kura lifecycle Apply dispatch changed its exact recovery owner".to_owned(),
            ));
        }
        let pending_tip_recovery = &mut self.pending_tip_recovery;
        let pending_tip_recovery_last_result = &mut self.pending_tip_recovery_last_result;
        let evidence = pending_tip_recovery
            .as_mut()
            .expect("pending-Kura dispatch preflight retained exact evidence");
        Ok(PreparedLifecycleDecisionApplyExecutorDispatchV1 {
            pending: Some(PendingKuraApplyDispatchTransitionV1 {
                evidence,
                last_result: pending_tip_recovery_last_result,
            }),
            successor_outputs: None,
        })
    }

    /// Install or physically refine the preliminary retransmit exclusion
    /// retained by one exact published or sidecar-woken Validate successor.
    ///
    /// A sidecar wake first owns the pre-execution carrier. Its authenticated
    /// worker completion then replaces only that same row's digest and may
    /// downgrade Apply authorization after deterministic rejection. No other
    /// coordinate or false-to-true authorization substitution is accepted.
    /// This owner carries no Apply receipt, child address, or worker identity
    /// and therefore cannot stand in for the later full live Apply owner.
    pub(in crate::sumeragi) fn arm_live_lifecycle_validate_successor(
        &mut self,
        dispatch_key: LifecycleValidateDispatchKeyV1,
        incumbent_dispatch_key: Option<LifecycleValidateDispatchKeyV1>,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        apply_is_authorized: bool,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        let candidate = LiveLifecycleValidateSuccessorOwnerV1 {
            dispatch_key,
            round,
            subject,
            apply_is_authorized,
        };
        if !dispatch_key.matches_height_context(&self.context)
            || round.context_id != self.context.id()
            || round.height != self.context.height
            || !self.exactly_owns_validate_retry_lifecycle_ordinal(
                (round, subject),
                dispatch_key.lifecycle_ordinal(),
            )
            || self.live_lifecycle_decision_apply.is_some()
        {
            return Err(EffectExecutorError::Contract(
                "Validate successor belongs to another executor cut".to_owned(),
            ));
        }
        match self.live_lifecycle_validate_successor.as_ref() {
            Some(existing)
                if existing.dispatch_key == candidate.dispatch_key
                    && existing.round == candidate.round
                    && existing.subject == candidate.subject
                    && existing.apply_is_authorized == candidate.apply_is_authorized =>
            {
                Ok(())
            }
            Some(existing)
                if incumbent_dispatch_key == Some(existing.dispatch_key)
                    && existing.can_refine_to(&candidate) =>
            {
                // Durable completion may refine only the explicitly attested
                // incumbent carrier at this physical Validate address.
                self.live_lifecycle_validate_successor = Some(candidate);
                Ok(())
            }
            Some(_) => Err(EffectExecutorError::Contract(
                "a second Validate successor changed the preliminary retransmit exclusion"
                    .to_owned(),
            )),
            None => {
                self.live_lifecycle_validate_successor = Some(candidate);
                Ok(())
            }
        }
    }

    /// Release the preliminary successor owner after the exact Validate row
    /// durably resolves without a live Apply child.
    pub(in crate::sumeragi) fn release_live_lifecycle_validate_successor(
        &mut self,
        ordinal: u128,
        resolution: LifecycleValidateRetryResolutionV1,
    ) -> Result<(), EffectExecutorError> {
        let Some(owner) = self.live_lifecycle_validate_successor.take() else {
            return Err(EffectExecutorError::Contract(
                "resolved Validate omitted its preliminary retransmit owner".to_owned(),
            ));
        };
        if owner.dispatch_key.lifecycle_ordinal() != ordinal {
            self.live_lifecycle_validate_successor = Some(owner);
            return Err(EffectExecutorError::Contract(
                "resolved Validate changed its preliminary retransmit owner".to_owned(),
            ));
        }
        if self.live_lifecycle_decision_apply.is_some() {
            self.live_lifecycle_validate_successor = Some(owner);
            return Err(EffectExecutorError::Contract(
                "non-Apply Validate successor found a full live Apply owner".to_owned(),
            ));
        }
        let key = (owner.round, owner.subject);
        match self.resolve_validate_retry_lifecycle_ordinal(key, ordinal, resolution) {
            Ok(true) => {}
            Ok(false) => {
                self.live_lifecycle_validate_successor = Some(owner);
                return Err(EffectExecutorError::Contract(
                    "resolved Validate lost its exact retry authority".to_owned(),
                ));
            }
            Err(error) => {
                self.live_lifecycle_validate_successor = Some(owner);
                return Err(error);
            }
        }
        Ok(())
    }

    /// Reconcile all competing executor work before a Ready live Apply can
    /// reserve worker capacity or claim its lifecycle lease.
    ///
    /// The registry-minted authority carries no queue identity. Failure closes
    /// the executor and output guard while the logical Ready row remains
    /// untouched; success is idempotent when capacity is unavailable later.
    pub(in crate::sumeragi) fn reconcile_live_lifecycle_decision_apply<S: V2EffectServices>(
        &mut self,
        authority: LiveLifecycleDecisionApplyReconciliationAuthorityV1,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        if let Err(error) = self.ensure_open() {
            return Err(error);
        }
        let dispatch_key = authority.dispatch_key();
        let validate_predecessor_ordinal = authority.validate_predecessor_ordinal();
        let tag = authority.tag();
        let subject = authority.subject();
        let certificate = authority.certificate().clone();
        let validated_receipt = authority.validated_receipt().clone();
        let durable_receipt = validated_receipt.durable();
        let decision = (
            certificate.round,
            certificate.proposal_round,
            subject,
            certificate.execution_commitment,
        );
        let body_key = (certificate.proposal_round, subject);
        let retry_parent_is_inert = match (
            self.durable_validate_retry_seals.get(&body_key),
            self.published_lifecycle_validate_retry_markers
                .get(&body_key),
        ) {
            (Some(seal), None) => seal.lifecycle_ordinal().is_none(),
            (None, Some(marker)) => !marker.owns_live_lifecycle_row(),
            (None, None) => true,
            (Some(_), Some(_)) => false,
        };
        let live_apply_owner_already_exact = self
            .live_lifecycle_decision_apply
            .as_ref()
            .is_some_and(|existing| {
                existing.exactly_matches(
                    dispatch_key,
                    tag,
                    subject,
                    &certificate,
                    &validated_receipt,
                    decision,
                )
            });
        let validate_retry_authority_is_exact = self
            .exactly_owns_validate_retry_lifecycle_ordinal(body_key, validate_predecessor_ordinal);
        let validate_retry_authority_is_absent =
            !self.durable_validate_retry_seals.contains_key(&body_key)
                && !self
                    .published_lifecycle_validate_retry_markers
                    .contains_key(&body_key);
        let preliminary_owner_is_exact = match self.live_lifecycle_validate_successor.as_ref() {
            Some(owner) if owner.exactly_precedes_live_apply(&authority) => {
                self.retained_effect_batch.as_ref().is_none_or(|batch| {
                    batch.effects.len() == 1
                        && batch.effects.front().is_some_and(|owned| {
                            authority.exactly_matches_owned_apply(&owned.effect, &owned.ownership)
                        })
                })
            }
            Some(_) => false,
            None => self.retained_effect_batch.is_none() && retry_parent_is_inert,
        };
        let runtime_decision = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime);
        let preflight = runtime_decision.and_then(|runtime_decision| {
            if dispatch_key.lineage() != LifecycleDecisionApplyLineageV1::Live
                || !dispatch_key.matches_height_context(&self.context)
                || dispatch_key.lifecycle_ordinal() == 0
                || validate_predecessor_ordinal == 0
                || validate_predecessor_ordinal >= dispatch_key.lifecycle_ordinal()
                || tag.height() != self.context.height
                || self.runtime.authoritative_tag() != Some(tag)
                || !preliminary_owner_is_exact
                || self.parked_effect_batch.is_some()
                || self.pending_tip_recovery.is_some()
                || self.finality_completion.is_some()
                || certificate.validate(&self.context).is_err()
                || certificate.phase != wire::GlobalPhase::Commit
                || certificate.round.context_id != self.context.id()
                || certificate.round.height != self.context.height
                || certificate.subject != subject
                || durable_receipt.context_id() != self.context.id()
                || durable_receipt.round() != certificate.proposal_round
                || durable_receipt.subject() != subject
                || validated_receipt.execution_commitment() != certificate.execution_commitment
                || self.durable_bodies.get(&body_key) != Some(durable_receipt)
                || self.validated_bodies.get(&body_key) != Some(&validated_receipt)
                || !(validate_retry_authority_is_exact
                    || (live_apply_owner_already_exact && validate_retry_authority_is_absent))
                || runtime_decision != Some(decision)
                || self.protected_decision != Some(decision)
                || (self.live_lifecycle_decision_apply.is_some() && !live_apply_owner_already_exact)
            {
                Err(EffectExecutorError::Contract(
                    "live lifecycle Apply cleanup authority differs from the exact decided body"
                        .to_owned(),
                ))
            } else {
                Ok(())
            }
        });
        if let Err(error) = preflight {
            return Err(self.close(error, services));
        }
        if let Err(error) = self.reconcile_decision_work(decision, true, services) {
            return Err(self.close(error, services));
        }
        if self.live_lifecycle_validate_successor.is_some() {
            let retained_is_exact = self.retained_effect_batch.as_ref().is_none_or(|batch| {
                batch.effects.len() == 1
                    && batch.effects.front().is_some_and(|owned| {
                        authority.exactly_matches_owned_apply(&owned.effect, &owned.ownership)
                    })
            });
            let validate_ordinal =
                self.live_lifecycle_validate_successor
                    .as_ref()
                    .and_then(|owner| {
                        owner
                            .exactly_precedes_live_apply(&authority)
                            .then_some(owner.dispatch_key.lifecycle_ordinal())
                    });
            let Some(validate_ordinal) = validate_ordinal else {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "Validate-to-Apply upgrade changed its retained authority".to_owned(),
                    ),
                    services,
                ));
            };
            if !retained_is_exact {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "Validate-to-Apply upgrade changed its retained authority".to_owned(),
                    ),
                    services,
                ));
            }
            if let Err(error) = self.release_live_lifecycle_validate_successor(
                validate_ordinal,
                LifecycleValidateRetryResolutionV1::AdvancedToSuccessor,
            ) {
                return Err(self.close(error, services));
            }
            self.retained_effect_batch = None;
        } else {
            match self
                .release_validate_retry_lifecycle_ordinal(body_key, validate_predecessor_ordinal)
            {
                Ok(true) => {}
                Ok(false) if live_apply_owner_already_exact => {}
                Ok(false) => {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "live Apply lost its exact Validate retry authority".to_owned(),
                        ),
                        services,
                    ));
                }
                Err(error) => return Err(self.close(error, services)),
            }
        }
        if self.protected_decision != Some(decision)
            || !self.decision_body_drained
            || self.pending_work() != 0
            || !self.recovered_decision_fetch_request_index_is_exact_and_empty()
            || !self.certified_work.is_empty()
            || !self.outstanding_requests.is_empty()
            || self.retained_effect_batch.is_some()
            || self.parked_effect_batch.is_some()
            || self.pending_tip_recovery.is_some()
            || self.finality_completion.is_some()
            || self.durable_bodies.get(&body_key) != Some(durable_receipt)
            || self.validated_bodies.get(&body_key) != Some(&validated_receipt)
        {
            return Err(self.close(
                EffectExecutorError::Contract(
                    "live lifecycle Apply cleanup left competing or changed decided-body ownership"
                        .to_owned(),
                ),
                services,
            ));
        }
        if self.live_lifecycle_decision_apply.is_none() {
            self.live_lifecycle_decision_apply = Some(LiveLifecycleDecisionApplyOwnerV1 {
                dispatch_key,
                tag,
                subject,
                certificate,
                validated_receipt,
                decision,
            });
        }
        Ok(())
    }

    /// Recheck that one Ready live Apply is already protected by the exact
    /// post-publication executor owner. Scheduler classification is read-only:
    /// it must never perform cleanup or mint this owner for an unrelated row.
    pub(in crate::sumeragi) fn exactly_owns_live_lifecycle_decision_apply(
        &self,
        authority: &LiveLifecycleDecisionApplyReconciliationAuthorityV1,
    ) -> bool {
        let certificate = authority.certificate();
        let subject = authority.subject();
        self.live_lifecycle_decision_apply
            .as_ref()
            .is_some_and(|owner| {
                owner.exactly_matches(
                    authority.dispatch_key(),
                    authority.tag(),
                    subject,
                    certificate,
                    authority.validated_receipt(),
                    (
                        certificate.round,
                        certificate.proposal_round,
                        subject,
                        certificate.execution_commitment,
                    ),
                )
            })
    }

    /// Freeze the exact executor/runtime around one lifecycle-owned Apply completion.
    pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(
        &mut self,
        authority: LifecycleDecisionApplyAdapterCompletionAuthorityV1,
    ) -> Result<PreparedLifecycleDecisionApplyAdapterCompletionV1<'_>, EffectExecutorError> {
        self.ensure_open()?;
        if authority.lineage() == LifecycleDecisionApplyLineageV1::Recovered {
            let validate_key = (
                authority.artifact().commit_qc.proposal_round,
                authority.subject(),
            );
            self.preflight_recovered_apply_validate_retry_predecessor(
                authority.dispatch_key(),
                validate_key,
                authority.validate_predecessor_ordinal(),
            )?;
        }
        let pending_recovery_is_exact = self.pending_tip_recovery.as_ref().is_none_or(|evidence| {
            authority.exactly_matches_pending_kura_recovery(&self.context, evidence)
        });
        let lineage_owner_is_exact = match authority.lineage() {
            LifecycleDecisionApplyLineageV1::Live => self
                .live_lifecycle_decision_apply
                .as_ref()
                .is_some_and(|owner| {
                    owner.exactly_matches_completion(
                        authority.dispatch_key(),
                        authority.tag(),
                        authority.subject(),
                        authority.receipt(),
                        authority.artifact(),
                    )
                }),
            LifecycleDecisionApplyLineageV1::Recovered => {
                self.live_lifecycle_decision_apply.is_none()
            }
        };
        let successor_outputs_are_exact =
            match self.lifecycle_decision_apply_successor_outputs.as_ref() {
                None => self.pending_lifecycle_output_admissions.is_empty(),
                Some(attestation) => {
                    attestation.dispatch_key() == authority.dispatch_key()
                        && attestation.pending_count()
                            == self.pending_lifecycle_output_admissions.len()
                        && attestation.exactly_matches_pending_keys(
                            self.pending_lifecycle_output_admissions.keys(),
                        )
                }
            };
        if self.pending_work() != self.pending_lifecycle_output_admissions.len()
            || !successor_outputs_are_exact
            || self.pending_runner_decision_cleanup.is_some()
            || !self.recovered_decision_fetch_request_index_is_exact_and_empty()
            || self.retained_effect_batch.is_some()
            || self.parked_effect_batch.is_some()
            || !pending_recovery_is_exact
            || self.finality_completion.is_some()
            || self.runtime.queued_commands() != 0
            || !lineage_owner_is_exact
        {
            return Err(EffectExecutorError::Contract(
                "lifecycle Decision Apply completion overtook retained executor work".to_owned(),
            ));
        }
        self.runtime
            .prepare_lifecycle_decision_apply_completion(authority)
            .map_err(|error| EffectExecutorError::Runtime(error.to_string()))
    }
    /// Install post-Ledger lifecycle Apply finality with no fallible tail.
    pub(in crate::sumeragi) fn commit_lifecycle_decision_apply_finality(
        &mut self,
        finality: LifecycleDecisionApplyAdapterFinalityV1,
    ) -> wire::SumeragiV2Status {
        let (dispatch_key, validate_predecessor_ordinal, tag, receipt, artifact, committed_status) =
            finality.consume_for_executor(LifecycleDecisionApplyExecutorFinalityPermitV1::new());
        let pending_recovery_is_exact = self.pending_tip_recovery.as_ref().is_none_or(|evidence| {
            dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Recovered
                && evidence.stage() == PendingKuraApplyRecoveryStage::ApplicationDispatched
                && evidence.is_exact(&self.context)
                && tag == evidence.replay_tag()
                && artifact.subject == evidence.commit_subject()
                && &artifact.commit_qc == evidence.commit_qc()
                && receipt.height() == evidence.frozen_height()
                && receipt.context_id() == evidence.frozen_context_id()
                && receipt.block_hash() == evidence.commit_subject().block_hash
                && receipt.subject() == evidence.commit_subject()
                && receipt.certificate() == evidence.commit_qc().as_ref()
                && receipt.artifact_hash() == HashOf::new(&artifact)
        });
        let lineage_owner_is_exact = match dispatch_key.lineage() {
            LifecycleDecisionApplyLineageV1::Live => self
                .live_lifecycle_decision_apply
                .take()
                .is_some_and(|owner| {
                    owner.exactly_matches_completion(
                        dispatch_key,
                        tag,
                        receipt.subject(),
                        &receipt,
                        &artifact,
                    )
                }),
            LifecycleDecisionApplyLineageV1::Recovered => {
                self.live_lifecycle_decision_apply.is_none()
            }
        };
        let successor_outputs_are_exact =
            match self.lifecycle_decision_apply_successor_outputs.as_ref() {
                None => self.pending_lifecycle_output_admissions.is_empty(),
                Some(attestation) => {
                    attestation.dispatch_key() == dispatch_key
                        && attestation.pending_count()
                            == self.pending_lifecycle_output_admissions.len()
                        && attestation.exactly_matches_pending_keys(
                            self.pending_lifecycle_output_admissions.keys(),
                        )
                }
            };
        assert!(
            lineage_owner_is_exact
                && self.finality_completion.is_none()
                && self.pending_work() == self.pending_lifecycle_output_admissions.len()
                && successor_outputs_are_exact
                && self.pending_runner_decision_cleanup.is_none()
                && self.recovered_decision_fetch_request_index_is_exact_and_empty()
                && pending_recovery_is_exact
                && dispatch_key.matches_height_context(&self.context)
                && artifact.height_context == self.context
                && artifact.subject == receipt.subject()
                && receipt.context_id() == self.context.id()
                && receipt.height() == self.context.height
                && receipt.artifact_hash() == HashOf::new(&artifact)
                && self.runtime.driver().ready_to_finish(),
            "pre-Ledger lifecycle Apply finality proof remains exact"
        );
        if dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Recovered {
            self.release_recovered_apply_validate_retry_predecessor(
                dispatch_key,
                (artifact.commit_qc.proposal_round, artifact.subject),
                validate_predecessor_ordinal,
            )
            .expect("preflighted recovered Apply Validate predecessor remains exact");
        }
        self.finality_completion = Some(FinalityCompletion {
            tag,
            receipt,
            artifact,
            ownership: FinalityCompletionOwner::LifecycleDecisionApply(dispatch_key),
        });
        if let Some(evidence) = self.pending_tip_recovery.as_mut() {
            evidence.stage = PendingKuraApplyRecoveryStage::Completed;
            self.pending_tip_recovery_last_result =
                Some(PendingTipRecoveryAttemptResult::Completed);
        }
        committed_status
    }
    /// Whether production may consume and register another local proposal.
    ///
    /// Capacity alone is insufficient after a Proposal has reached guarded
    /// fanout: that transition consumes the armed view's one-shot producer.
    /// A same-view lock update can retire the runner's prior candidate owner,
    /// so consult the serialized reservation before consuming replacement
    /// bytes. A clean `false` leaves the pacemaker free to enter the next view;
    /// invalid reservation evidence remains a runtime error.
    pub(crate) fn can_schedule_local_proposal(&mut self) -> Result<bool, EffectExecutorError> {
        self.ensure_open()?;
        if !self.can_admit_local_proposal() {
            return Ok(false);
        }
        let tag = self.current_tag();
        self.runtime
            .local_proposal_admission_available(tag)
            .map_err(EffectExecutorError::Runtime)
    }
    /// Prepare the reducer status installed only when this height's live
    /// activation boundary succeeds.
    pub(crate) fn successor_activation_status_snapshot(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        self.runtime.successor_activation_status_snapshot()
    }

    /// Snapshot one completed interrupted tip while pacemaker clocks stay unarmed.
    pub(in crate::sumeragi) fn pending_kura_activation_status_snapshot(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        let pending_ready = self.ready_to_finish()
            && self.lifecycle_live_clocks_are_unarmed()
            && self.pending_tip_recovery.as_ref().is_some_and(|evidence| {
                evidence.stage() == PendingKuraApplyRecoveryStage::Completed
            });
        if !pending_ready {
            return Err(AdapterError::PendingKuraActivationNotReady);
        }
        self.runtime.pending_kura_activation_status_snapshot()
    }

    /// Bind an interrupted Kura tip to the exact reducer Decision and durable
    /// validation marker reconstructed before network ingress opens.
    ///
    /// This must be called immediately after [`Self::open`] whenever recovery
    /// returns a [`PendingKuraApply`]. A missing Decision, a different block,
    /// or absent exact body/validation durability fails closed before the
    /// startup effects can be dispatched. Exact height-one replay returns a
    /// capability binding the frozen Nexus/AMX projection for pre-apply lane
    /// work; other heights return `None`. The startup batch must contain the
    /// sole certified `FetchBody` reconstructed from the Decision. Its full
    /// CommitQC and reducer incarnation seed the closed-ingress
    /// Fetch → Store → Validate → Apply stage machine.
    pub(crate) fn verify_pending_kura_apply_replay(
        &mut self,
        expected: PendingKuraApply,
        startup_effects: &[AdapterEffect],
        deferred_validated_marker: super::v2::DeferredPendingKuraValidatedMarkerV1,
    ) -> Result<Option<VerifiedPendingGenesisNexusAmxContext>, EffectExecutorError> {
        self.ensure_open()?;
        if self.pending_tip_recovery.is_some() {
            return Err(EffectExecutorError::PendingApplyRecoveryMismatch(
                "interrupted Kura tip replay was verified more than once".to_owned(),
            ));
        }
        let decision = self.runtime.replayed_decision_key().map_err(|error| {
            EffectExecutorError::PendingApplyRecoveryMismatch(error.to_string())
        })?;
        let [
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                manifest,
                certified_sources,
                certificate: Some(certificate),
            },
        ] = startup_effects
        else {
            return Err(EffectExecutorError::PendingApplyRecoveryMismatch(
                "replayed Decision must produce exactly one certified FetchBody before ingress"
                    .to_owned(),
            ));
        };
        let owner_tag = self.current_tag();
        if *tag != owner_tag
            || *round != certificate.proposal_round
            || *subject != certificate.subject
        {
            return Err(EffectExecutorError::PendingApplyRecoveryMismatch(
                "replayed certified FetchBody changed its Decision tag, body origin, or subject"
                    .to_owned(),
            ));
        }
        let expected_sources = self.frozen_archive_sources();
        if certified_sources != &expected_sources {
            return Err(EffectExecutorError::PendingApplyRecoveryMismatch(
                "replayed certified FetchBody changed the canonical frozen-roster archive sources"
                    .to_owned(),
            ));
        }
        self.runtime
            .verify_certificate(&self.context, certificate)
            .map_err(EffectExecutorError::PendingApplyRecoveryMismatch)?;
        let (genesis_context, evidence) = verify_pending_kura_apply_parts_with_marker(
            &self.context,
            decision,
            &self.recovered_bodies,
            &self.validated_bodies,
            expected,
            *tag,
            owner_tag,
            certificate.clone(),
            manifest.as_ref(),
            deferred_validated_marker,
        )?;
        if evidence.durable_round() != *round {
            return Err(EffectExecutorError::PendingApplyRecoveryMismatch(
                "replayed certified FetchBody did not select the exact recovered body origin"
                    .to_owned(),
            ));
        }
        self.pending_tip_recovery = Some(evidence);
        Ok(genesis_context)
    }
    /// Authenticate and enqueue one reducer-directed v2 network message while
    /// preserving the exact fair-ingress owner through serialized dispatch.
    pub(crate) fn enqueue_network_with_ingress_ownership(
        &mut self,
        message: wire::ConsensusMessageV2,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<EventTag, NetworkIngressError> {
        if self.fatal_reason.is_some() || self.output_guard.restart_required() {
            return Err(NetworkIngressError::FailClosed);
        }
        let result = self
            .runtime
            .enqueue_network_with_ingress_ownership(message, ingress_ownership);
        if matches!(&result, Err(NetworkIngressError::FailClosed)) {
            self.output_guard.activate_restart_required();
            self.fatal_reason.get_or_insert_with(|| {
                "Sumeragi v2 runtime rejected authenticated ingress ownership".to_owned()
            });
        }
        result
    }
    /// Admit an authenticated block-sync CommitQC and return exact one-shot ownership evidence.
    ///
    /// The evidence is minted only after serialized reducer admission succeeds and is bound to
    /// the complete canonical message. Discovery uses it to retire the matching request.
    pub(crate) fn enqueue_discovered_commit_certificate(
        &mut self,
        message: wire::ConsensusMessageV2,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<CommitCertificateReducerAdmission, NetworkIngressError> {
        let wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) = &message.payload
        else {
            return Err(NetworkIngressError::TransportPayload);
        };
        if certificate.phase != wire::GlobalPhase::Commit {
            return Err(NetworkIngressError::Authentication(
                AdapterError::DurableCommitMismatch,
            ));
        }
        let message_hash = HashOf::new(&message);
        let _tag = self.enqueue_network_with_ingress_ownership(message, ingress_ownership)?;
        Ok(CommitCertificateReducerAdmission { message_hash })
    }
    /// Test-only direct helper. Production must provide the ownership carrier
    /// produced by fair authenticated ingress.
    #[cfg(test)]
    pub(crate) fn enqueue_network(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<EventTag, NetworkIngressError> {
        if self.fatal_reason.is_some() || self.output_guard.restart_required() {
            return Err(NetworkIngressError::FailClosed);
        }
        let result = self.runtime.enqueue_network(message);
        if matches!(&result, Err(NetworkIngressError::FailClosed)) {
            self.output_guard.activate_restart_required();
            self.fatal_reason.get_or_insert_with(|| {
                "Sumeragi v2 runtime rejected authenticated ingress ownership".to_owned()
            });
        }
        result
    }
    /// Return the exact reducer incarnation currently owning timers and work.
    pub(crate) const fn current_tag(&self) -> EventTag {
        self.runtime.round_tag()
    }
    /// Whether body-store open routed one nondeferred validation marker into the runtime.
    #[cfg(test)]
    pub(in crate::sumeragi) fn recovered_validated_body_was_bound_for_test(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
    ) -> bool {
        self.runtime
            .recovered_validated_body_was_bound_for_test(key)
    }
    /// Rebuild an ownerless cold Apply executor's Decision protection from its
    /// real reopened runtime without selecting a new Runtime turn.
    ///
    /// Production has already crossed this reconciliation before a synchronous
    /// Ready Validate-to-Apply publication. Focused reopen tests use this seam
    /// instead of manufacturing a later periodic Apply with a different
    /// pending-effect identity, and name whether their exact cut already owns
    /// the preliminary queued-successor fence.
    #[cfg(test)]
    pub(in crate::sumeragi) fn reconcile_reopened_decision_for_lifecycle_apply_lineage_test<
        S: V2EffectServices,
    >(
        &mut self,
        services: &mut S,
        preliminary_successor_is_expected: bool,
    ) -> Result<
        (
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        ),
        EffectExecutorError,
    > {
        if !self.runtime.lifecycle_live_clocks_are_armed()
            || self.live_lifecycle_validate_successor.is_some() != preliminary_successor_is_expected
            || self.live_lifecycle_decision_apply.is_some()
            || self.protected_decision.is_some()
            || self.pending_runner_decision_cleanup.is_some()
            || self.decision_body_drained
            || self.pending_work() != 0
            || !self.recovered_decision_fetch_request_index_is_exact_and_empty()
            || !self.certified_work.is_empty()
            || !self.outstanding_requests.is_empty()
            || self.retained_effect_batch.is_some()
            || self.parked_effect_batch.is_some()
            || self.pending_tip_recovery.is_some()
            || self.finality_completion.is_some()
            || self.runtime.queued_commands() != 0
            || self.fatal_reason.is_some()
            || self.output_guard.restart_required()
        {
            return Err(EffectExecutorError::Contract(
                "live Apply lineage fixture did not reopen at the exact ownerless cold cut"
                    .to_owned(),
            ));
        }
        let decision = self.reconcile_runtime_decision(services)?.ok_or_else(|| {
            EffectExecutorError::Contract(
                "live Apply fixture runtime omitted its durable Decision".to_owned(),
            )
        })?;
        if self.live_lifecycle_validate_successor.is_some() != preliminary_successor_is_expected
            || self.live_lifecycle_decision_apply.is_some()
            || self.protected_decision != Some(decision)
            || self.pending_runner_decision_cleanup.is_some()
            || self.decision_body_drained
            || self.pending_work() != 0
            || !self.recovered_decision_fetch_request_index_is_exact_and_empty()
            || !self.certified_work.is_empty()
            || !self.outstanding_requests.is_empty()
            || self.retained_effect_batch.is_some()
            || self.parked_effect_batch.is_some()
            || self.pending_tip_recovery.is_some()
            || self.finality_completion.is_some()
            || self.runtime.queued_commands() != 0
            || self.fatal_reason.is_some()
            || self.output_guard.restart_required()
        {
            return Err(EffectExecutorError::Contract(
                "live Apply lineage fixture Decision reconciliation changed its ownerless cold cut"
                    .to_owned(),
            ));
        }
        Ok(decision)
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
    /// Prove that changing only a completion authority's lineage cannot enter
    /// either branch of the shared executor preview or mutate retained state.
    #[cfg(test)]
    pub(in crate::sumeragi) fn assert_lifecycle_apply_completion_lineage_substitution_is_inert_for_test(
        &mut self,
        mut authority: LifecycleDecisionApplyAdapterCompletionAuthorityV1,
        opposite: LifecycleDecisionApplyLineageV1,
    ) {
        let expected = authority.dispatch_key();
        assert_ne!(expected.lineage(), opposite);
        authority.substitute_dispatch_lineage_for_test(opposite);
        assert_eq!(
            authority
                .dispatch_key()
                .with_lineage_for_test(expected.lineage()),
            expected,
            "executor substitution may change only the live/recovered lineage"
        );
        let before = (
            (
                self.live_lifecycle_decision_apply
                    .as_ref()
                    .map(|owner| owner.dispatch_key),
                self.live_lifecycle_validate_successor.is_some(),
                self.pending_work(),
                self.recovered_decision_fetches.len(),
                self.recovered_decision_fetch_by_request.len(),
                self.durable_validate_retry_seals.len(),
                self.retained_effect_batch.is_some(),
                self.parked_effect_batch.is_some(),
                self.pending_tip_recovery.is_some(),
                self.finality_completion.is_some(),
            ),
            (
                self.protected_decision,
                self.decision_body_drained,
                self.next_work_id,
                self.reconciled_tag,
                self.runtime.queued_commands(),
                self.runtime.authoritative_tag(),
                self.runtime
                    .decided_body()
                    .expect("inspect exact lifecycle Apply runtime Decision"),
                self.fatal_reason.clone(),
                self.output_guard.restart_required(),
            ),
        );
        let result = self.prepare_lifecycle_decision_apply_completion(authority);
        assert!(matches!(
            result,
            Err(EffectExecutorError::Contract(ref reason))
                if reason == "lifecycle Decision Apply completion overtook retained executor work"
        ));
        let after = (
            (
                self.live_lifecycle_decision_apply
                    .as_ref()
                    .map(|owner| owner.dispatch_key),
                self.live_lifecycle_validate_successor.is_some(),
                self.pending_work(),
                self.recovered_decision_fetches.len(),
                self.recovered_decision_fetch_by_request.len(),
                self.durable_validate_retry_seals.len(),
                self.retained_effect_batch.is_some(),
                self.parked_effect_batch.is_some(),
                self.pending_tip_recovery.is_some(),
                self.finality_completion.is_some(),
            ),
            (
                self.protected_decision,
                self.decision_body_drained,
                self.next_work_id,
                self.reconciled_tag,
                self.runtime.queued_commands(),
                self.runtime.authoritative_tag(),
                self.runtime
                    .decided_body()
                    .expect("reinspect exact lifecycle Apply runtime Decision"),
                self.fatal_reason.clone(),
                self.output_guard.restart_required(),
            ),
        );
        assert_eq!(
            after, before,
            "completion-authority lineage substitution must leave executor and runtime inert"
        );
    }
    /// List the exact census owners preventing the explicit rollover transaction.
    pub(crate) fn ready_to_finish_blockers(&self) -> Vec<&'static str> {
        let mut blockers = Vec::new();
        macro_rules! record {
            ($blocked:expr, $name:literal) => {
                if $blocked {
                    blockers.push($name);
                }
            };
        }
        record!(self.output_guard.restart_required(), "restart-required");
        record!(self.finality_completion.is_none(), "finality-missing");
        record!(
            self.pending_runner_decision_cleanup.is_some(),
            "runner-decision-cleanup"
        );
        record!(
            self.live_lifecycle_decision_apply.is_some(),
            "live-apply-owner"
        );
        record!(
            self.live_lifecycle_validate_successor.is_some(),
            "live-validate-successor"
        );
        record!(
            self.retained_effect_batch.is_some(),
            "retained-effect-batch"
        );
        record!(self.parked_effect_batch.is_some(), "parked-effect-batch");
        record!(
            self.runtime.has_dormant_remote_proposal_replay(),
            "dormant-remote-proposal-replay"
        );
        record!(
            !self.remote_proposal_replay.is_empty(),
            "remote-proposal-replay"
        );
        record!(
            !self.authenticated_genesis_replay.is_empty(),
            "authenticated-genesis-replay"
        );
        record!(
            !self.pending_durable_validate_admissions.is_empty(),
            "durable-validate-admission"
        );
        record!(
            !self.durable_validate_retry_seals_are_finalization_inert(),
            "durable-validate-retry-seal"
        );
        record!(
            self.pending_live_wal_sign_admission.is_some(),
            "live-wal-sign-admission"
        );
        record!(
            !self.pending_lifecycle_output_admissions.is_empty(),
            "lifecycle-output-admission"
        );
        record!(
            self.lifecycle_decision_apply_successor_outputs.is_some(),
            "post-apply-output-census"
        );
        record!(
            !self.recovered_decision_fetch_request_index_is_exact_and_empty(),
            "recovered-decision-fetch"
        );
        record!(self.runtime.queued_commands() != 0, "runtime-command");
        record!(!self.runtime.driver().ready_to_finish(), "runtime-driver");
        blockers
    }

    /// Whether application completion has drained through the reducer and the
    /// height is ready for the explicit rollover transaction.
    pub(crate) fn ready_to_finish(&self) -> bool {
        !self.output_guard.restart_required()
            && self.finality_completion.is_some()
            && self.pending_runner_decision_cleanup.is_none()
            && self.live_lifecycle_decision_apply.is_none()
            && self.live_lifecycle_validate_successor.is_none()
            && self.pending_released_lifecycle_validate_apply.is_none()
            && self.retained_effect_batch.is_none()
            && self.parked_effect_batch.is_none()
            && !self.runtime.has_dormant_remote_proposal_replay()
            && self.remote_proposal_replay.is_empty()
            && self.authenticated_genesis_replay.is_empty()
            && self.pending_durable_validate_admissions.is_empty()
            && self.durable_validate_retry_seals_are_finalization_inert()
            && self.pending_live_wal_sign_admission.is_none()
            && self.pending_lifecycle_output_admissions.is_empty()
            && self.lifecycle_decision_apply_successor_outputs.is_none()
            && self.recovered_decision_fetch_request_index_is_exact_and_empty()
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
    /// Whether executor-owned mutation and handoff state has drained before Apply.
    #[cfg(test)]
    fn lifecycle_decision_apply_executor_owners_are_empty(&self) -> bool {
        self.pending_work() == 0
            && self.pending_runner_decision_cleanup.is_none()
            && self.recovered_decision_fetch_request_index_is_exact_and_empty()
            && self.retained_effect_batch.is_none()
            && self.parked_effect_batch.is_none()
            && self.finality_completion.is_none()
            && self.runtime.queued_commands() == 0
    }

    /// Whether process-local settlement still excludes a Decision Apply dispatch.
    fn decision_apply_dispatch_barrier_is_occupied(&self) -> bool {
        self.pending_runner_decision_cleanup.is_some()
            || !self.pending_durable_validate_admissions.is_empty()
            || self.pending_released_lifecycle_validate_apply.is_some()
            || self.pending_live_wal_sign_admission.is_some()
            || !self.pending_lifecycle_output_admissions.is_empty()
    }

    /// Reconcile the already-durable Decision needed by one pending runner handoff.
    ///
    /// This performs no reducer step and dispatches no effect. It only restores
    /// the executor's exact Decision protection before process-local proposal
    /// and lane owners are retired and the handoff is acknowledged.
    pub(crate) fn reconcile_pending_runner_decision_cleanup<S: V2EffectServices>(
        &mut self,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let Some(pending) = self.pending_runner_decision_cleanup else {
            return Ok(());
        };
        let decision = self.reconcile_runtime_decision(services)?.ok_or_else(|| {
            EffectExecutorError::Contract(
                "runner Decision cleanup lost its durable runtime Decision".to_owned(),
            )
        })?;
        if decision != pending.decision {
            return Err(EffectExecutorError::Contract(
                "runner Decision cleanup changed during executor reconciliation".to_owned(),
            ));
        }
        Ok(())
    }

    /// Release the live Decision-to-Apply fence after the runner synchronously
    /// retires its process-local proposal lease and losing lane work.
    pub(crate) fn acknowledge_runner_decision_cleanup(
        &mut self,
        runner_tag: EventTag,
        decided_subject: Option<wire::BlockSubject>,
    ) -> Result<(), EffectExecutorError> {
        let Some(pending) = self.pending_runner_decision_cleanup else {
            return Ok(());
        };
        let decision = pending.decision;
        let (retained_apply_count, exact_retained_apply_count) = self
            .retained_effect_batch
            .as_ref()
            .into_iter()
            .chain(self.parked_effect_batch.as_ref())
            .flat_map(|batch| batch.effects.iter())
            .fold((0_usize, 0_usize), |(count, exact_count), owned| {
                let AdapterEffect::Apply {
                    tag,
                    subject,
                    certificate,
                } = &owned.effect
                else {
                    return (count, exact_count);
                };
                let exact = *tag == pending.owner_tag
                    && *subject == decision.2
                    && certificate.phase == wire::GlobalPhase::Commit
                    && certificate.round == decision.0
                    && certificate.proposal_round == decision.1
                    && certificate.subject == decision.2
                    && certificate.execution_commitment == decision.3;
                (count + 1, exact_count + usize::from(exact))
            });
        let runtime_decision = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)?;
        if runtime_decision != Some(decision)
            || self.protected_decision != Some(decision)
            || runner_tag != pending.owner_tag
            || self.runtime.authoritative_tag() != Some(pending.owner_tag)
            || pending.owner_tag.height() != decision.0.height
            || decided_subject != Some(decision.2)
            || retained_apply_count > 1
            || retained_apply_count != exact_retained_apply_count
        {
            return Err(EffectExecutorError::Contract(
                "runner Decision cleanup changed the exact Decision handoff".to_owned(),
            ));
        }
        self.pending_runner_decision_cleanup = None;
        Ok(())
    }

    /// Restore one live lifecycle validation marker to the executor catalog.
    ///
    /// Durable validation has already fsynced this receipt. The move-only Ready
    /// authority is available for local and remote origins alike; this join
    /// runs before any successor can emit a Vote and is idempotent for retries.
    fn record_lifecycle_validated_body(
        &mut self,
        authority: super::v2_lifecycle_coordinator::ReadyValidatedExecutorCatalogAuthorityV1,
    ) -> Result<(), EffectExecutorError> {
        let validated = authority.into_validated_receipt();
        let durable = validated.durable();
        let key = (durable.round(), durable.subject());
        let retained_body_is_exact =
            self.recovered_bodies
                .get(&key)
                .is_some_and(|(retained_manifest, retained)| {
                    retained == durable
                        && retained_manifest.round == durable.round()
                        && retained_manifest.subject == durable.subject()
                        && HashOf::new(retained_manifest) == durable.manifest_hash()
                });
        if durable.context_id() != self.context.id()
            || !retained_body_is_exact
            || self.durable_bodies.get(&key) != Some(durable)
        {
            return Err(EffectExecutorError::BodyStore(
                "lifecycle validation marker differs from its exact durable body catalogs"
                    .to_owned(),
            ));
        }
        if self.rejected_bodies.contains_key(&key)
            || self.retired_rejected_bodies.contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "one exact durable body produced both validated and rejected outcomes".to_owned(),
            ));
        }
        if self
            .validated_bodies
            .get(&key)
            .is_some_and(|existing| existing != &validated)
        {
            return Err(EffectExecutorError::BodyStore(
                "one exact durable body produced conflicting validation receipts".to_owned(),
            ));
        }
        let projected_recovered_seal = self
            .durable_validate_retry_seals
            .get(&key)
            .map(|seal| seal.project_recovered_commitment_ceiling(validated.execution_commitment()))
            .transpose()
            .map_err(EffectExecutorError::Contract)?
            .flatten();
        self.validated_bodies.entry(key).or_insert(validated);
        if let Some(seal) = projected_recovered_seal {
            self.durable_validate_retry_seals.insert(key, seal);
        }
        Ok(())
    }
    /// Borrow the immutable context governing this executor height.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
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
        self.runtime.authenticate_certified_body_request(
            &self.context,
            request,
            authenticated_requester,
        )
    }
    /// Return the runtime's durable terminal subject used by fair ingress.
    pub(crate) fn lifecycle_terminal_subject(
        &self,
    ) -> Result<Option<wire::BlockSubject>, EffectExecutorError> {
        self.runtime
            .decided_body()
            .map(|decision| decision.map(|(_, _, subject, _)| subject))
            .map_err(EffectExecutorError::Runtime)
    }
    /// Whether the fair-ingress head can make progress without violating the
    /// retained reducer-effect prefix.
    pub(crate) fn can_admit_network_message_with_ingress_ownership(
        &self,
        message: &wire::ConsensusMessageV2,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        if self.fatal_reason.is_some() || self.output_guard.restart_required() {
            return false;
        }
        let retained_dispatch_allows =
            self.retained_dispatch_allows_network_ingress(&message.payload);
        let timeout_vote_recovery_episode = !retained_dispatch_allows
            && self
                .runtime
                .can_admit_timeout_vote_recovery_episode(message, ingress_ownership);
        (retained_dispatch_allows || timeout_vote_recovery_episode)
            && self
                .runtime
                .can_admit_network_message_with_ingress_ownership(message, ingress_ownership)
    }

    /// Rejoin one launched service to this executor's exact body-store owner.
    ///
    /// Context/root equality is insufficient: a reopened store at the same
    /// path receives a different instance marker. The service must also share
    /// this executor's requester identity and canonical fail-stop output gate.
    pub(in crate::sumeragi) fn matches_recovered_lifecycle_body_service(
        &self,
        context: &wire::HeightContext,
        requester: &PeerId,
        output_guard: &Arc<ConsensusOutputGuard>,
        body_store_identity: &V2BodyStoreInstanceIdentity,
    ) -> bool {
        self.context == *context
            && self.requester == *requester
            && Arc::ptr_eq(&self.output_guard, output_guard)
            && self
                .lifecycle_body_store_identity
                .as_ref()
                .is_some_and(|executor_identity| {
                    executor_identity.same_instance(body_store_identity)
                })
    }
    /// Validate that this executor can mint one ordinary ingress selector cut.
    ///
    /// The caller still needs a complete queue census and per-occurrence
    /// classification. This check only prevents a fatal or restart-required
    /// executor from being represented by an empty/default selector debt.
    pub(crate) fn validate_lifecycle_ingress_selector_authority(
        &self,
    ) -> Result<(), EffectTransportError> {
        if self.output_guard.restart_required() {
            return Err(EffectTransportError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            ));
        }
        if let Some(reason) = &self.fatal_reason {
            return Err(EffectTransportError::FailClosed(reason.clone()));
        }
        Ok(())
    }
    /// Validate every active certified-request owner and report exact presence.
    ///
    /// `false` is returned only when the tracker, work index, and every
    /// pending-fetch reverse owner are all empty. Any missing, duplicate, or
    /// conflicting edge fails closed instead of authorizing zero selector
    /// debt from one incomplete index.
    pub(crate) fn validated_certified_request_presence(
        &self,
    ) -> Result<bool, EffectTransportError> {
        if !self.outstanding_requests.validate_exact_indexes() {
            return Err(EffectTransportError::FailClosed(
                "certified request tracker indexes are not an exact bounded cut".to_owned(),
            ));
        }
        if self.recovered_decision_fetches.len() != self.recovered_decision_fetch_by_request.len()
            || self.recovered_decision_fetches.len() > 1
            || self
                .outstanding_requests
                .len()
                .checked_add(self.recovered_decision_fetches.len())
                .is_none_or(|owned| owned > self.config.max_certified_requests)
        {
            return Err(EffectTransportError::FailClosed(
                "recovered Decision Fetch indexes are not one exact bounded cut".to_owned(),
            ));
        }
        let mut recovered_hashes = BTreeSet::new();
        for (key, owner) in &self.recovered_decision_fetches {
            let request_hash = owner.request_hash();
            if owner.dispatch_key() != *key
                || !owner.validates_exact_executor_context(&self.context, &self.requester)
                || self.recovered_decision_fetch_by_request.get(&request_hash) != Some(key)
                || !recovered_hashes.insert(request_hash)
                || self.certified_work.contains_key(&request_hash)
                || self.outstanding_requests.contains(request_hash)
                || owner.conflicts_with_ordinary_tracker(&self.outstanding_requests)
                || self.pending_fetches.values().any(|pending| {
                    owner.matches_body_coordinates(pending.task.round, pending.task.subject)
                })
            {
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(request_hash),
                ));
            }
        }
        for (request_hash, key) in &self.recovered_decision_fetch_by_request {
            if self
                .recovered_decision_fetches
                .get(key)
                .is_none_or(|owner| owner.request_hash() != *request_hash)
            {
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(*request_hash),
                ));
            }
        }
        let mut pending_hashes = BTreeSet::new();
        for (work_id, pending) in &self.pending_fetches {
            let sidecar_hash = pending.request_hash;
            let task_hash = pending.task.certified_request().map(HashOf::new);
            if sidecar_hash != task_hash {
                let request_hash = sidecar_hash.or(task_hash).ok_or_else(|| {
                    EffectTransportError::FailClosed(
                        "certified request presence lost its exact reverse hash".to_owned(),
                    )
                })?;
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(request_hash),
                ));
            }
            let Some(request_hash) = sidecar_hash else {
                continue;
            };
            if pending.task.id() != *work_id
                || !pending_hashes.insert(request_hash)
                || self.certified_work.get(&request_hash) != Some(work_id)
                || !self.outstanding_requests.contains(request_hash)
            {
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(request_hash),
                ));
            }
        }
        for (request_hash, work_id) in &self.certified_work {
            let Some(pending) = self.pending_fetches.get(work_id) else {
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(*request_hash),
                ));
            };
            if pending.request_hash != Some(*request_hash)
                || pending.task.certified_request().map(HashOf::new) != Some(*request_hash)
                || !self.outstanding_requests.contains(*request_hash)
            {
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(*request_hash),
                ));
            }
        }
        if pending_hashes.len() != self.certified_work.len()
            || pending_hashes.len() != self.outstanding_requests.len()
        {
            return Err(EffectTransportError::FailClosed(
                "certified request indexes have different exact owner counts".to_owned(),
            ));
        }
        Ok(!pending_hashes.is_empty() || !recovered_hashes.is_empty())
    }
    /// Freeze the receiver-local physical predecessor cut used by any producer
    /// continuation created during the next serialized runtime turn.
    pub(crate) fn set_ingress_physical_cut(
        &mut self,
        physical_cut: u128,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        self.runtime
            .set_ingress_physical_cut(physical_cut)
            .map_err(EffectExecutorError::Runtime)
    }
    /// Return the immutable archive fanout in frozen roster order.
    fn frozen_archive_sources(&self) -> Vec<PeerId> {
        self.context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect()
    }
    /// Borrow the complete process-local evidence for interrupted-tip replay.
    pub(crate) const fn pending_kura_apply_recovery_evidence(
        &self,
    ) -> Option<&PendingKuraApplyRecoveryEvidence> {
        self.pending_tip_recovery.as_ref()
    }
    /// Return whether retained reducer dispatch debt permits this outer
    /// ingress envelope to reach its handler.
    ///
    /// The exact protected Progress roots may enter the runtime while an
    /// ordinary effect suffix is retained: the typed pacemaker turn parks that
    /// suffix before selecting them. This only opens authentication and
    /// bounded runtime admission; it does not let the raw wire execute reducer
    /// control directly.
    fn retained_dispatch_allows_network_ingress(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        network_ingress_is_certified_fence_escape(payload)
            || self
                .runtime
                .wire_ingress_may_use_pacemaker_progress(payload)
            || (self.retained_effect_batch.is_none() && self.parked_effect_batch.is_none()
                || !Self::network_ingress_requires_reducer_order(payload))
    }
    /// Return whether handling this outer ingress envelope can execute reducer
    /// control and must therefore stay behind retained effect debt. Transport
    /// completions may enqueue a trusted `BodyAvailable` command, but remain
    /// admissible because they only discharge already-owned recovery work.
    fn network_ingress_requires_reducer_order(payload: &wire::ConsensusMessageV2Payload) -> bool {
        match payload {
            wire::ConsensusMessageV2Payload::Proposal(_)
            | wire::ConsensusMessageV2Payload::Vote(_)
            | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutVote(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => true,
            wire::ConsensusMessageV2Payload::PayloadManifest(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => false,
        }
    }
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
        mut runtime: R,
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
        runtime
            .configure_external_lifecycle_owner_capacity(config.max_pending_work)
            .map_err(EffectExecutorError::Runtime)?;
        let reconciled_tag = runtime.authoritative_tag();
        let outstanding_requests =
            OutstandingCertifiedBodyRequests::new(config.max_certified_requests)
                .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        Ok(Self {
            runtime,
            output_guard,
            lifecycle_body_store_identity: None,
            recovered_bodies,
            context,
            requester,
            local_validator,
            config,
            next_work_id: 0,
            pending_signatures: BTreeMap::new(),
            pending_fetches: BTreeMap::new(),
            pending_stores: BTreeMap::new(),
            local_store_replay: BTreeMap::new(),
            remote_proposal_replay: BTreeMap::new(),
            authenticated_genesis_replay: BTreeMap::new(),
            pending_durable_validate_admissions: BTreeMap::new(),
            durable_validate_retry_seals: BTreeMap::new(),
            published_lifecycle_store_retry_markers: BTreeMap::new(),
            published_lifecycle_validate_retry_markers: BTreeMap::new(),
            pending_released_lifecycle_validate_apply: None,
            #[cfg(test)]
            last_recovered_validate_retry_trace_root: None,
            #[cfg(test)]
            last_recovered_validate_retry_trace_ordinal: None,
            #[cfg(test)]
            last_runtime_step_observation: None,
            pending_live_wal_sign_admission: None,
            pending_lifecycle_output_admissions: BTreeMap::new(),
            lifecycle_decision_apply_successor_outputs: None,
            local_proposal_ready_replay: BTreeMap::new(),
            local_proposal_intent_replay: BTreeMap::new(),
            deferred_merge_work: BTreeMap::new(),
            pending_applications: BTreeMap::new(),
            body_pipeline_owners: BTreeMap::new(),
            certified_work: BTreeMap::new(),
            outstanding_requests,
            recovered_decision_fetches: BTreeMap::new(),
            recovered_decision_fetch_by_request: BTreeMap::new(),
            ready_bodies: BTreeMap::new(),
            reconciled_tag,
            protected_lock: None,
            protected_decision: None,
            pending_runner_decision_cleanup: None,
            live_lifecycle_decision_apply: None,
            live_lifecycle_validate_successor: None,
            pending_tip_recovery: None,
            pending_tip_recovery_attempts: 0,
            pending_tip_recovery_last_result: None,
            decision_body_drained: false,
            authenticated_genesis_body: None,
            retained_locked_body: None,
            ready_body_bytes: 0,
            pending_store_bytes: 0,
            durable_bodies: BTreeMap::new(),
            validated_bodies: BTreeMap::new(),
            rejected_bodies: BTreeMap::new(),
            retired_rejected_bodies: BTreeMap::new(),
            finality_completion: None,
            retained_effect_batch: None,
            parked_effect_batch: None,
            fatal_reason: None,
        })
    }
    /// Install the crash-recovered validation catalog together with the exact
    /// durable receipts that authorize it.
    ///
    /// Authenticated reducer/Decision replay may consume a semantically
    /// restored validation marker directly. A local body with no durable
    /// ProposalIntent deliberately does not: it crosses one idempotent Store
    /// and real Validate worker to regenerate its non-serializable replay
    /// lineage. Keep the authority-bearing and denial-only catalogs distinct
    /// at this boundary.
    fn install_recovered_validation_catalog(
        &mut self,
        recovered_validations: BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            ValidatedBodyReceipt,
        >,
        recovered_rejections: BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            DurableBodyReceipt,
        >,
        retired_recovered_rejections: BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            DurableBodyReceipt,
        >,
    ) -> Result<(), EffectExecutorError> {
        let mut recovered_durable_bodies = BTreeMap::new();
        for (key, (manifest, durable_receipt)) in &self.recovered_bodies {
            if *key != (manifest.round, manifest.subject)
                || *key != (durable_receipt.round(), durable_receipt.subject())
                || !store_completion_matches(&self.context, manifest, durable_receipt)
            {
                return Err(EffectExecutorError::BodyStore(
                    "recovered durable body catalog is internally inconsistent".to_owned(),
                ));
            }
            recovered_durable_bodies.insert(*key, durable_receipt.clone());
        }
        for (key, validated_receipt) in &recovered_validations {
            let Some((_, durable_receipt)) = self.recovered_bodies.get(key) else {
                return Err(EffectExecutorError::BodyStore(
                    "validated recovery marker has no exact durable body".to_owned(),
                ));
            };
            if validated_receipt.durable() != durable_receipt {
                return Err(EffectExecutorError::BodyStore(
                    "validated recovery marker differs from its durable body".to_owned(),
                ));
            }
        }
        for (key, rejected_receipt) in &recovered_rejections {
            let Some((_, durable_receipt)) = self.recovered_bodies.get(key) else {
                return Err(EffectExecutorError::BodyStore(
                    "rejected recovery marker has no exact durable body".to_owned(),
                ));
            };
            if rejected_receipt != durable_receipt || recovered_validations.contains_key(key) {
                return Err(EffectExecutorError::BodyStore(
                    "rejected recovery marker differs from its durable body".to_owned(),
                ));
            }
        }
        for (key, rejected_receipt) in &retired_recovered_rejections {
            let Some((_, durable_receipt)) = self.recovered_bodies.get(key) else {
                return Err(EffectExecutorError::BodyStore(
                    "retired rejection marker has no exact durable body".to_owned(),
                ));
            };
            if rejected_receipt != durable_receipt
                || recovered_validations.contains_key(key)
                || recovered_rejections.contains_key(key)
            {
                return Err(EffectExecutorError::BodyStore(
                    "retired rejection marker differs from its durable body".to_owned(),
                ));
            }
        }
        self.durable_bodies = recovered_durable_bodies;
        self.validated_bodies = recovered_validations;
        self.rejected_bodies = recovered_rejections;
        self.retired_rejected_bodies = retired_recovered_rejections;
        Ok(())
    }
    /// Whether a new local proposal can reserve its first exact-body work owner.
    ///
    /// The production preflight combines this capacity check with the
    /// serialized active-view producer reservation before consuming a prepared
    /// candidate or registering outbound payload bytes. Reducer retransmission
    /// and local completions continue while admission is deferred. This
    /// capacity rule remains runtime-independent so deterministic executor
    /// tests can exercise the same resource boundary.
    pub(crate) fn can_admit_local_proposal(&self) -> bool {
        self.fatal_reason.is_none()
            && !self.output_guard.restart_required()
            && self.retained_effect_batch.is_none()
            && self.parked_effect_batch.is_none()
            && self.pending_work() < self.config.max_pending_work
    }
    /// Exact runtime FIFO capacity currently available to trusted completions.
    pub(crate) fn remaining_completion_capacity(&self) -> usize {
        self.runtime.remaining_completion_capacity()
    }
    /// Reconcile recovery ownership with the reducer's exact monotonic PrepareQC lock.
    ///
    /// A higher lock can be installed without changing the reducer [`EventTag`]. Before the
    /// runner stages bytes for that lock, every round-bound owner of the superseded lock must be
    /// retired. The raw subject-byte cache is retained only when the exact
    /// subject is unchanged; it does not authorize a different proposal round.
    /// Publishing the replacement rank last also prevents a delayed observation
    /// of an older lock from reclaiming the cache after `A -> B` reconciliation.
    pub(crate) fn reconcile_locked_body_for_recovery<S: V2EffectServices>(
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
        let frontier = self
            .runtime
            .reconciliation_frontier()
            .map_err(EffectExecutorError::Runtime)?;
        self.preflight_highest_prepare_frontier(Some(tag), frontier.highest_prepare)?;
        let changed = match self.reconcile_protected_lock(
            tag,
            Some(lock),
            highest_prepare_body(frontier.highest_prepare),
            services,
        ) {
            Ok(changed) => changed,
            Err(error) => {
                return Err(self.close_after_transferring_runtime_terminals(error, services));
            }
        };
        if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
            return Err(self.close(error, services));
        }
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
        highest_prepare_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
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
        // This method is also entered directly by locked-origin recovery,
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
        let key_is_superseded = |round, subject| {
            protected_lock_retires_body_key(
                superseded,
                (replacement_round, replacement_subject),
                (round, subject),
            )
        };
        // The durable high-water mark is cleanup authority only. It may keep
        // one immutable Store task/replay alive while an older TC-carried
        // PrepareQC becomes the voting lock, but it cannot preserve Fetch,
        // ready-body, signing, or application work.
        let retained_highest_store = highest_prepare_body.filter(|highest| {
            self.pending_stores.values().any(|pending| {
                (pending.task.manifest.round, pending.task.manifest.subject) == *highest
            })
        });
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
        for key in self
            .remote_proposal_replay
            .keys()
            .chain(self.authenticated_genesis_replay.keys())
            .chain(self.pending_durable_validate_admissions.keys())
            .chain(self.durable_validate_retry_seals.keys())
            .chain(self.published_lifecycle_store_retry_markers.keys())
            .chain(self.published_lifecycle_validate_retry_markers.keys())
            .copied()
        {
            if key_is_superseded(key.0, key.1) {
                superseded_keys.insert(key);
            }
        }
        if let Some(cleanup_only_high) =
            highest_prepare_body.filter(|highest| *highest != replacement)
        {
            // The durable high is retained only as bounded Store/Stored
            // cleanup lineage.  It can be newer than the first TC-selected
            // voting lock (and may have the same subject), so the ordinary
            // lock-supersession predicate intentionally does not select it.
            // Force that exact key through non-Store cleanup while the
            // stage-specific retention clauses below preserve only an
            // immutable in-flight Store or Store/Stored replay token.
            superseded_keys.insert(cleanup_only_high);
        }
        if self
            .pending_durable_validate_admissions
            .keys()
            .any(|key| superseded_keys.contains(key))
        {
            return Err(EffectExecutorError::Contract(
                "protected-lock cleanup cannot retire a parked lifecycle Validate admission"
                    .to_owned(),
            ));
        }
        if self.pending_live_wal_sign_admission.is_some() {
            return Err(EffectExecutorError::Contract(
                "protected-lock cleanup cannot overtake a parked live WAL Sign admission"
                    .to_owned(),
            ));
        }
        for pending in self.pending_applications.values() {
            let key = (
                pending.task.validated_receipt.durable().round(),
                pending.task.subject,
            );
            if key_is_superseded(key.0, key.1) {
                superseded_keys.insert(key);
            }
        }
        let signatures = self
            .pending_signatures
            .iter()
            .filter_map(|(id, pending)| {
                pending
                    .request
                    .body_round()
                    .zip(pending.request.subject())
                    .is_some_and(|key| key_is_superseded(key.0, key.1))
                    .then_some(*id)
            })
            .collect::<Vec<_>>();
        if self.pending_applications.values().any(|pending| {
            superseded_keys.contains(&(
                pending.task.validated_receipt.durable().round(),
                pending.task.subject,
            ))
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
                let key = (pending.task.manifest.round, pending.task.manifest.subject);
                superseded_keys.contains(&key) && Some(key) != retained_highest_store
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
        let retirement_trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_RETIRE,
            relation_exact: plan_exact_body_retirement_accounting(
                self.ready_body_bytes,
                retained_bytes,
                ready_bytes,
                self.pending_store_bytes,
                retired_store_bytes,
            ) == Some(accounting),
            protected_before: 0,
            protected_after: 0,
            owner_before: 0,
            owner_after: 0,
            owner_reused: false,
            ready_before: self.ready_body_bytes,
            retired_retained: retained_bytes,
            retired_ready: ready_bytes,
            ready_after: accounting.ready_after,
            store_before: self.pending_store_bytes,
            retired_store: retired_store_bytes,
            store_after: accounting.store_after,
            cursor_before: 0,
            completion_ready: false,
            progress_ready: false,
            normal_ready: false,
            selected: 0,
            cursor_after: 0,
        };
        let Some(checked_retirement) =
            check_production_body_capacity_retirement_effective_lock_transition(retirement_trace)
        else {
            return Err(EffectExecutorError::Contract(
                "body retirement did not refine exact effective-lock capacity".to_owned(),
            ));
        };
        let mut pipeline_owners = Vec::new();
        for key in &superseded_keys {
            if Some(*key) == retained_highest_store {
                continue;
            }
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
        let _authorized_retirement = checked_retirement.into_projection();
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
        for id in &signatures {
            services.cancel_consensus_sign(*id).map_err(service_error)?;
        }
        for plan in &fetches {
            services
                .cancel_body_fetch(&plan.pending.task)
                .map_err(service_error)?;
        }
        for (id, _) in &stores {
            services.cancel_body_store(*id).map_err(service_error)?;
        }
        for plan in fetches {
            self.commit_pending_fetch_retirement(plan)?;
        }
        for id in signatures {
            self.pending_signatures.remove(&id);
        }
        for (id, _) in stores {
            self.pending_stores.remove(&id);
            self.local_store_replay.remove(&id);
        }
        for ((round, subject), owner, _) in pipeline_owners {
            self.retire_local_proposal_ready_replay(owner.tag, round, subject);
        }
        self.ready_bodies
            .retain(|key, _| !superseded_keys.contains(key));
        self.body_pipeline_owners.retain(|key, _| {
            !superseded_keys.contains(key) || Some(*key) == retained_highest_store
        });
        self.remote_proposal_replay.retain(|key, stage| {
            !superseded_keys.contains(key)
                || (Some(*key) == highest_prepare_body
                    && matches!(
                        stage,
                        RemoteProposalReplayStageV1::Store { .. }
                            | RemoteProposalReplayStageV1::Stored { .. }
                    ))
        });
        self.authenticated_genesis_replay.retain(|key, stage| {
            !superseded_keys.contains(key)
                || (Some(*key) == highest_prepare_body
                    && matches!(
                        stage,
                        AuthenticatedGenesisReplayStageV1::Store { .. }
                            | AuthenticatedGenesisReplayStageV1::Stored { .. }
                    ))
        });
        // An active published Store marker is the executor half of a still
        // executable lifecycle-registry row. Lock reconciliation has no
        // authority to cancel that row, so only the atomic Store-to-Validate
        // handoff may remove its marker.
        self.durable_validate_retry_seals.retain(|key, seal| {
            seal.lifecycle_ordinal().is_some()
                || !superseded_keys.contains(key)
                || Some(*key) == highest_prepare_body
        });
        self.published_lifecycle_validate_retry_markers
            .retain(|key, marker| {
                marker.owns_live_lifecycle_row()
                    || !superseded_keys.contains(key)
                    || Some(*key) == highest_prepare_body
            });
        if retire_retained {
            self.retained_locked_body = None;
        }
        self.ready_body_bytes = accounting.ready_after;
        self.pending_store_bytes = accounting.store_after;
        self.protected_lock = Some(replacement);
        Ok(true)
    }
    /// Retain exact locked bytes under the round that originally installed the lock.
    ///
    /// The lifecycle tag may advance while the body origin does not. This cache
    /// may therefore satisfy only the exact protected `(round, subject)` fetch;
    /// it never remints a manifest, validation marker, or proposal in the
    /// current proposal view. The runner may separately use the immutable bytes
    /// to build a same-subject reproposal. If acquisition already started, the trusted local
    /// bytes finish that exact fetch immediately and retire its network owner.
    pub(crate) fn retain_locked_body_for_recovery<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        canonical_wire: Vec<u8>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        if tag.height() != self.context.height
            || round.context_id != self.context.id()
            || round.height != self.context.height
            || round.view > tag.view()
            || self.protected_lock != Some((round, subject))
        {
            return Err(EffectExecutorError::Contract(
                "retained locked body differs from its exact protected lock".to_owned(),
            ));
        }
        let ready = ReadyBody::derive(&self.context, round, subject, canonical_wire)
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        let key = (round, subject);
        let retention = self.plan_retained_locked_body(subject, Arc::clone(&ready.bytes))?;
        if let Some(existing) = self.ready_bodies.get(&key) {
            if existing.manifest != ready.manifest || existing.bytes != ready.bytes {
                return Err(EffectExecutorError::Contract(
                    "retained locked body conflicts with its staged protected bytes".to_owned(),
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
            if let Err(error) = self.commit_fetch_completion(plan) {
                let error = self.fail_closed_transport(runtime_enqueue_error(error), services);
                return Err(EffectExecutorError::Service(error.to_string()));
            }
            return self.publish_status(services);
        }
        let ready_plan =
            self.plan_ready_body_install_with_retention(key, ready, None, Some(&retention))?;
        self.commit_retained_locked_body(retention);
        self.commit_ready_body_install(ready_plan);
        self.publish_status(services)
    }
    /// Retain one reducer batch while optionally fencing its exact Apply until
    /// the synchronous runner has retired process-local Decision losers.
    fn consume_effects_with_runner_decision_cleanup<S: V2EffectServices>(
        &mut self,
        effects: Vec<AdapterEffect>,
        services: &mut S,
        pending_runner_decision_cleanup: Option<PendingRunnerDecisionCleanup>,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        if effects.len() > MAX_EFFECTS_PER_STEP {
            return Err(self.close(
                EffectExecutorError::Contract(format!(
                    "one adapter macro-step emitted {} effects above the adapter bound {MAX_EFFECTS_PER_STEP}",
                    effects.len()
                )),
                services,
            ));
        }
        if self.pending_runner_decision_cleanup.is_some() {
            return Err(self.close(
                EffectExecutorError::Contract(
                    "reducer effects overtook pending runner Decision cleanup".to_owned(),
                ),
                services,
            ));
        }
        if let Some(pending) = pending_runner_decision_cleanup {
            if !Self::new_decision_batch_has_only_exact_apply(
                &effects,
                pending.decision,
                Some(pending.owner_tag),
            ) {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "new Decision Apply handoff changed its exact retained suffix".to_owned(),
                    ),
                    services,
                ));
            }
        }
        let frontier = self
            .runtime
            .reconciliation_frontier()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        if let Err(error) = self.preflight_effect_batch_frontier(&effects, frontier) {
            return Err(self.close(error, services));
        }
        let ownership = match self.runtime.take_effect_ownership(&effects) {
            Ok(ownership) => ownership,
            Err(error) => {
                return Err(self.close(EffectExecutorError::Runtime(error), services));
            }
        };
        let local_proposal_replay_projections = self
            .plan_local_proposal_replay_consumptions(&effects, &ownership)
            .map_err(|error| self.close(error, services))?;
        let mut live_proposal_sign = self
            .runtime
            .take_live_proposal_intent_wal_sign(&effects)
            .map_err(|error| EffectExecutorError::Runtime(error.to_string()))
            .map_err(|error| self.close(error, services))?;
        if let Err(error) = self.retain_effect_batch_at_frontier(effects, ownership, frontier) {
            return Err(self.close(error, services));
        }
        let mut lifecycle_handoffs = 0usize;
        for projection in local_proposal_replay_projections {
            let ready = self
                .local_proposal_ready_replay
                .remove(&projection.command_identity)
                .expect("preflighted ProposalIntent replay authority remains installed");
            let replay = match ready.bind_proposal_intent(
                projection.command_identity,
                &projection.effect,
                &projection.ownership,
            ) {
                Ok(replay) => replay,
                Err(ready) => {
                    let Entry::Vacant(slot) = self
                        .local_proposal_ready_replay
                        .entry(projection.command_identity)
                    else {
                        unreachable!(
                            "the exact ready replay entry was removed immediately before restoration"
                        )
                    };
                    slot.insert(ready);
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "preflighted local ProposalIntent changed before retention".to_owned(),
                        ),
                        services,
                    ));
                }
            };
            if let Some(handoff) = live_proposal_sign.take() {
                if self.pending_live_wal_sign_admission.is_some() {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "one live WAL Sign admission overtook its predecessor".to_owned(),
                        ),
                        services,
                    ));
                }
                let pending = match handoff.join_local_proposal(replay) {
                    Ok(pending) => pending,
                    Err((_handoff, _replay)) => {
                        return Err(self.close(
                            EffectExecutorError::Contract(
                                "local ProposalIntent WAL owner changed before lifecycle handoff"
                                    .to_owned(),
                            ),
                            services,
                        ));
                    }
                };
                let retained = self
                    .retained_effect_batch
                    .as_mut()
                    .and_then(|batch| batch.effects.pop_front());
                if !retained.is_some_and(|owned| {
                    owned.effect == projection.effect && owned.ownership == projection.ownership
                }) || self
                    .retained_effect_batch
                    .as_ref()
                    .is_some_and(|batch| !batch.effects.is_empty())
                {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "live ProposalIntent handoff did not own the exact retained batch"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
                self.retained_effect_batch = None;
                self.pending_live_wal_sign_admission = Some(pending);
                lifecycle_handoffs = lifecycle_handoffs.saturating_add(1);
            } else {
                let duplicate = match self
                    .local_proposal_intent_replay
                    .entry(projection.command_identity)
                {
                    Entry::Vacant(slot) => {
                        slot.insert(replay);
                        false
                    }
                    Entry::Occupied(_) => true,
                };
                if duplicate {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "one local ProposalIntent acquired duplicate replay authority"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
            }
        }
        if live_proposal_sign.is_some() {
            return Err(self.close(
                EffectExecutorError::Contract(
                    "ProposalIntent WAL sidecar had no exact local replay companion".to_owned(),
                ),
                services,
            ));
        }
        self.pending_runner_decision_cleanup = pending_runner_decision_cleanup;
        if let Err(error) = self.commit_reconciliation_frontier(frontier, services) {
            return Err(self.close_after_transferring_runtime_terminals(error, services));
        }
        if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
            return Err(self.close(error, services));
        }
        let count = self
            .drain_retained_effect_batch(services, true)
            .map_err(|error| self.close_after_transferring_runtime_terminals(error, services))?;
        if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
            return Err(self.close(error, services));
        }
        Ok(count.saturating_add(lifecycle_handoffs))
    }
    fn new_decision_batch_has_only_exact_apply(
        effects: &[AdapterEffect],
        decision: DurableDecision,
        authoritative_tag: Option<EventTag>,
    ) -> bool {
        let mut apply_count = 0usize;
        for effect in effects {
            let AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            } = effect
            else {
                continue;
            };
            apply_count = apply_count.saturating_add(1);
            if apply_count > 1
                || Some(*tag) != authoritative_tag
                || *subject != decision.2
                || certificate.phase != wire::GlobalPhase::Commit
                || certificate.round != decision.0
                || certificate.proposal_round != decision.1
                || certificate.subject != decision.2
                || certificate.execution_commitment != decision.3
            {
                return false;
            }
        }
        true
    }
}
include!("v2_effects_runner_decision_cleanup_plan.rs");
#[cfg(test)]
include!("v2_effects_test_consumer_wrappers.rs");
impl<R: EffectRuntime> V2EffectExecutor<R> {
    fn plan_local_proposal_replay_consumptions(
        &self,
        effects: &[AdapterEffect],
        ownership: &[RuntimeEffectOwnership],
    ) -> Result<Vec<LocalProposalIntentProjection>, EffectExecutorError> {
        if effects.len() != ownership.len() {
            return Err(EffectExecutorError::Contract(
                "ProposalIntent replay preflight observed mismatched effect ownership".to_owned(),
            ));
        }
        let mut consumed = Vec::new();
        let mut matched_effects = BTreeSet::new();
        for (identity, replay) in &self.local_proposal_ready_replay {
            let mut matches =
                effects
                    .iter()
                    .zip(ownership)
                    .enumerate()
                    .filter(|(_, (effect, _))| {
                        replay.exactly_matches_proposal_intent_effect(*identity, effect)
                    });
            let Some((index, (effect, ownership))) = matches.next() else {
                continue;
            };
            if matches.next().is_some()
                || !matched_effects.insert(index)
                || !replay.exactly_matches_proposal_intent(*identity, effect, ownership)
            {
                return Err(EffectExecutorError::Contract(
                    "local ProposalIntent changed its exact command or causal owner".to_owned(),
                ));
            }
            if self.local_proposal_intent_replay.contains_key(identity) {
                return Err(EffectExecutorError::Contract(
                    "one local command already retained ProposalIntent replay authority".to_owned(),
                ));
            }
            consumed.push(LocalProposalIntentProjection {
                command_identity: *identity,
                effect: effect.clone(),
                ownership: ownership.clone(),
            });
        }
        for (identity, replay) in &self.local_proposal_intent_replay {
            if effects
                .iter()
                .any(|effect| replay.exactly_matches_proposal_intent_effect(*identity, effect))
            {
                return Err(EffectExecutorError::Contract(
                    "an emitted local ProposalIntent duplicated retained composite authority"
                        .to_owned(),
                ));
            }
        }
        Ok(consumed)
    }
    fn retire_local_proposal_ready_replay(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) {
        self.local_proposal_ready_replay.retain(|identity, replay| {
            !replay.exactly_matches_retirement(*identity, tag, round, subject)
        });
        self.local_proposal_intent_replay
            .retain(|identity, replay| {
                !replay.exactly_matches_retirement(*identity, tag, round, subject)
            });
    }
    /// Transfer the runtime's terminal sidecar only after the matching effect
    /// ownership and complete causal suffix have been retained. This keeps an
    /// empty-effect terminal from being inferred as durable and prevents a
    /// later scheduler turn from overtaking receiver-gate retirement.
    fn consume_leader_wire_runtime_terminals<S: V2EffectServices>(
        &mut self,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let terminals = self
            .runtime
            .take_leader_wire_runtime_terminals()
            .map_err(EffectExecutorError::Runtime)?;
        let mut first_error = None;
        for terminal in terminals {
            if let Err(error) = services.complete_leader_wire_runtime_terminal(terminal)
                && first_error.is_none()
            {
                first_error = Some(EffectExecutorError::Service(error.to_string()));
            }
        }
        first_error.map_or(Ok(()), Err)
    }
    /// Fail closed only after transferring every runtime terminal already
    /// emitted before a later synchronous executor operation failed.
    ///
    /// The runtime sidecar is independent of the failing adapter or service
    /// callback. Dropping it would leave a consumed packet's generic gate in
    /// Runtime and turn the required restart into a carrierless replay barrier.
    fn close_after_transferring_runtime_terminals<S: V2EffectServices>(
        &mut self,
        error: EffectExecutorError,
        services: &mut S,
    ) -> EffectExecutorError {
        match self.consume_leader_wire_runtime_terminals(services) {
            Ok(()) => self.close(error, services),
            Err(terminal_error) => self.close(
                EffectExecutorError::Service(format!(
                    "{error}; additionally failed to transfer leader-wire terminal: {terminal_error}"
                )),
                services,
            ),
        }
    }
    /// Install one complete adapter macro-step before dispatching any prefix.
    ///
    /// Rejecting a second batch while debt exists is deliberate: only the
    /// serialized runtime can establish causal order, and it is not stepped
    /// again until this suffix drains. The adapter proves that its flattened
    /// persistence continuations remain within the reducer-sized bound.
    fn retained_candidate_owners(
        &self,
        entering_view: Option<EventTag>,
    ) -> Result<BTreeMap<Hash, RuntimeEffectOwnership>, EffectExecutorError> {
        let mut owners = BTreeMap::<Hash, RuntimeEffectOwnership>::new();
        let mut insert = |ownership: &RuntimeEffectOwnership| {
            let identity = ownership.candidate_semantic_identity().ok_or_else(|| {
                EffectExecutorError::Contract(
                    "pending asynchronous work omitted its route-neutral candidate identity"
                        .to_owned(),
                )
            })?;
            match owners.get(&identity) {
                Some(existing) if existing != ownership => Err(EffectExecutorError::Contract(
                    "one semantic candidate lifecycle had conflicting exact owners".to_owned(),
                )),
                Some(_) => Ok(()),
                None => {
                    owners.insert(identity, ownership.clone());
                    Ok(())
                }
            }
        };
        for pending in self.pending_signatures.values() {
            // `EnterView` is ordered before every freshly reissued Sign in the
            // reducer batch. Its service callback cancels the old tagged task,
            // so that task is not an incumbent for the new-generation
            // candidate. Counting it here would coalesce away the replacement
            // and leave the reducer awaiting a signature no service owns.
            if entering_view.is_some_and(|tag| tag.strictly_advances(pending.tag)) {
                continue;
            }
            insert(&pending.ownership)?;
        }
        for pending in self.pending_fetches.values() {
            insert(pending.task.ownership())?;
        }
        for pending in self.pending_stores.values() {
            insert(pending.task.ownership())?;
        }
        for pending in self.pending_applications.values() {
            insert(&pending.ownership)?;
        }
        if let Some(finality) = &self.finality_completion {
            match &finality.ownership {
                FinalityCompletionOwner::Runtime(ownership) => insert(ownership)?,
                FinalityCompletionOwner::LifecycleDecisionApply(key)
                    if key.matches_height_context(&self.context) => {}
                FinalityCompletionOwner::LifecycleDecisionApply(_) => {
                    return Err(EffectExecutorError::Contract(
                        "lifecycle Decision Apply finality changed its height context".to_owned(),
                    ));
                }
            }
        }
        if let Some(batch) = &self.parked_effect_batch {
            for owned in &batch.effects {
                if entering_view
                    .is_some_and(|tag| Self::parked_effect_is_retired_by_view(owned, tag))
                {
                    continue;
                }
                if owned.ownership.candidate_semantic_identity().is_some() {
                    insert(&owned.ownership)?;
                }
            }
        }
        Ok(owners)
    }
    fn effect_execution_tag(effect: &AdapterEffect) -> Option<EventTag> {
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
    /// Return whether a not-yet-dispatched ordinary effect is superseded by
    /// an installed view before it can acquire service ownership.
    ///
    /// Diagnostics remain valid across views. A control broadcast has no
    /// concrete reducer tag, so its immutable causal root supplies the same
    /// check; the reducer's post-TC outbound catalog reproduces any control
    /// message which remains active in the installed view.
    fn parked_effect_is_retired_by_view(owned: &OwnedAdapterEffect, tag: EventTag) -> bool {
        match &owned.effect {
            AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => false,
            AdapterEffect::Broadcast(_) => {
                tag.strictly_advances(owned.ownership.owner().causal_origin().root_tag)
            }
            effect => Self::effect_execution_tag(effect)
                .is_some_and(|effect_tag| tag.strictly_advances(effect_tag)),
        }
    }
    fn entering_view_tag(
        effects: &[AdapterEffect],
    ) -> Result<Option<EventTag>, EffectExecutorError> {
        let mut entering = effects.iter().filter_map(|effect| match effect {
            AdapterEffect::EnterView { tag, .. } => Some(*tag),
            _ => None,
        });
        let tag = entering.next();
        if entering.next().is_some() {
            return Err(EffectExecutorError::Contract(
                "one adapter macro-step emitted more than one EnterView".to_owned(),
            ));
        }
        Ok(tag)
    }
    fn adapter_effect_body_key(
        effect: &AdapterEffect,
    ) -> Option<(wire::ConsensusRound, wire::BlockSubject)> {
        match effect {
            AdapterEffect::Sign { request, .. } => request.body_round().zip(request.subject()),
            AdapterEffect::FetchBody { round, subject, .. }
            | AdapterEffect::StoreBody { round, subject, .. }
            | AdapterEffect::ValidateBody { round, subject, .. } => Some((*round, *subject)),
            AdapterEffect::Apply {
                subject,
                certificate,
                ..
            } => Some((certificate.proposal_round, *subject)),
            AdapterEffect::Broadcast(_)
            | AdapterEffect::EnterView { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => None,
        }
    }
    /// Validate the cleanup-only high-water mark read from the same durable
    /// reducer frontier as the effect batch. This reference cannot select a
    /// lock or authorize body use; it only bounds which completed/in-flight
    /// Store lineage survives stale-view retirement.
    fn preflight_highest_prepare_frontier(
        &self,
        tag: Option<EventTag>,
        highest_prepare: Option<wire::QuorumCertificateRef>,
    ) -> Result<(), EffectExecutorError> {
        let Some(highest) = highest_prepare else {
            return Ok(());
        };
        let tag = tag.ok_or_else(|| {
            EffectExecutorError::Contract(
                "durable highest Prepare omitted its reducer incarnation".to_owned(),
            )
        })?;
        if tag.height() != self.context.height
            || highest.phase != wire::GlobalPhase::Prepare
            || highest.round != highest.proposal_round
            || highest.round.context_id != self.context.id()
            || highest.round.height != self.context.height
            || highest.round.view > tag.view()
        {
            return Err(EffectExecutorError::Contract(
                "durable highest Prepare is outside its frozen reducer frontier".to_owned(),
            ));
        }
        Ok(())
    }
    /// Reject a malformed reducer frontier before consuming its move-only
    /// lifecycle sidecar. A rejected view transition must leave that exact
    /// owner available to the fail-stop/restart boundary.
    fn preflight_effect_batch_frontier(
        &self,
        effects: &[AdapterEffect],
        frontier: RuntimeReconciliationFrontier,
    ) -> Result<Option<EventTag>, EffectExecutorError> {
        let entering_view = Self::entering_view_tag(effects)?;
        self.preflight_highest_prepare_frontier(frontier.tag, frontier.highest_prepare)?;
        if entering_view.is_some()
            && !matches!(effects.first(), Some(AdapterEffect::EnterView { .. }))
        {
            return Err(EffectExecutorError::Contract(
                "EnterView must be the first effect in its reducer macro-step".to_owned(),
            ));
        }
        match (self.reconciled_tag, frontier.tag) {
            (Some(current), Some(next)) if current == next => {
                if entering_view.is_some() {
                    return Err(EffectExecutorError::Contract(
                        "EnterView did not advance the reconciled reducer incarnation".to_owned(),
                    ));
                }
            }
            (Some(current), Some(next)) if next.strictly_advances(current) => {
                if entering_view != Some(next) {
                    return Err(EffectExecutorError::Contract(
                        "an advancing reducer frontier omitted its leading EnterView".to_owned(),
                    ));
                }
            }
            (None, None) => {
                if entering_view.is_some() {
                    return Err(EffectExecutorError::Contract(
                        "EnterView has no authoritative reducer incarnation".to_owned(),
                    ));
                }
            }
            (None, Some(next)) => {
                if entering_view != Some(next) {
                    return Err(EffectExecutorError::Contract(
                        "the first authoritative reducer incarnation omitted EnterView".to_owned(),
                    ));
                }
            }
            (Some(_), None) | (Some(_), Some(_)) => {
                return Err(EffectExecutorError::Contract(
                    "the reducer reconciliation frontier regressed or changed incomparably"
                        .to_owned(),
                ));
            }
        }
        let mut protected = effects.iter().filter_map(|effect| match effect {
            AdapterEffect::EnterView {
                tag,
                protected_lock,
                ..
            } => Some((*tag, protected_lock_body(protected_lock.as_ref()))),
            _ => None,
        });
        if let Some((tag, protected_body)) = protected.next() {
            if protected.next().is_some() {
                return Err(EffectExecutorError::Contract(
                    "one adapter macro-step emitted more than one EnterView".to_owned(),
                ));
            }
            if frontier.tag != Some(tag)
                || (frontier.lock_is_authoritative
                    && frontier.decision.is_none()
                    && frontier.locked_body != protected_body)
            {
                return Err(EffectExecutorError::Contract(
                    "EnterView disagreed with the reducer reconciliation frontier".to_owned(),
                ));
            }
            if frontier
                .highest_prepare
                .is_some_and(|highest| highest.round.view >= tag.view())
            {
                return Err(EffectExecutorError::Contract(
                    "EnterView cleanup frontier retained a non-historical highest Prepare"
                        .to_owned(),
                ));
            }
        }
        Ok(entering_view)
    }
    /// Retire only not-yet-dispatched ordinary work which the reducer's new
    /// durable frontier makes impossible. This pure in-memory phase runs before
    /// candidate coalescing; service/runtime cancellation waits until the full
    /// replacement batch is retained.
    fn prepare_parked_effects_for_frontier(
        &mut self,
        effects: &[AdapterEffect],
        frontier: RuntimeReconciliationFrontier,
    ) -> Result<Option<EventTag>, EffectExecutorError> {
        let entering_view = self.preflight_effect_batch_frontier(effects, frontier)?;
        let lock_transition = frontier
            .decision
            .is_none()
            .then_some(frontier.locked_body)
            .flatten()
            .filter(|replacement| Some(*replacement) != self.protected_lock)
            .map(|replacement| (self.protected_lock, replacement));
        if let Some(batch) = self.parked_effect_batch.as_mut() {
            batch.effects.retain(|owned| {
                if entering_view
                    .is_some_and(|tag| Self::parked_effect_is_retired_by_view(owned, tag))
                {
                    return false;
                }
                if let Some(decision) = frontier.decision
                    && !Self::effect_survives_decision(&owned.effect, decision)
                {
                    return false;
                }
                if let Some((superseded, replacement)) = lock_transition
                    && Self::adapter_effect_body_key(&owned.effect).is_some_and(|key| {
                        protected_lock_retires_body_key(superseded, replacement, key)
                    })
                {
                    return false;
                }
                true
            });
        }
        if self
            .parked_effect_batch
            .as_ref()
            .is_some_and(|batch| batch.effects.is_empty())
        {
            self.parked_effect_batch = None;
        }
        Ok(entering_view)
    }
    /// Commit the service-owning half of one already-retained reducer
    /// frontier before any fresh adapter effect can dispatch.
    fn commit_reconciliation_frontier<S: V2EffectServices>(
        &mut self,
        frontier: RuntimeReconciliationFrontier,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        if let Some(decision) = frontier.decision {
            return self.reconcile_decision_work(decision, false, services);
        }
        if !frontier.lock_is_authoritative {
            return Ok(());
        }
        let Some(replacement) = frontier.locked_body else {
            if self.protected_lock.is_some() || self.retained_locked_body.is_some() {
                return Err(EffectExecutorError::Contract(
                    "the reducer reconciliation frontier cleared a durable PrepareQC lock"
                        .to_owned(),
                ));
            }
            return Ok(());
        };
        let tag = frontier.tag.ok_or_else(|| {
            EffectExecutorError::Contract(
                "durable lock reconciliation omitted its reducer tag".to_owned(),
            )
        })?;
        if self.protected_lock == Some(replacement) {
            return Ok(());
        }
        self.preflight_observed_protected_lock(tag, replacement)?;
        self.reconcile_protected_lock(
            tag,
            Some(replacement),
            highest_prepare_body(frontier.highest_prepare),
            services,
        )?;
        Ok(())
    }
    #[cfg(test)]
    fn retain_effect_batch(
        &mut self,
        effects: Vec<AdapterEffect>,
        ownership: Vec<RuntimeEffectOwnership>,
    ) -> Result<(), EffectExecutorError> {
        let frontier = self
            .runtime
            .reconciliation_frontier()
            .map_err(EffectExecutorError::Runtime)?;
        self.retain_effect_batch_at_frontier(effects, ownership, frontier)
    }
    fn retain_effect_batch_at_frontier(
        &mut self,
        effects: Vec<AdapterEffect>,
        mut ownership: Vec<RuntimeEffectOwnership>,
        frontier: RuntimeReconciliationFrontier,
    ) -> Result<(), EffectExecutorError> {
        #[cfg(test)]
        let mut recovered_validate_retry_trace_root = None;
        #[cfg(test)]
        let mut recovered_validate_retry_trace_ordinal = None;
        if self.retained_effect_batch.is_some() {
            return Err(EffectExecutorError::Contract(
                "a second adapter macro-step overtook retained causal dispatch debt".to_owned(),
            ));
        }
        if effects.len() > MAX_EFFECTS_PER_STEP {
            return Err(EffectExecutorError::Contract(format!(
                "one adapter macro-step emitted {} effects above the adapter bound {MAX_EFFECTS_PER_STEP}",
                effects.len()
            )));
        }
        if effects.len() != ownership.len() {
            return Err(EffectExecutorError::Contract(
                "one adapter macro-step had mismatched lifecycle ownership".to_owned(),
            ));
        }
        let entering_view = self.prepare_parked_effects_for_frontier(&effects, frontier)?;
        let entering_protected_body = match effects.first() {
            Some(AdapterEffect::EnterView { protected_lock, .. }) => {
                protected_lock_body(protected_lock.as_ref())
            }
            _ => None,
        };
        if effects.is_empty() {
            return Ok(());
        }
        let effect_count = u8::try_from(effects.len()).map_err(|_| {
            EffectExecutorError::Contract(
                "one adapter macro-step effect count was not representable".to_owned(),
            )
        })?;
        let candidate_count_usize = effects
            .iter()
            .filter(|effect| {
                production_adapter_effect_candidate_semantic_identity(effect).is_some()
            })
            .count();
        if candidate_count_usize > super::v2_core::MAX_CAUSAL_SUCCESSORS_PER_COMMAND {
            return Err(EffectExecutorError::Contract(format!(
                "one adapter macro-step emitted {candidate_count_usize} causal candidates above the abstract bound {}",
                super::v2_core::MAX_CAUSAL_SUCCESSORS_PER_COMMAND
            )));
        }
        let candidate_count = u8::try_from(candidate_count_usize).map_err(|_| {
            EffectExecutorError::Contract(
                "one adapter macro-step candidate count was not representable".to_owned(),
            )
        })?;
        let mut retained_candidate_owners = self.retained_candidate_owners(entering_view)?;
        // A body acquisition has one physical owner even as authenticated
        // consensus evidence refines it from an ordinary Proposal fetch to a
        // Prepare- or Commit-certified fetch. The route-neutral candidate
        // identity deliberately includes phase and execution commitment, so
        // retain a separate, strictly narrower lineage index for that one
        // monotonic authority transition.
        let mut retained_fetch_lineages = BTreeMap::<
            (EventTag, wire::ConsensusRound, wire::BlockSubject),
            RuntimeEffectOwnership,
        >::new();
        for pending in self.pending_fetches.values() {
            // EnterView is dispatched before the rest of this macro-step and
            // rebinds the exact protected fetch to its new consumer tag. Use
            // that post-prefix tag while coalescing the following authority
            // refinement, otherwise the same physical fetch is missed solely
            // because its still-pending task carries the pre-EnterView tag.
            let effective_tag = entering_view
                .filter(|tag| {
                    tag.strictly_advances(pending.task.tag)
                        && entering_protected_body
                            == Some((pending.task.round, pending.task.subject))
                })
                .unwrap_or(pending.task.tag);
            let key = (effective_tag, pending.task.round, pending.task.subject);
            let effective_ownership = if effective_tag == pending.task.tag {
                pending.task.ownership().clone()
            } else {
                pending
                    .task
                    .rebind_consumer(effective_tag)
                    .ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "protected body-fetch ownership could not rebind before lineage retention"
                                .to_owned(),
                        )
                    })?
                    .ownership()
                    .clone()
            };
            let candidate_identity = effective_ownership
                .candidate_semantic_identity()
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "protected body-fetch ownership omitted its candidate identity".to_owned(),
                    )
                })?;
            let candidate_owner = retained_candidate_owners
                .get_mut(&candidate_identity)
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "protected body-fetch lineage omitted its retained candidate owner"
                            .to_owned(),
                    )
                })?;
            if *candidate_owner != effective_ownership {
                return Err(EffectExecutorError::Contract(
                    "protected body-fetch rebind changed its immutable lifecycle owner".to_owned(),
                ));
            }
            *candidate_owner = effective_ownership.clone();
            if let Some(existing) = retained_fetch_lineages.insert(key, effective_ownership.clone())
                && existing != effective_ownership
            {
                return Err(EffectExecutorError::Contract(
                    "one body-fetch lineage had conflicting exact owners".to_owned(),
                ));
            }
        }
        if let Some(batch) = &self.parked_effect_batch {
            for owned in &batch.effects {
                let AdapterEffect::FetchBody {
                    tag,
                    round,
                    subject,
                    ..
                } = &owned.effect
                else {
                    continue;
                };
                let key = (*tag, *round, *subject);
                if let Some(existing) = retained_fetch_lineages.insert(key, owned.ownership.clone())
                    && existing != owned.ownership
                {
                    return Err(EffectExecutorError::Contract(
                        "one parked body-fetch lineage had conflicting exact owners".to_owned(),
                    ));
                }
            }
        }
        // StoreBody is one physical task per exact body, even when a later
        // reducer carrier has acquired stronger Prepare/Commit authority.
        // Keep a stage-separated lineage index beside the ordinary candidate
        // index. A parked Validate has not yet crossed lifecycle admission, so
        // it is indexed from the parked batch below.
        let mut retained_store_lineages =
            BTreeMap::<(wire::ConsensusRound, wire::BlockSubject), RuntimeEffectOwnership>::new();
        for pending in self.pending_stores.values() {
            let key = (pending.task.manifest.round, pending.task.manifest.subject);
            if let Some(existing) =
                retained_store_lineages.insert(key, pending.task.ownership().clone())
                && existing != *pending.task.ownership()
            {
                return Err(EffectExecutorError::Contract(
                    "one body-store lineage had conflicting exact owners".to_owned(),
                ));
            }
        }
        let mut retained_validation_lineages =
            BTreeMap::<(wire::ConsensusRound, wire::BlockSubject), RuntimeEffectOwnership>::new();
        // A protected Proposal can finish Store before EnterView or Decision
        // emits its Prepare/Commit Validate carrier. Retain the Store's exact
        // causal root as the incumbent Validate lineage; the normal authority
        // adoption below may strengthen only its candidate statement before
        // the move-only replay token is consumed.
        for effect in &effects {
            let AdapterEffect::ValidateBody { round, subject, .. } = effect else {
                continue;
            };
            let key = (*round, *subject);
            let Some(incumbent) = self.stored_replay_incumbent_validate_ownership(key, effect)?
            else {
                continue;
            };
            if let Some(existing) = retained_validation_lineages.insert(key, incumbent.clone())
                && existing != incumbent
            {
                return Err(EffectExecutorError::Contract(
                    "one stored Proposal body had conflicting Validate incumbents".to_owned(),
                ));
            }
        }
        if let Some(batch) = &self.parked_effect_batch {
            for owned in &batch.effects {
                let (lineages, key) = match &owned.effect {
                    AdapterEffect::StoreBody { round, subject, .. } => {
                        (&mut retained_store_lineages, (*round, *subject))
                    }
                    AdapterEffect::ValidateBody { round, subject, .. } => {
                        (&mut retained_validation_lineages, (*round, *subject))
                    }
                    _ => continue,
                };
                if let Some(existing) = lineages.insert(key, owned.ownership.clone())
                    && existing != owned.ownership
                {
                    return Err(EffectExecutorError::Contract(
                        "one parked physical body stage had conflicting exact owners".to_owned(),
                    ));
                }
            }
        }
        let mut retain_effect = Vec::with_capacity(effects.len());
        let mut retire_parked_fetch_lineages = BTreeSet::new();
        let mut retire_parked_body_stage_lineages = BTreeSet::new();
        let mut runtime_terminal_commits = Vec::new();
        let mut retained_validate_retry_seals = self.durable_validate_retry_seals.clone();
        let mut retained_published_store_retry_markers =
            self.published_lifecycle_store_retry_markers.clone();
        let mut retained_published_validate_retry_markers =
            self.published_lifecycle_validate_retry_markers.clone();
        let mut candidate_position = 0u8;
        for (index, (effect, evidence)) in effects.iter().zip(&mut ownership).enumerate() {
            let mut stored_replay_adopted = false;
            if let AdapterEffect::StoreBody { round, subject, .. } = effect
                && let Some(adopted) = self.stored_replay_incumbent_store_ownership(
                    (*round, *subject),
                    effect,
                    evidence,
                )?
            {
                // Runtime terminal planning precedes StoreBody dispatch. A
                // durable replay or its inert post-Validate Store seal is
                // the executor-side proof which may bind a later carrier to
                // that already-completed physical Store. Adopt it before the
                // queued BodyStored owner is compared; the Store handler
                // rechecks the same projection.
                *evidence = adopted;
                stored_replay_adopted = true;
            }
            let preterminal_body_stage_incumbent = match effect {
                AdapterEffect::StoreBody { round, subject, .. } => {
                    retained_store_lineages.get(&(*round, *subject))
                }
                AdapterEffect::ValidateBody { round, subject, .. } => {
                    retained_validation_lineages.get(&(*round, *subject))
                }
                _ => None,
            };
            if let Some(incumbent) = preterminal_body_stage_incumbent {
                if stored_replay_adopted && incumbent.owner() != evidence.owner() {
                    return Err(EffectExecutorError::Contract(
                        "stored replay and executor body-stage lineage retained different physical owners"
                            .to_owned(),
                    ));
                }
                if incumbent != &*evidence {
                    // A pending/parked Store or Stored-replay/parked Validate
                    // is already the sole executor-held physical stage. Adopt
                    // it before the runtime compares a queued terminal, while
                    // the typed lattice still rejects another body or stage.
                    *evidence = incumbent
                        .adopt_incumbent_body_stage_for_retry_or_authority(evidence, effect)
                        .map_err(EffectExecutorError::Contract)?;
                }
            }
            let candidate = production_adapter_effect_candidate_semantic_identity(effect);
            if candidate.is_some() {
                candidate_position = candidate_position.checked_add(1).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "one adapter macro-step candidate position overflowed".to_owned(),
                    )
                })?;
            }
            let effect_position = u8::try_from(index + 1).map_err(|_| {
                EffectExecutorError::Contract(
                    "one adapter macro-step effect position was not representable".to_owned(),
                )
            })?;
            if let AdapterEffect::StoreBody { round, subject, .. } = effect
                && let Some(marker) = retained_published_store_retry_markers
                    .get(&(*round, *subject))
                    .cloned()
            {
                // The direct lifecycle transaction already published the sole
                // executable Store row. Its ordinal-free marker stutters a
                // same-body retransmission before the runtime compares a
                // queued BodyStored terminal under the retry's fresh owner,
                // retaining the strongest authority in a comparison-only
                // overlay while the published fingerprint remains immutable.
                if stored_replay_adopted || preterminal_body_stage_incumbent.is_some() {
                    return Err(EffectExecutorError::Contract(
                        "published lifecycle Store retry overlapped another physical lineage"
                            .to_owned(),
                    ));
                }
                let key = (*round, *subject);
                let receipt = self.durable_bodies.get(&key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "published lifecycle Store retry lost its durable receipt".to_owned(),
                    )
                })?;
                let projected = marker
                    .project_active_store_retry(receipt, effect, evidence)
                    .map_err(EffectExecutorError::Contract)?;
                let identity = evidence.candidate_semantic_identity().ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "published lifecycle Store retry omitted its candidate identity".to_owned(),
                    )
                })?;
                if retained_candidate_owners
                    .get(&identity)
                    .is_some_and(|existing| existing != &*evidence)
                {
                    return Err(EffectExecutorError::Contract(
                        "published lifecycle Store retry disagreed with an exact incumbent owner"
                            .to_owned(),
                    ));
                }
                let admission =
                    production_adapter_effect_candidate_admission_disposition(effect, 1, 1)
                        .map_err(EffectExecutorError::Contract)?;
                if admission != RuntimeCandidateAdmissionDisposition::CoalescedRetry {
                    return Err(EffectExecutorError::Contract(
                        "published lifecycle Store retry did not classify as an owner stutter"
                            .to_owned(),
                    ));
                }
                let projection = production_adapter_effect_candidate_trace_projection(
                    effect,
                    evidence,
                    effect_position,
                    effect_count,
                    candidate.as_ref().map_or(0, |_| candidate_position),
                    candidate_count,
                    1,
                    1,
                    true,
                )
                .map_err(EffectExecutorError::Contract)?;
                let _authorized_store_retry = check_production_effect_to_candidate_transition(
                    projection,
                )
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "published lifecycle Store retry failed candidate refinement".to_owned(),
                    )
                })?;
                retained_published_store_retry_markers.insert((*round, *subject), projected);
                retain_effect.push(false);
                continue;
            }
            if let AdapterEffect::StoreBody { round, subject, .. } = effect
                && let Some(marker) =
                    retained_published_validate_retry_markers.get(&(*round, *subject))
            {
                // The direct lifecycle transaction already advanced this exact
                // durable Store into its Validate row. Its opaque ordinal-free
                // predecessor seal may stutter a compatible later Store before
                // the runtime compares any queued BodyStored terminal under the
                // retry's fresh owner. Live replay/seal or executor Store work
                // beside this published marker would instead be two lineages.
                if stored_replay_adopted || preterminal_body_stage_incumbent.is_some() {
                    return Err(EffectExecutorError::Contract(
                        "published lifecycle Store retry overlapped another physical lineage"
                            .to_owned(),
                    ));
                }
                let key = (*round, *subject);
                let receipt = self.durable_bodies.get(&key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "published lifecycle Store retry lost its durable receipt".to_owned(),
                    )
                })?;
                marker
                    .project_store_retry(receipt, effect, evidence)
                    .map_err(EffectExecutorError::Contract)?;
                let identity = evidence.candidate_semantic_identity().ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "published lifecycle Store retry omitted its candidate identity".to_owned(),
                    )
                })?;
                if retained_candidate_owners
                    .get(&identity)
                    .is_some_and(|existing| existing != &*evidence)
                {
                    return Err(EffectExecutorError::Contract(
                        "published lifecycle Store retry disagreed with an exact incumbent owner"
                            .to_owned(),
                    ));
                }
                let admission =
                    production_adapter_effect_candidate_admission_disposition(effect, 1, 1)
                        .map_err(EffectExecutorError::Contract)?;
                if admission != RuntimeCandidateAdmissionDisposition::CoalescedRetry {
                    return Err(EffectExecutorError::Contract(
                        "published lifecycle Store retry did not classify as an owner stutter"
                            .to_owned(),
                    ));
                }
                let projection = production_adapter_effect_candidate_trace_projection(
                    effect,
                    evidence,
                    effect_position,
                    effect_count,
                    candidate.as_ref().map_or(0, |_| candidate_position),
                    candidate_count,
                    1,
                    1,
                    true,
                )
                .map_err(EffectExecutorError::Contract)?;
                let _authorized_store_retry = check_production_effect_to_candidate_transition(
                    projection,
                )
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "published lifecycle Store retry failed candidate refinement".to_owned(),
                    )
                })?;
                retain_effect.push(false);
                continue;
            }
            if let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = effect
                && let Some(marker) = retained_published_validate_retry_markers
                    .get(&(*round, *subject))
                    .cloned()
            {
                // The direct lifecycle Validate remains the sole physical
                // operation while its row is live or a same/stale retry merely
                // rediscovers its terminal marker. One closed exception exists:
                // an ordinal-free older marker plus the exact cached successful
                // receipt may redispatch a strictly newer Commit refinement so
                // normal lifecycle admission can mint the missing Apply child.
                let projected = marker
                    .project_retry(effect, evidence)
                    .map_err(EffectExecutorError::Contract)?;
                let key = (*round, *subject);
                let readmit_protected_decision = frontier.decision.is_some_and(|decision| {
                    self.runtime
                        .has_exact_pending_live_decision_apply(*tag, decision)
                        && self.validated_bodies.get(&key).is_some_and(|validated| {
                            marker
                                .is_unbound_exact_decision_upgrade(&projected, decision, validated)
                        })
                });
                let identity = evidence.candidate_semantic_identity().ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "published lifecycle Validate retry omitted its candidate identity"
                            .to_owned(),
                    )
                })?;
                if retained_candidate_owners
                    .get(&identity)
                    .is_some_and(|existing| existing != &*evidence)
                {
                    return Err(EffectExecutorError::Contract(
                        "published lifecycle Validate retry disagreed with an exact incumbent owner"
                            .to_owned(),
                    ));
                }
                let admission =
                    production_adapter_effect_candidate_admission_disposition(effect, 1, 1)
                        .map_err(EffectExecutorError::Contract)?;
                if admission != RuntimeCandidateAdmissionDisposition::CoalescedRetry {
                    return Err(EffectExecutorError::Contract(
                        "published lifecycle Validate retry did not classify as an owner stutter"
                            .to_owned(),
                    ));
                }
                let projection = production_adapter_effect_candidate_trace_projection(
                    effect,
                    evidence,
                    effect_position,
                    effect_count,
                    candidate.as_ref().map_or(0, |_| candidate_position),
                    candidate_count,
                    1,
                    1,
                    true,
                )
                .map_err(EffectExecutorError::Contract)?;
                let _authorized_validate_retry = check_production_effect_to_candidate_transition(
                    projection,
                )
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "published lifecycle Validate retry failed candidate refinement".to_owned(),
                    )
                })?;
                retained_published_validate_retry_markers.insert(key, projected);
                retain_effect.push(readmit_protected_decision);
                continue;
            }
            if let AdapterEffect::ValidateBody { round, subject, .. } = effect
                && let Some(seal) = retained_validate_retry_seals
                    .get(&(*round, *subject))
                    .cloned()
            {
                let projected = seal
                    .project_retry(effect, evidence)
                    .map_err(EffectExecutorError::Contract)?;
                let key = (*round, *subject);
                let readmit_protected_prepare = frontier.decision.is_none()
                    && frontier.lock_is_authoritative
                    && frontier.locked_body == Some(key)
                    && seal.is_unbound_live_ordinary_to_prepare_upgrade(&projected);
                if readmit_protected_prepare {
                    // The old row is terminal and cannot emit the newer
                    // view's ValidationCompleted callback. Retire only this
                    // volatile tombstone; the current reducer owner falls
                    // through to the ordinary protected-lock reseed below.
                    let removed = retained_validate_retry_seals.remove(&key);
                    debug_assert_eq!(removed, Some(seal));
                } else {
                    // A live row, cold owner, or same/stale/Commit retry still
                    // owns the sole physical Validate lifecycle and therefore
                    // coalesces without redispatch.
                    #[cfg(test)]
                    {
                        recovered_validate_retry_trace_root =
                            Some(evidence.owner().causal_origin().lifecycle_key);
                        recovered_validate_retry_trace_ordinal =
                            Some(evidence.owner().lifecycle_ordinal());
                    }
                    let identity = projected
                        .ownership
                        .candidate_semantic_identity()
                        .ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "durable Validate retry seal omitted its candidate identity"
                                    .to_owned(),
                            )
                        })?;
                    if retained_candidate_owners
                        .get(&identity)
                        .is_some_and(|existing| existing != &projected.ownership)
                    {
                        return Err(EffectExecutorError::Contract(
                            "durable Validate retry disagreed with an exact incumbent owner"
                                .to_owned(),
                        ));
                    }
                    let admission =
                        production_adapter_effect_candidate_admission_disposition(effect, 1, 1)
                            .map_err(EffectExecutorError::Contract)?;
                    if admission != RuntimeCandidateAdmissionDisposition::CoalescedRetry {
                        return Err(EffectExecutorError::Contract(
                            "durable Validate retry did not classify as an exact owner stutter"
                                .to_owned(),
                        ));
                    }
                    let projection = production_adapter_effect_candidate_trace_projection(
                        effect,
                        &projected.ownership,
                        effect_position,
                        effect_count,
                        candidate.as_ref().map_or(0, |_| candidate_position),
                        candidate_count,
                        1,
                        1,
                        true,
                    )
                    .map_err(EffectExecutorError::Contract)?;
                    let _authorized_validate_retry =
                        check_production_effect_to_candidate_transition(projection).ok_or_else(
                            || {
                                EffectExecutorError::Contract(
                                    "durable Validate retry failed its incumbent-owner refinement"
                                        .to_owned(),
                                )
                            },
                        )?;
                    *evidence = projected.ownership.clone();
                    retained_candidate_owners.insert(identity, projected.ownership.clone());
                    retained_validate_retry_seals.insert((*round, *subject), projected.seal);
                    retain_effect.push(false);
                    continue;
                }
            }
            let mut candidate_semantic_identity = evidence.candidate_semantic_identity();
            let mut exact_incumbent = candidate_semantic_identity
                .as_ref()
                .and_then(|identity| retained_candidate_owners.get(identity))
                .cloned();
            if let Some(incumbent) = exact_incumbent.as_ref()
                && incumbent != &*evidence
            {
                *evidence = incumbent
                    .adopt_incumbent_candidate_for_semantic_retry(evidence, effect)
                    .map_err(EffectExecutorError::Contract)?;
            }
            let runtime_terminal_ownership = self
                .runtime
                .plan_body_pipeline_candidate_terminal(effect, evidence)
                .map_err(EffectExecutorError::Runtime)?;
            let runtime_terminal_incumbent = runtime_terminal_ownership.is_some();
            if runtime_terminal_incumbent && exact_incumbent.is_some() {
                return Err(EffectExecutorError::Contract(
                    "one body candidate was owned by both executor work and a runtime terminal"
                        .to_owned(),
                ));
            }
            if let Some(adopted) = runtime_terminal_ownership {
                let adopted_identity = adopted.candidate_semantic_identity().ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "one runtime body terminal omitted its candidate identity".to_owned(),
                    )
                })?;
                if retained_candidate_owners.contains_key(&adopted_identity) {
                    return Err(EffectExecutorError::Contract(
                        "one body candidate was owned by both executor work and a runtime terminal"
                            .to_owned(),
                    ));
                }
                *evidence = adopted;
                candidate_semantic_identity = Some(adopted_identity);
                exact_incumbent = None;
            }
            let fetch_key = match effect {
                AdapterEffect::FetchBody {
                    tag,
                    round,
                    subject,
                    ..
                } => Some((*tag, *round, *subject)),
                _ => None,
            };
            let fetch_lineage_incumbent = fetch_key
                .as_ref()
                .and_then(|key| retained_fetch_lineages.get(key))
                .cloned();
            if let (Some(exact), Some(lineage)) = (&exact_incumbent, &fetch_lineage_incumbent)
                && exact != lineage
            {
                return Err(EffectExecutorError::Contract(
                    "one body-fetch candidate disagreed with its physical lineage owner".to_owned(),
                ));
            }
            let lineage_only_incumbent = exact_incumbent
                .is_none()
                .then_some(fetch_lineage_incumbent)
                .flatten();
            let body_stage_incumbent = match effect {
                AdapterEffect::StoreBody { round, subject, .. } => {
                    retained_store_lineages.get(&(*round, *subject)).cloned()
                }
                AdapterEffect::ValidateBody { round, subject, .. } => retained_validation_lineages
                    .get(&(*round, *subject))
                    .cloned(),
                _ => None,
            };
            let body_stage_key = match effect {
                AdapterEffect::StoreBody { round, subject, .. }
                | AdapterEffect::ValidateBody { round, subject, .. } => {
                    let (kind, _) = candidate.as_ref().ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "one physical body stage omitted its candidate identity".to_owned(),
                        )
                    })?;
                    Some((*kind, *round, *subject))
                }
                _ => None,
            };
            if let (Some(exact), Some(lineage)) = (&exact_incumbent, &body_stage_incumbent)
                && exact != lineage
            {
                return Err(EffectExecutorError::Contract(
                    "one physical body-stage candidate disagreed with its lineage owner".to_owned(),
                ));
            }
            let body_stage_only_incumbent = exact_incumbent
                .is_none()
                .then_some(body_stage_incumbent)
                .flatten();
            let replacing_body_stage_lineage = body_stage_only_incumbent
                .is_some()
                .then_some(body_stage_key)
                .flatten();
            let lifecycle_validate_incumbent = match effect {
                AdapterEffect::ValidateBody { round, subject, .. } => self
                    .pending_durable_validate_admissions
                    .get(&(*round, *subject))
                    .map(|pending| {
                        if pending.exactly_matches_retry(effect, evidence) {
                            Ok(true)
                        } else {
                            Err(EffectExecutorError::Contract(
                                "pending lifecycle Validate disagreed with its duplicate runtime owner"
                                    .to_owned(),
                            ))
                        }
                    })
                    .transpose()?
                    .unwrap_or(false),
                _ => false,
            };
            let candidate_owner_count_before =
                candidate_semantic_identity.as_ref().map_or(0, |identity| {
                    u8::from(
                        retained_candidate_owners.contains_key(identity)
                            || runtime_terminal_incumbent
                            || lifecycle_validate_incumbent,
                    )
                });
            let candidate_owner_count_after = u8::from(candidate.is_some());
            let mut admission = production_adapter_effect_candidate_admission_disposition(
                effect,
                candidate_owner_count_before,
                candidate_owner_count_after,
            )
            .map_err(EffectExecutorError::Contract)?;
            let projection = production_adapter_effect_candidate_trace_projection(
                effect,
                evidence,
                effect_position,
                effect_count,
                candidate.as_ref().map_or(0, |_| candidate_position),
                candidate_count,
                candidate_owner_count_before,
                candidate_owner_count_after,
                true,
            )
            .map_err(EffectExecutorError::Contract)?;
            let checked =
                check_production_effect_to_candidate_transition(projection).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "one adapter effect failed its exact candidate-ownership refinement"
                            .to_owned(),
                    )
                })?;
            let _authorized_effect_candidate = checked.into_projection();
            let mut fetch_authority_relation = None;
            if let Some(incumbent) = lineage_only_incumbent {
                let (adopted, relation) = incumbent
                    .adopt_incumbent_fetch_for_retry_or_authority(evidence, effect)
                    .map_err(EffectExecutorError::Contract)?;
                admission = production_adapter_effect_candidate_admission_disposition(
                    effect,
                    1,
                    candidate_owner_count_after,
                )
                .map_err(EffectExecutorError::Contract)?;
                let adopted_projection = production_adapter_effect_candidate_trace_projection(
                    effect,
                    &adopted,
                    effect_position,
                    effect_count,
                    candidate.as_ref().map_or(0, |_| candidate_position),
                    candidate_count,
                    1,
                    candidate_owner_count_after,
                    true,
                )
                .map_err(EffectExecutorError::Contract)?;
                let _authorized_fetch_refinement =
                    check_production_effect_to_candidate_transition(adopted_projection)
                        .ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "coalesced body-fetch authority refinement failed its incumbent-owner refinement"
                                    .to_owned(),
                            )
                        })?;
                *evidence = adopted;
                fetch_authority_relation = Some(relation);
                if let Some(identity) = candidate_semantic_identity {
                    retained_candidate_owners.insert(identity, evidence.clone());
                }
            }
            if let Some(incumbent) = body_stage_only_incumbent {
                let adopted = incumbent
                    .adopt_incumbent_body_stage_for_retry_or_authority(evidence, effect)
                    .map_err(EffectExecutorError::Contract)?;
                admission = production_adapter_effect_candidate_admission_disposition(
                    effect,
                    1,
                    candidate_owner_count_after,
                )
                .map_err(EffectExecutorError::Contract)?;
                let adopted_projection = production_adapter_effect_candidate_trace_projection(
                    effect,
                    &adopted,
                    effect_position,
                    effect_count,
                    candidate.as_ref().map_or(0, |_| candidate_position),
                    candidate_count,
                    1,
                    candidate_owner_count_after,
                    true,
                )
                .map_err(EffectExecutorError::Contract)?;
                let _authorized_body_stage_refinement =
                    check_production_effect_to_candidate_transition(adopted_projection)
                        .ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "coalesced body-stage authority refinement failed its incumbent-owner refinement"
                                    .to_owned(),
                            )
                        })?;
                *evidence = adopted;
            }
            match (admission, candidate_semantic_identity) {
                (RuntimeCandidateAdmissionDisposition::FirstAdmission, Some(identity)) => {
                    retained_candidate_owners.insert(identity, evidence.clone());
                    retain_effect.push(true);
                }
                (RuntimeCandidateAdmissionDisposition::CoalescedRetry, Some(_)) => {
                    let redispatch = if runtime_terminal_incumbent {
                        false
                    } else if matches!(
                        fetch_authority_relation,
                        Some(RuntimeFetchAuthorityRelation::Stale)
                    ) {
                        // A weaker carrier can arrive after Prepare/Commit authority
                        // already owns the one physical Fetch. For an ordinary
                        // Proposal, its signed replay envelope belongs to the weaker
                        // incoming lifecycle and cannot be rebound onto the certified
                        // incumbent. The stronger task already performs the complete
                        // acquisition, so every stale carrier terminates here.
                        false
                    } else {
                        Self::candidate_retry_is_redispatched(effect).ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "candidate retry omitted its closed adapter-effect policy"
                                    .to_owned(),
                            )
                        })?
                    };
                    // Redispatched stages reach their idempotent handler with
                    // the incumbent owner and task ID, including an
                    // authenticated retry whose earlier physical command has
                    // already drained. A runtime-owned terminal stutters here
                    // before a cached handler could mint a second callback;
                    // signing likewise stutters while its task remains live.
                    retain_effect.push(redispatch);
                }
                (RuntimeCandidateAdmissionDisposition::NonCandidate, None) => {
                    if Self::candidate_retry_is_redispatched(effect).is_some() {
                        return Err(EffectExecutorError::Contract(
                            "non-candidate effect entered the candidate retry table".to_owned(),
                        ));
                    }
                    retain_effect.push(true);
                }
                _ => {
                    return Err(EffectExecutorError::Contract(
                        "candidate admission disposition disagreed with its bound identity"
                            .to_owned(),
                    ));
                }
            }
            if runtime_terminal_incumbent {
                runtime_terminal_commits.push(index);
            }
            if retain_effect.last() == Some(&true)
                && let Some(key) = replacing_body_stage_lineage
            {
                retire_parked_body_stage_lineages.insert(key);
            }
            if let Some(key) = fetch_key {
                match fetch_authority_relation {
                    Some(RuntimeFetchAuthorityRelation::Stale) => {}
                    Some(RuntimeFetchAuthorityRelation::Same)
                    | Some(RuntimeFetchAuthorityRelation::Upgrade)
                    | None => {
                        retained_fetch_lineages.insert(key, evidence.clone());
                    }
                }
                if retain_effect.last() == Some(&true)
                    && !matches!(
                        fetch_authority_relation,
                        Some(RuntimeFetchAuthorityRelation::Stale)
                    )
                {
                    retire_parked_fetch_lineages.insert(key);
                }
            }
        }
        if !runtime_terminal_commits.is_empty() {
            let terminals = runtime_terminal_commits
                .iter()
                .map(|index| (&effects[*index], &ownership[*index]))
                .collect::<Vec<_>>();
            self.runtime
                .commit_body_pipeline_candidate_terminals(&terminals)
                .map_err(EffectExecutorError::Runtime)?;
        }
        self.durable_validate_retry_seals = retained_validate_retry_seals;
        self.published_lifecycle_store_retry_markers = retained_published_store_retry_markers;
        self.published_lifecycle_validate_retry_markers = retained_published_validate_retry_markers;
        #[cfg(test)]
        if let Some(trace_root) = recovered_validate_retry_trace_root {
            self.last_recovered_validate_retry_trace_root = Some(trace_root);
        }
        #[cfg(test)]
        if let Some(trace_ordinal) = recovered_validate_retry_trace_ordinal {
            self.last_recovered_validate_retry_trace_ordinal = Some(trace_ordinal);
        }
        debug_assert!(effects.iter().all(Self::diagnostic_pending_work_is_exact));
        let retained = effects
            .into_iter()
            .zip(ownership)
            .zip(retain_effect)
            .enumerate()
            .filter_map(|(index, ((effect, ownership), retain))| {
                let highest_prepare_retention = (index == 0
                    && matches!(&effect, AdapterEffect::EnterView { .. }))
                .then_some(frontier.highest_prepare)
                .flatten();
                retain.then_some(OwnedAdapterEffect {
                    effect,
                    ownership,
                    highest_prepare_retention,
                })
            })
            .collect::<VecDeque<_>>();
        if retained.is_empty() {
            return Ok(());
        }
        let replacement_candidates = retained
            .iter()
            .filter_map(|owned| owned.ownership.candidate_semantic_identity())
            .collect::<BTreeSet<_>>();
        if let Some(batch) = self.parked_effect_batch.as_mut() {
            batch.effects.retain(|owned| {
                let replaced_candidate = owned
                    .ownership
                    .candidate_semantic_identity()
                    .is_some_and(|identity| replacement_candidates.contains(&identity));
                let replaced_fetch = match &owned.effect {
                    AdapterEffect::FetchBody {
                        tag,
                        round,
                        subject,
                        ..
                    } => retire_parked_fetch_lineages.contains(&(*tag, *round, *subject)),
                    _ => false,
                };
                let replaced_body_stage = match &owned.effect {
                    AdapterEffect::StoreBody { round, subject, .. }
                    | AdapterEffect::ValidateBody { round, subject, .. } => {
                        production_adapter_effect_candidate_semantic_identity(&owned.effect)
                            .is_some_and(|(kind, _)| {
                                retire_parked_body_stage_lineages
                                    .contains(&(kind, *round, *subject))
                            })
                    }
                    _ => false,
                };
                !replaced_candidate && !replaced_fetch && !replaced_body_stage
            });
        }
        if self
            .parked_effect_batch
            .as_ref()
            .is_some_and(|batch| batch.effects.is_empty())
        {
            self.parked_effect_batch = None;
        }
        self.retained_effect_batch = Some(RetainedEffectBatch {
            effects: retained,
            oldest_at: Instant::now(),
        });
        Ok(())
    }
    /// Snapshot every runnable lifecycle owner retained beyond runtime ingress.
    ///
    /// The executor's maps and retained effect batch are already bounded by
    /// the configured pending-work/completion capacities. Deduplicating by the
    /// immutable ordinal keeps a fan-out lifecycle constant-size for clock
    /// arbitration; two different owners claiming one ordinal fail closed.
    ///
    /// A pending `FetchBody` is deliberately absent. Once its request has been
    /// admitted it is passive network acquisition, not runnable actor work. Its
    /// exact lifecycle owner remains in `pending_fetches` and is transferred to
    /// the reserved `BodyAvailable` completion before the fetch is retired. The
    /// completion therefore re-enters the scheduler at the original ordinal,
    /// while a missing response cannot become a global-minimum barrier to the
    /// timeout, proposal, QC, or retransmit which can resolve that acquisition.
    fn external_lifecycle_owners(&self) -> Result<Vec<RuntimeLifecycleOwner>, EffectExecutorError> {
        let mut owners = BTreeMap::<u128, RuntimeLifecycleOwner>::new();
        let mut insert =
            |owner: &RuntimeLifecycleOwner| match owners.get(&owner.lifecycle_ordinal()) {
                Some(existing) if existing != owner => Err(EffectExecutorError::Contract(
                    "two external lifecycle owners claimed one admission ordinal".to_owned(),
                )),
                Some(_) => Ok(()),
                None => {
                    owners.insert(owner.lifecycle_ordinal(), owner.clone());
                    Ok(())
                }
            };
        if let Some(batch) = &self.retained_effect_batch {
            for owned in &batch.effects {
                insert(owned.ownership.owner())?;
            }
        }
        if let Some(batch) = &self.parked_effect_batch {
            for owned in &batch.effects {
                insert(owned.ownership.owner())?;
            }
        }
        for pending in self.pending_signatures.values() {
            insert(pending.ownership.owner())?;
        }
        for pending in self.pending_stores.values() {
            insert(pending.task.ownership().owner())?;
        }
        for pending in self.pending_applications.values() {
            insert(pending.ownership.owner())?;
        }
        for pending in self.pending_lifecycle_output_admissions.values() {
            let owner = pending.lifecycle_owner();
            insert(&owner)?;
        }
        Ok(owners.into_values().collect())
    }
    fn publish_external_lifecycle_owners(&mut self) -> Result<(), EffectExecutorError> {
        let owners = self.external_lifecycle_owners()?;
        self.runtime
            .set_external_lifecycle_owners(owners)
            .map_err(EffectExecutorError::Runtime)
    }
    fn park_retained_effect_batch(&mut self) -> Result<(), EffectExecutorError> {
        if self.parked_effect_batch.is_some() {
            return Err(EffectExecutorError::Contract(
                "a second ordinary suffix attempted to enter the pacemaker escape".to_owned(),
            ));
        }
        let batch = self.retained_effect_batch.take().ok_or_else(|| {
            EffectExecutorError::Contract(
                "pacemaker escape attempted to park missing dispatch debt".to_owned(),
            )
        })?;
        self.parked_effect_batch = Some(batch);
        Ok(())
    }
    fn restore_parked_effect_batch(&mut self) -> Result<(), EffectExecutorError> {
        if self.retained_effect_batch.is_some() {
            return Err(EffectExecutorError::Contract(
                "pacemaker control debt still occupied the dispatch slot".to_owned(),
            ));
        }
        self.retained_effect_batch = self.parked_effect_batch.take();
        Ok(())
    }
    /// Drain the retained causal suffix in exact FIFO order.
    ///
    /// Pending-work and certified-request exhaustion are retryable for every
    /// pending-work producer. The exact owned `FetchBody` remains at the FIFO
    /// head until capacity changes; periodic production only coalesces. Exact body transport
    /// completions remain admissible while retained pending-work debt exists
    /// and can release that resource. Every other boundary failure remains
    /// fail-closed. Durable Decision ownership is reconciled before every
    /// attempt so a suffix retained across a local completion cannot resurrect
    /// work finality retired in the meantime.
    fn drain_retained_effect_batch<S: V2EffectServices>(
        &mut self,
        services: &mut S,
        restore_parked: bool,
    ) -> Result<usize, EffectExecutorError> {
        if restore_parked
            && self.retained_effect_batch.is_none()
            && self.parked_effect_batch.is_some()
        {
            self.restore_parked_effect_batch()?;
        }
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
                .retain(|owned| Self::effect_survives_decision(&owned.effect, decision));
        }
        if let Some(decision) = decision
            && let Some(batch) = self.parked_effect_batch.as_mut()
        {
            batch
                .effects
                .retain(|owned| Self::effect_survives_decision(&owned.effect, decision));
        }
        if self
            .retained_effect_batch
            .as_ref()
            .is_some_and(|batch| batch.effects.is_empty())
        {
            self.retained_effect_batch = None;
        }
        if self
            .parked_effect_batch
            .as_ref()
            .is_some_and(|batch| batch.effects.is_empty())
        {
            self.parked_effect_batch = None;
        }
        let mut consumed = 0usize;
        loop {
            let Some(owned) = self
                .retained_effect_batch
                .as_ref()
                .and_then(|batch| batch.effects.front())
                .cloned()
            else {
                break;
            };
            if let (
                Some(owner),
                AdapterEffect::Apply {
                    subject,
                    certificate,
                    ..
                },
            ) = (
                self.live_lifecycle_validate_successor.as_ref(),
                &owned.effect,
            ) {
                if !owner.exactly_matches_apply(*subject, certificate) {
                    return Err(EffectExecutorError::Contract(
                        "reducer Apply conflicts with the retained Validate successor".to_owned(),
                    ));
                }
                // Retain the complete owned Apply at the FIFO head. It may be
                // consumed only after the exact Validate-to-Apply child is
                // durably published and upgrades this preliminary owner.
                break;
            }
            let released_validation_will_apply = match &owned.effect {
                AdapterEffect::ValidateBody { round, subject, .. } => self
                    .published_lifecycle_validate_retry_markers
                    .get(&(*round, *subject))
                    .is_some_and(|marker| {
                        !marker.owns_live_lifecycle_row()
                            && marker.latest_statement.phase() == Some(wire::GlobalPhase::Commit)
                    }),
                _ => false,
            };
            // Decision emits CommitQC before Apply. Keep either the Apply or
            // an exact released-marker Validate which will atomically emit
            // Apply at the FIFO head until every lifecycle admission owner
            // settles and the synchronous runner retires its local handoff.
            if (matches!(&owned.effect, AdapterEffect::Apply { .. })
                || released_validation_will_apply)
                && self.decision_apply_dispatch_barrier_is_occupied()
            {
                break;
            }
            let pending_work_producer = Self::pending_work_producer(&owned.effect);
            match self.consume_one(
                owned.effect,
                owned.ownership,
                owned.highest_prepare_retention,
                services,
            ) {
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
                Err(
                    EffectExecutorError::PendingWorkCapacity { .. }
                    | EffectExecutorError::CertifiedRequestCapacity { .. },
                ) => {
                    debug_assert!(pending_work_producer.is_some());
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        self.publish_status(services)?;
        Ok(consumed)
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
            AdapterEffect::ReportEquivocation { .. } => {
                RestartEffectSource::DurableAccountabilityEvidence
            }
            AdapterEffect::ReportInvalidCertifiedBody { .. } => RestartEffectSource::DiagnosticOnly,
        }
    }
    /// Closed retry policy for every candidate-producing adapter effect.
    ///
    /// Fetch, Store, Validate, and Apply services accept the incumbent task ID
    /// idempotently. Signing has no such downstream fanout requirement, so an
    /// exact retry stutters while its sole signature task remains live. The
    /// three Sign request forms plus the four service stages cover all seven
    /// candidate kinds; the remaining four adapter-effect classes are
    /// non-candidates and therefore never reach this table.
    fn candidate_retry_is_redispatched(effect: &AdapterEffect) -> Option<bool> {
        match effect {
            AdapterEffect::Sign {
                request:
                    SignRequest::Proposal(_) | SignRequest::Vote(_) | SignRequest::TimeoutVote(_),
                ..
            } => Some(false),
            AdapterEffect::FetchBody { .. }
            | AdapterEffect::StoreBody { .. }
            | AdapterEffect::ValidateBody { .. }
            | AdapterEffect::Apply { .. } => Some(true),
            AdapterEffect::Broadcast(_)
            | AdapterEffect::EnterView { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => None,
        }
    }
    /// Return whether an already-emitted effect remains owned after durable finality.
    ///
    /// The exact CommitQC may still need propagation, and the exact decided body must finish its
    /// local recovery/application pipeline. Diagnostic reports do not create consensus work.
    /// Every other effect belongs to a pre-Decision transition and is terminally stale.
    fn effect_survives_decision(effect: &AdapterEffect, decision: DurableDecision) -> bool {
        let (decision_round, proposal_round, decision_subject, decision_commitment) = decision;
        match effect {
            AdapterEffect::Broadcast(message) => matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate)
                    if certificate.phase == wire::GlobalPhase::Commit
                        && certificate.round == decision_round
                        && certificate.proposal_round == proposal_round
                        && certificate.subject == decision_subject
                        && certificate.execution_commitment == decision_commitment
            ),
            AdapterEffect::FetchBody { round, subject, .. }
            | AdapterEffect::StoreBody { round, subject, .. }
            | AdapterEffect::ValidateBody { round, subject, .. } => {
                *subject == decision_subject && *round == proposal_round
            }
            AdapterEffect::Apply {
                subject,
                certificate,
                ..
            } => {
                *subject == decision_subject
                    && certificate.phase == wire::GlobalPhase::Commit
                    && certificate.round == decision_round
                    && certificate.proposal_round == proposal_round
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
    /// required to resolve from the already reopened local catalogs before its sole exact effect
    /// is dispatched. A skipped, duplicated, reordered, retagged, or recertified stage fails
    /// closed.
    pub(crate) fn consume_pending_tip_recovery_effects<S: V2EffectServices>(
        &mut self,
        effects: Vec<AdapterEffect>,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        if effects.len() > MAX_EFFECTS_PER_STEP {
            return Err(self.close(
                EffectExecutorError::Contract(format!(
                    "one recovery adapter macro-step emitted {} effects above the adapter bound {MAX_EFFECTS_PER_STEP}",
                    effects.len()
                )),
                services,
            ));
        }
        if let Some(stage) = self
            .pending_kura_apply_recovery_evidence()
            .map(PendingKuraApplyRecoveryEvidence::stage)
        {
            let is_effect_stage = matches!(
                stage,
                PendingKuraApplyRecoveryStage::CertifiedFetch
                    | PendingKuraApplyRecoveryStage::DurableStore
                    | PendingKuraApplyRecoveryStage::DeterministicValidation
                    | PendingKuraApplyRecoveryStage::Apply
            );
            let invalid_count = if is_effect_stage {
                effects.len() != 1
            } else {
                !effects.is_empty()
            };
            if invalid_count {
                let reason = if is_effect_stage {
                    "interrupted-tip recovery must emit exactly one effect for its current stage"
                } else {
                    "completed interrupted-tip recovery must not emit another effect"
                };
                return Err(self.close(EffectExecutorError::Contract(reason.to_owned()), services));
            }
        }
        for effect in &effects {
            if let Err(error) = self.ensure_pending_tip_recovery_effect_is_local(effect) {
                return Err(self.close(error, services));
            }
        }
        let frontier = self
            .runtime
            .reconciliation_frontier()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        if let Err(error) = self.preflight_effect_batch_frontier(&effects, frontier) {
            return Err(self.close(error, services));
        }
        let ownership = self
            .runtime
            .take_effect_ownership(&effects)
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        if let Err(error) = self.retain_effect_batch_at_frontier(effects, ownership, frontier) {
            return Err(self.close(error, services));
        }
        if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
            return Err(self.close(error, services));
        }
        let count = self
            .drain_retained_effect_batch(services, true)
            .map_err(|error| self.close_after_transferring_runtime_terminals(error, services))?;
        if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
            return Err(self.close(error, services));
        }
        Ok(count)
    }
    fn consume_pacemaker_effects_with_runner_decision_cleanup<S: V2EffectServices>(
        &mut self,
        effects: Vec<AdapterEffect>,
        services: &mut S,
        pending_runner_decision_cleanup: Option<PendingRunnerDecisionCleanup>,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        if effects.len() > MAX_EFFECTS_PER_STEP {
            return Err(self.close(
                EffectExecutorError::Contract(format!(
                    "one pacemaker adapter macro-step emitted {} effects above the adapter bound {MAX_EFFECTS_PER_STEP}",
                    effects.len()
                )),
                services,
            ));
        }
        if self.pending_runner_decision_cleanup.is_some() {
            return Err(self.close(
                EffectExecutorError::Contract(
                    "pacemaker effects overtook pending runner Decision cleanup".to_owned(),
                ),
                services,
            ));
        }
        if let Some(pending) = pending_runner_decision_cleanup
            && !Self::new_decision_batch_has_only_exact_apply(
                &effects,
                pending.decision,
                Some(pending.owner_tag),
            )
        {
            return Err(self.close(
                EffectExecutorError::Contract(
                    "pacemaker Decision Apply handoff changed its exact retained suffix".to_owned(),
                ),
                services,
            ));
        }
        let frontier = self
            .runtime
            .reconciliation_frontier()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        if let Err(error) = self.preflight_effect_batch_frontier(&effects, frontier) {
            return Err(self.close(error, services));
        }
        let ownership = self
            .runtime
            .take_effect_ownership(&effects)
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        if ownership
            .iter()
            .any(|evidence| evidence.owner().causal_origin().root_class != SERVICE_CLASS_PROGRESS)
        {
            return Err(self.close(
                EffectExecutorError::Contract(
                    "typed pacemaker escape returned a non-Progress causal owner".to_owned(),
                ),
                services,
            ));
        }
        if let Err(error) = self.retain_effect_batch_at_frontier(effects, ownership, frontier) {
            return Err(self.close(error, services));
        }
        self.pending_runner_decision_cleanup = pending_runner_decision_cleanup;
        if let Err(error) = self.commit_reconciliation_frontier(frontier, services) {
            return Err(self.close_after_transferring_runtime_terminals(error, services));
        }
        if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
            return Err(self.close(error, services));
        }
        let count = self
            .drain_retained_effect_batch(services, false)
            .map_err(|error| self.close_after_transferring_runtime_terminals(error, services))?;
        if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
            return Err(self.close(error, services));
        }
        Ok(count)
    }
    /// Freeze the already-due timeout owner and its fixed fair-ingress cut
    /// only when the reducer exposes an actionable unchanged-lock Prepare
    /// target.
    pub(crate) fn freeze_pre_timeout_locked_prepare_qc_cut(
        &mut self,
        now: Instant,
        physical_cut: u128,
    ) -> Result<Option<PreTimeoutLockedPrepareQcCutV1>, EffectExecutorError> {
        self.ensure_open()?;
        if self.retained_effect_batch.is_some()
            || self.parked_effect_batch.is_some()
            || self.pending_runner_decision_cleanup.is_some()
        {
            return Ok(None);
        }
        self.publish_external_lifecycle_owners()?;
        self.runtime
            .set_ingress_physical_cut(physical_cut)
            .map_err(EffectExecutorError::Runtime)?;
        self.runtime
            .freeze_pre_timeout_locked_prepare_qc_cut(now)
            .map_err(EffectExecutorError::Runtime)
    }
    /// Deep-preview one fair-ingress payload without consuming its queue row.
    pub(crate) fn wire_previews_pre_timeout_locked_prepare_qc(
        &self,
        cut: &PreTimeoutLockedPrepareQcCutV1,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        self.runtime
            .wire_previews_pre_timeout_locked_prepare_qc(cut, payload)
    }
    /// Dispatch one exact already-admitted pre-cut Prepare carrier and consume
    /// its effects through the ordinary Progress-root executor path.
    pub(crate) fn step_pre_timeout_locked_prepare_qc_once<S: V2EffectServices>(
        &mut self,
        now: Instant,
        cut: &PreTimeoutLockedPrepareQcCutV1,
        services: &mut S,
    ) -> Result<EffectExecutorStep, EffectExecutorError> {
        self.ensure_open()?;
        if self.retained_effect_batch.is_some()
            || self.parked_effect_batch.is_some()
            || self.pending_runner_decision_cleanup.is_some()
        {
            return Ok(EffectExecutorStep::Idle);
        }
        if let Err(error) = self.publish_external_lifecycle_owners() {
            return Err(self.close(error, services));
        }
        let decision_before_step = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        let wal_step = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| {
                EffectExecutorError::FailClosed(
                    "process restart is required after a fatal consensus failure".to_owned(),
                )
            })?;
        let step = match self
            .runtime
            .step_pre_timeout_locked_prepare_qc_effects(now, cut)
        {
            Ok(step) => step,
            Err(reason) => {
                drop(wal_step);
                return Err(self.close(EffectExecutorError::Runtime(reason), services));
            }
        };
        if step.is_some()
            && let Err(reason) = self.runtime.take_scheduler_ownership()
        {
            drop(wal_step);
            return Err(self.close(EffectExecutorError::Runtime(reason), services));
        }
        wal_step.complete();
        if let Err(error) = self.finish_runtime_step_reconciliation(services) {
            return Err(self.close(error, services));
        }
        let decision_after_step = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        let pending_runner_decision_cleanup = self
            .plan_runner_decision_cleanup(decision_before_step, decision_after_step)
            .map_err(|error| self.close(error, services))?;
        match step {
            None | Some(RuntimeStep::Idle) => {
                self.pending_runner_decision_cleanup = pending_runner_decision_cleanup;
                if let Err(error) = self.publish_external_lifecycle_owners() {
                    return Err(self.close(error, services));
                }
                if let Err(error) = self.publish_status(services) {
                    return Err(self.close(error, services));
                }
                Ok(EffectExecutorStep::Idle)
            }
            Some(RuntimeStep::Advanced(effects)) => {
                let count = self.consume_pacemaker_effects_with_runner_decision_cleanup(
                    effects,
                    services,
                    pending_runner_decision_cleanup,
                )?;
                Ok(EffectExecutorStep::Advanced { effects: count })
            }
        }
    }
    /// Give one bounded scheduler turn only to absolute timeout or an
    /// authenticated Progress-root lifecycle.
    ///
    /// If ordinary adapter debt occupies the dispatch slot, its exact suffix
    /// is parked first and restored after the control turn. A retained control
    /// suffix is drained before another scheduler owner may be selected.
    pub(crate) fn step_pacemaker_once<S: V2EffectServices>(
        &mut self,
        now: Instant,
        services: &mut S,
    ) -> Result<EffectExecutorStep, EffectExecutorError> {
        self.ensure_open()?;
        if self.pending_runner_decision_cleanup.is_some() {
            let count = self
                .drain_retained_effect_batch(services, false)
                .map_err(|error| {
                    self.close_after_transferring_runtime_terminals(error, services)
                })?;
            if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
                return Err(self.close(error, services));
            }
            return Ok(if count == 0 {
                EffectExecutorStep::Idle
            } else {
                EffectExecutorStep::Advanced { effects: count }
            });
        }
        if self.retained_effect_batch.is_some() && self.parked_effect_batch.is_some() {
            let count = self
                .drain_retained_effect_batch(services, false)
                .map_err(|error| {
                    self.close_after_transferring_runtime_terminals(error, services)
                })?;
            if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
                return Err(self.close(error, services));
            }
            return Ok(if count == 0 {
                EffectExecutorStep::Idle
            } else {
                EffectExecutorStep::Advanced { effects: count }
            });
        }
        if self.retained_effect_batch.is_some() {
            self.park_retained_effect_batch()
                .map_err(|error| self.close(error, services))?;
        }
        if let Err(error) = self.publish_external_lifecycle_owners() {
            return Err(self.close(error, services));
        }
        let decision_before_step = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        let wal_step = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| {
                EffectExecutorError::FailClosed(
                    "process restart is required after a fatal consensus failure".to_owned(),
                )
            })?;
        let step = match self.runtime.step_pacemaker_effects(now) {
            Ok(step) => step,
            Err(reason) => {
                drop(wal_step);
                return Err(self.close(EffectExecutorError::Runtime(reason), services));
            }
        };
        if step.is_some()
            && let Err(reason) = self.runtime.take_scheduler_ownership()
        {
            drop(wal_step);
            return Err(self.close(EffectExecutorError::Runtime(reason), services));
        }
        wal_step.complete();
        if let Err(error) = self.finish_runtime_step_reconciliation(services) {
            return Err(self.close(error, services));
        }
        let decision_after_step = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        let pending_runner_decision_cleanup = self
            .plan_runner_decision_cleanup(decision_before_step, decision_after_step)
            .map_err(|error| self.close(error, services))?;
        match step {
            None | Some(RuntimeStep::Idle) => {
                self.pending_runner_decision_cleanup = pending_runner_decision_cleanup;
                if self.retained_effect_batch.is_none() && self.parked_effect_batch.is_some() {
                    self.restore_parked_effect_batch()
                        .map_err(|error| self.close(error, services))?;
                }
                if let Err(error) = self.publish_external_lifecycle_owners() {
                    return Err(self.close(error, services));
                }
                if let Err(error) = self.publish_status(services) {
                    return Err(self.close(error, services));
                }
                Ok(EffectExecutorStep::Idle)
            }
            Some(RuntimeStep::Advanced(effects)) => {
                let count = self.consume_pacemaker_effects_with_runner_decision_cleanup(
                    effects,
                    services,
                    pending_runner_decision_cleanup,
                )?;
                Ok(EffectExecutorStep::Advanced { effects: count })
            }
        }
    }
    /// Run at most one serialized runtime step and dispatch all of its effects.
    pub(crate) fn step<S: V2EffectServices>(
        &mut self,
        now: Instant,
        services: &mut S,
    ) -> Result<EffectExecutorStep, EffectExecutorError> {
        self.ensure_open()?;
        if self.pending_runner_decision_cleanup.is_some()
            && self.retained_effect_batch.is_none()
            && self.parked_effect_batch.is_none()
        {
            return Ok(EffectExecutorStep::Idle);
        }
        if self.retained_effect_batch.is_some() || self.parked_effect_batch.is_some() {
            let count = self
                .drain_retained_effect_batch(services, true)
                .map_err(|error| {
                    self.close_after_transferring_runtime_terminals(error, services)
                })?;
            if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
                return Err(self.close(error, services));
            }
            if count != 0 {
                return Ok(EffectExecutorStep::Advanced { effects: count });
            }
            if self.pending_runner_decision_cleanup.is_some() {
                return Ok(EffectExecutorStep::Idle);
            }
            if self.retained_effect_batch.is_some() && self.parked_effect_batch.is_none() {
                self.park_retained_effect_batch()
                    .map_err(|error| self.close(error, services))?;
                return self.step_pacemaker_once(now, services);
            }
            return Ok(EffectExecutorStep::Idle);
        }
        if let Err(error) = self.publish_external_lifecycle_owners() {
            return Err(self.close(error, services));
        }
        let decision_before_step = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
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
        #[cfg(test)]
        let selected = self.runtime.last_scheduler_selection_for_test();
        if let Err(reason) = self.runtime.take_scheduler_ownership() {
            drop(wal_step);
            return Err(self.close(EffectExecutorError::Runtime(reason), services));
        }
        // Runtime stepping includes the safety-WAL append. Release its permit
        // before invoking any service callback so service operations acquire
        // their own non-nested guard boundary.
        wal_step.complete();
        if let Err(error) = self.finish_runtime_step_reconciliation(services) {
            return Err(self.close(error, services));
        }
        let decision_after_step = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)
            .map_err(|error| self.close(error, services))?;
        let pending_runner_decision_cleanup = self
            .plan_runner_decision_cleanup(decision_before_step, decision_after_step)
            .map_err(|error| self.close(error, services))?;
        match step {
            RuntimeStep::Idle => {
                self.pending_runner_decision_cleanup = pending_runner_decision_cleanup;
                #[cfg(test)]
                {
                    self.last_runtime_step_observation = Some(RuntimeStepObservationV1 {
                        selected,
                        effect_count: 0,
                        validate_count: 0,
                        non_validate_class: None,
                        broadcast_count: 0,
                        canonical_prepare_qc_digest: None,
                    });
                }
                if let Err(error) = self.publish_status(services) {
                    return Err(self.close(error, services));
                }
                Ok(EffectExecutorStep::Idle)
            }
            RuntimeStep::Advanced(effects) => {
                #[cfg(test)]
                {
                    self.last_runtime_step_observation = Some(RuntimeStepObservationV1 {
                        selected,
                        effect_count: effects.len(),
                        validate_count: effects
                            .iter()
                            .filter(|effect| matches!(effect, AdapterEffect::ValidateBody { .. }))
                            .count(),
                        non_validate_class: observed_non_validate_class(&effects),
                        broadcast_count: effects
                            .iter()
                            .filter(|effect| matches!(effect, AdapterEffect::Broadcast(_)))
                            .count(),
                        canonical_prepare_qc_digest: observed_canonical_prepare_qc_digest(&effects),
                    });
                }
                let count = self.consume_effects_with_runner_decision_cleanup(
                    effects,
                    services,
                    pending_runner_decision_cleanup,
                )?;
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
        self.pending_tip_recovery_attempts = self.pending_tip_recovery_attempts.saturating_add(1);
        if self.retained_effect_batch.is_some() {
            let count = self
                .drain_retained_effect_batch(services, true)
                .map_err(|error| {
                    self.close_after_transferring_runtime_terminals(error, services)
                })?;
            if let Err(error) = self.consume_leader_wire_runtime_terminals(services) {
                return Err(self.close(error, services));
            }
            let step = if count == 0 {
                self.pending_tip_recovery_last_result =
                    Some(PendingTipRecoveryAttemptResult::Waiting);
                EffectExecutorStep::Idle
            } else {
                self.pending_tip_recovery_last_result =
                    Some(PendingTipRecoveryAttemptResult::Advanced);
                EffectExecutorStep::Advanced { effects: count }
            };
            self.publish_status(services)
                .map_err(|error| self.close(error, services))?;
            return Ok(step);
        }
        if let Err(error) = self.publish_external_lifecycle_owners() {
            return Err(self.close(error, services));
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
        if let Err(reason) = self.runtime.take_scheduler_ownership() {
            drop(wal_step);
            return Err(self.close(EffectExecutorError::Runtime(reason), services));
        }
        wal_step.complete();
        if let Err(error) = self.finish_runtime_step_reconciliation(services) {
            return Err(self.close(error, services));
        }
        match step {
            RuntimeStep::Idle => {
                self.pending_tip_recovery_last_result =
                    Some(PendingTipRecoveryAttemptResult::Waiting);
                if let Err(error) = self.publish_status(services) {
                    return Err(self.close(error, services));
                }
                Ok(EffectExecutorStep::Idle)
            }
            RuntimeStep::Advanced(effects) => {
                let count = self.consume_pending_tip_recovery_effects(effects, services)?;
                self.pending_tip_recovery_last_result =
                    Some(PendingTipRecoveryAttemptResult::Advanced);
                self.publish_status(services)
                    .map_err(|error| self.close(error, services))?;
                Ok(EffectExecutorStep::Advanced { effects: count })
            }
        }
    }
    /// Publish the terminal scheduler observation immediately before the runner
    /// latches restart-required for an exhausted recovery deadline.
    pub(crate) fn record_pending_tip_recovery_deadline_exceeded<S: V2EffectServices>(
        &mut self,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        self.pending_tip_recovery_last_result =
            Some(PendingTipRecoveryAttemptResult::DeadlineExceeded);
        self.publish_status(services)
            .map_err(|error| self.close(error, services))
    }
    /// Number of serialized interrupted-tip recovery attempts made so far.
    pub(crate) const fn pending_tip_recovery_attempts(&self) -> u64 {
        self.pending_tip_recovery_attempts
    }
    /// Begin the asynchronous durable-store → deterministic-validation chain
    /// for a locally built proposal.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn admit_local_proposal<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        canonical_wire: Vec<u8>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        let key = (manifest.round, manifest.subject);
        let has_recovered_pre_intent = self.recovered_bodies.contains_key(&key)
            || self.retired_rejected_bodies.contains_key(&key);
        if let Err(error) = manifest.validate(&self.context) {
            let error = EffectExecutorError::Contract(error.to_string());
            return Err(if has_recovered_pre_intent {
                self.close(error, services)
            } else {
                error
            });
        }
        if u64::try_from(canonical_wire.len()).ok() != Some(manifest.payload_size_bytes)
            || Hash::new(&canonical_wire) != manifest.subject.payload_hash
        {
            let error = EffectExecutorError::Contract(
                "local proposal bytes do not match the canonical manifest".to_owned(),
            );
            return Err(if has_recovered_pre_intent {
                self.close(error, services)
            } else {
                error
            });
        }
        if manifest.round.context_id != self.context.id()
            || self.runtime.authoritative_tag() != Some(tag)
            || manifest.round.height != tag.height()
            || manifest.round.view != tag.view()
        {
            let error = EffectExecutorError::Contract(
                "local proposal does not belong to the exact authoritative round".to_owned(),
            );
            return Err(if has_recovered_pre_intent {
                self.close(error, services)
            } else {
                error
            });
        }
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        if let Some((work_id, pending)) = self
            .pending_stores
            .iter()
            .find(|(_, pending)| {
                pending.task.manifest.round == manifest.round
                    && pending.task.manifest.subject == manifest.subject
            })
            .map(|(work_id, pending)| (*work_id, pending.clone()))
        {
            let exact_consumer = matches!(
                &pending.consumer,
                Some(StoreConsumer::LocalProposal {
                    tag: consumer_tag,
                    ownership,
                    ..
                }) if *consumer_tag == tag && ownership == pending.task.ownership()
            );
            let exact_replay = self.local_store_replay.get(&work_id).is_some_and(|replay| {
                replay.exactly_matches_store_task(
                    &store_effect,
                    &manifest,
                    pending.task.ownership(),
                )
            });
            if !exact_consumer
                || !exact_replay
                || pending.task.manifest != manifest
                || pending.task.canonical_wire.as_ref() != canonical_wire.as_slice()
            {
                return Err(EffectExecutorError::Contract(
                    "local proposal retry changed its exact Store owner or replay seal".to_owned(),
                ));
            }
            services
                .enqueue_body_store(pending.task)
                .map_err(service_error)?;
            return self
                .publish_status(services)
                .map_err(|error| self.close(error, services));
        }
        if let Some(pending) = self.pending_durable_validate_admissions.get(&key) {
            let exact_durable_body =
                self.recovered_bodies
                    .get(&key)
                    .is_some_and(|(retained_manifest, receipt)| {
                        retained_manifest == &manifest
                            && receipt.manifest_hash() == HashOf::new(&manifest)
                            && self.durable_bodies.get(&key) == Some(receipt)
                            && pending.exactly_matches_local_body_retry(tag, &manifest, receipt)
                    });
            if !exact_durable_body {
                return Err(EffectExecutorError::Contract(
                    "local proposal retry changed its exact pending lifecycle Validate body"
                        .to_owned(),
                ));
            }
            return self
                .publish_status(services)
                .map_err(|error| self.close(error, services));
        }
        if self
            .local_proposal_ready_replay
            .iter()
            .any(|(identity, replay)| replay.exactly_matches_retry(*identity, tag, &manifest))
            || self
                .local_proposal_intent_replay
                .iter()
                .any(|(identity, replay)| replay.exactly_matches_retry(*identity, tag, &manifest))
        {
            return self
                .publish_status(services)
                .map_err(|error| self.close(error, services));
        }
        let local_origin = match self.recovered_bodies.get(&key) {
            Some((recovered_manifest, recovered_receipt)) => {
                if recovered_manifest != &manifest
                    || !store_completion_matches(&self.context, &manifest, recovered_receipt)
                    || self
                        .durable_bodies
                        .get(&key)
                        .is_none_or(|durable| durable != recovered_receipt)
                    || self
                        .validated_bodies
                        .get(&key)
                        .is_some_and(|validated| validated.durable() != recovered_receipt)
                {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "cold local proposal differs from its exact recovered body frame"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
                if self.rejected_bodies.contains_key(&key)
                    || self.retired_rejected_bodies.contains_key(&key)
                {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "cold local proposal body has a durable deterministic rejection"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
                LocalProposalBodyOrigin::RecoveredPreIntent
            }
            None => {
                if self.durable_bodies.contains_key(&key)
                    || self.validated_bodies.contains_key(&key)
                    || self.rejected_bodies.contains_key(&key)
                    || self.retired_rejected_bodies.contains_key(&key)
                {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "local proposal retry found durable body state without its exact recovered frame"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
                LocalProposalBodyOrigin::Fresh
            }
        };
        let owner_plan = self.plan_body_pipeline_owner(tag, &manifest)?;
        let replay_ownership = self
            .runtime
            .mint_local_proposal_effect_ownership(tag, &manifest)
            .map_err(EffectExecutorError::Runtime)?;
        let ownership = replay_ownership
            .exact_store_task_ownership(&store_effect, &manifest)
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "local proposal runtime mint did not own its exact Store work".to_owned(),
                )
            })?;
        if let Err(error) = self.begin_store_with_plans(
            tag,
            manifest.clone(),
            Arc::from(canonical_wire),
            StorePurpose::LocalProposal,
            local_origin,
            None,
            Some(owner_plan),
            ownership.clone(),
            services,
        ) {
            return Err(self.close(error, services));
        }
        let work_id = self.pending_stores.iter().find_map(|(work_id, pending)| {
            (pending.task.manifest == manifest
                && pending.task.ownership() == &ownership
                && matches!(
                    &pending.consumer,
                    Some(StoreConsumer::LocalProposal {
                        tag: consumer_tag,
                        ownership: consumer_ownership,
                        ..
                    }) if *consumer_tag == tag && consumer_ownership == &ownership
                ))
            .then_some(*work_id)
        });
        if let Some(work_id) = work_id {
            let duplicate = match self.local_store_replay.entry(work_id) {
                Entry::Vacant(slot) => {
                    slot.insert(replay_ownership);
                    false
                }
                Entry::Occupied(_) => true,
            };
            if duplicate {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "local proposal Store admission duplicated replay authority".to_owned(),
                    ),
                    services,
                ));
            }
        } else {
            let receipt = self.durable_bodies.get(&key).cloned().ok_or_else(|| {
                self.close(
                    EffectExecutorError::Contract(
                        "local proposal Store admission installed neither pending nor durable work"
                            .to_owned(),
                    ),
                    services,
                )
            })?;
            let validate_effect = AdapterEffect::ValidateBody {
                tag,
                round: manifest.round,
                subject: manifest.subject,
            };
            let validate_ownership = ownership
                .rebind_as_inherited_adapter_effect(&validate_effect)
                .map_err(|reason| self.close(EffectExecutorError::Contract(reason), services))?;
            let store_terminal =
                DurableStoreTerminalRetrySealV1::seal_exact(&store_effect, &ownership, &receipt)
                    .ok_or_else(|| {
                        self.close(
                            EffectExecutorError::Contract(
                                "durable local Store could not seal its terminal retry owner"
                                    .to_owned(),
                            ),
                            services,
                        )
                    })?;
            let validate_replay = replay_ownership
                .project_exact_validate(
                    &store_effect,
                    &manifest,
                    &receipt,
                    &validate_effect,
                    &validate_ownership,
                )
                .map_err(|_| {
                    self.close(
                        EffectExecutorError::Contract(
                            "durable local Store could not project its exact Validate replay"
                                .to_owned(),
                        ),
                        services,
                    )
                })?;
            let prepared = PreparedLocalBodyValidateReplayPreAdmission::seal_exact_validate(
                validate_effect.clone(),
                validate_ownership.clone(),
                receipt.clone(),
                validate_replay,
            )
            .map_err(|_| {
                self.close(
                    EffectExecutorError::Contract(
                        "durable local Store could not seal its lifecycle Validate owner"
                            .to_owned(),
                    ),
                    services,
                )
            })?;
            self.recovered_bodies
                .entry(key)
                .or_insert_with(|| (manifest.clone(), receipt));
            self.install_pending_durable_validate_admission(
                key,
                &validate_effect,
                &validate_ownership,
                prepared.into_pending_durable_validate_admission(),
                Some(store_terminal),
            )
            .map_err(|error| self.close(error, services))?;
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
        let ownership = pending.ownership.clone();
        if let Err(error) = self
            .runtime
            .enqueue_signature_with_owner(tag, signature, &ownership)
        {
            return Err(self.close(runtime_enqueue_error(error), services));
        }
        self.pending_signatures.remove(&work_id);
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Accepted)
    }
    /// Accept a body-store-minted durable completion under its immutable task.
    #[allow(clippy::too_many_lines)]
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
            if self.local_store_replay.contains_key(&completion.work_id()) {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "orphaned local Store replay authority outlived its exact task".to_owned(),
                    ),
                    services,
                ));
            }
            if matches!(
                self.remote_proposal_replay.get(&key),
                Some(RemoteProposalReplayStageV1::Store { work_id, .. })
                    if *work_id == completion.work_id()
            ) || self
                .authenticated_genesis_replay
                .get(&key)
                .and_then(AuthenticatedGenesisReplayStageV1::store_work_id)
                == Some(completion.work_id())
            {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "orphaned replay-authorized Store authority outlived its exact task"
                            .to_owned(),
                    ),
                    services,
                ));
            }
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
        let completes_remote_proposal_store = match self.remote_proposal_replay.get(&key) {
            Some(RemoteProposalReplayStageV1::Store { work_id, replay }) => {
                let store_effect = AdapterEffect::StoreBody {
                    tag: pending.task.tag(),
                    round: manifest.round,
                    subject: manifest.subject,
                };
                if *work_id != completion.work_id() {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "Proposal Store completion changed its exact replay work ID".to_owned(),
                        ),
                        services,
                    ));
                }
                if !replay.exactly_matches_retry(&store_effect, pending.task.ownership()) {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "Proposal Store completion changed its exact replay owner".to_owned(),
                        ),
                        services,
                    ));
                }
                true
            }
            Some(RemoteProposalReplayStageV1::Fetch { .. })
            | Some(RemoteProposalReplayStageV1::BodyAvailable(_))
            | Some(RemoteProposalReplayStageV1::StoreAdmission(_)) => {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "Proposal Store completion preceded its retained replay stage".to_owned(),
                    ),
                    services,
                ));
            }
            Some(RemoteProposalReplayStageV1::Stored { .. }) | None => false,
        };
        let completes_authenticated_genesis_store = self
            .preflight_authenticated_genesis_store_completion(key, &pending, completion.work_id())
            .map_err(|error| self.close(error, services))?;
        if completes_remote_proposal_store && completes_authenticated_genesis_store {
            return Err(self.close(
                EffectExecutorError::Contract(
                    "one Store completion retained two replay authorities".to_owned(),
                ),
                services,
            ));
        }
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
                StoreConsumer::Reducer { tag, .. } | StoreConsumer::LocalProposal { tag, .. } => {
                    *tag
                }
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
        // A certified lifecycle Fetch may publish the same immutable body
        // through Store (or already through Validate) while an older ordinary
        // reducer Store is still in flight.  Its eventual worker completion
        // bypasses `retain_effect_batch`, so prove the same marker-authorized
        // stutter here before it can enqueue a second `BodyStored` terminal
        // under the legacy task's foreign lifecycle owner.  The task and any
        // origin replay still settle below; only the duplicate runtime
        // successor is suppressed.  An active Store marker retains a valid
        // monotonic authority refinement, whereas the post-Validate marker is
        // already at least as strong as every Store completion it may absorb.
        let (marker_effect, marker_ownership, historical_completion) = match &pending.consumer {
            Some(StoreConsumer::Reducer { tag, ownership }) => (
                AdapterEffect::StoreBody {
                    tag: *tag,
                    round: manifest.round,
                    subject: manifest.subject,
                },
                ownership,
                false,
            ),
            Some(StoreConsumer::LocalProposal { .. }) | None => (
                AdapterEffect::StoreBody {
                    tag: pending.task.tag(),
                    round: manifest.round,
                    subject: manifest.subject,
                },
                pending.task.ownership(),
                true,
            ),
        };
        let published_store_completion_plan = match (
            self.published_lifecycle_store_retry_markers.get(&key),
            self.published_lifecycle_validate_retry_markers.get(&key),
        ) {
            (Some(_), Some(_)) => {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "late Store completion found two published lifecycle terminals".to_owned(),
                    ),
                    services,
                ));
            }
            (Some(marker), None) => {
                let projection = if historical_completion {
                    marker.project_historical_store_completion(
                        &receipt,
                        &marker_effect,
                        marker_ownership,
                    )
                } else {
                    marker.project_active_store_retry(&receipt, &marker_effect, marker_ownership)
                };
                let projected = match projection {
                    Ok(projected) => projected,
                    Err(reason) => {
                        return Err(self.close(EffectExecutorError::Contract(reason), services));
                    }
                };
                PublishedLifecycleStoreCompletionPlanV1::ActiveStore(projected)
            }
            (None, Some(marker)) => {
                let projection = if historical_completion {
                    marker.project_historical_store_completion(
                        &receipt,
                        &marker_effect,
                        marker_ownership,
                    )
                } else {
                    marker.project_store_retry(&receipt, &marker_effect, marker_ownership)
                };
                if let Err(reason) = projection {
                    return Err(self.close(EffectExecutorError::Contract(reason), services));
                }
                PublishedLifecycleStoreCompletionPlanV1::PublishedValidate
            }
            (None, None) => PublishedLifecycleStoreCompletionPlanV1::NoPublishedMarker,
        };
        let coalesces_published_store_terminal =
            published_store_completion_plan.coalesces_terminal();
        let coalesced_pipeline_owner = if coalesces_published_store_terminal {
            match (
                &pending.consumer,
                self.body_pipeline_owners.get(&key).copied(),
            ) {
                (Some(_), Some(owner)) => Some(owner),
                (Some(_), None) => unreachable!(
                    "an attached Store completion preflighted its exact pipeline owner"
                ),
                (None, Some(owner))
                    if owner.manifest_hash == Some(HashOf::new(&manifest))
                        && (owner.tag == pending.task.tag()
                            || owner.tag.strictly_advances(pending.task.tag())) =>
                {
                    Some(owner)
                }
                (None, Some(_)) => {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "detached Store completion changed its retained pipeline owner"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
                (None, None) => None,
            }
        } else {
            None
        };
        let mut coalesces_local_store_replay = false;
        let local_replay_projection = match &pending.consumer {
            Some(StoreConsumer::LocalProposal { tag, ownership, .. }) => {
                let store_effect = AdapterEffect::StoreBody {
                    tag: *tag,
                    round: manifest.round,
                    subject: manifest.subject,
                };
                if !self.local_store_replay.contains_key(&completion.work_id()) {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "local body-store completion lost its pre-intent replay seal"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
                let local_store_replay_is_exact = {
                    let replay = self
                        .local_store_replay
                        .get(&completion.work_id())
                        .expect("preflighted local Store replay remains installed");
                    replay.exactly_matches_store_task(
                        &store_effect,
                        &manifest,
                        pending.task.ownership(),
                    )
                };
                if self.pending_durable_validate_admissions.contains_key(&key)
                    || !local_store_replay_is_exact
                {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "local body-store completion changed its exact replay lineage"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
                if coalesces_published_store_terminal {
                    // The published row is already the sole successor. Retire
                    // the exact pre-intent replay below without constructing
                    // any executable Validate capability for a path that must
                    // stutter.
                    coalesces_local_store_replay = true;
                    None
                } else {
                    let validate_effect = AdapterEffect::ValidateBody {
                        tag: *tag,
                        round: manifest.round,
                        subject: manifest.subject,
                    };
                    let validate_ownership = ownership
                        .rebind_as_inherited_adapter_effect(&validate_effect)
                        .map_err(|reason| {
                            self.close(EffectExecutorError::Contract(reason), services)
                        })?;
                    let store_terminal = DurableStoreTerminalRetrySealV1::seal_exact(
                        &store_effect,
                        pending.task.ownership(),
                        &receipt,
                    )
                    .ok_or_else(|| {
                        self.close(
                            EffectExecutorError::Contract(
                                "local Store completion could not seal its terminal retry owner"
                                    .to_owned(),
                            ),
                            services,
                        )
                    })?;
                    let local_validate_replay_is_exact = {
                        let replay = self
                            .local_store_replay
                            .get(&completion.work_id())
                            .expect("preflighted local Store replay remains serialized");
                        replay.exactly_projects_validate_task(
                            &store_effect,
                            &manifest,
                            &receipt,
                            &validate_effect,
                            &validate_ownership,
                        )
                    };
                    if !local_validate_replay_is_exact {
                        return Err(self.close(
                            EffectExecutorError::Contract(
                                "local body-store completion changed its exact Validate lineage"
                                    .to_owned(),
                            ),
                            services,
                        ));
                    }
                    Some((
                        store_effect,
                        validate_effect,
                        validate_ownership,
                        store_terminal,
                    ))
                }
            }
            Some(StoreConsumer::Reducer { .. }) | None => {
                if self.local_store_replay.contains_key(&completion.work_id()) {
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "non-local body-store work retained local replay authority".to_owned(),
                        ),
                        services,
                    ));
                }
                None
            }
        };
        if completes_remote_proposal_store {
            let Some(RemoteProposalReplayStageV1::Store { replay, .. }) =
                self.remote_proposal_replay.remove(&key)
            else {
                unreachable!("preflighted Proposal Store replay remains installed")
            };
            let stored = match replay.bind_durable_body(receipt.clone()) {
                Ok(stored) => stored,
                Err(error) => {
                    let previous = self.remote_proposal_replay.insert(
                        key,
                        RemoteProposalReplayStageV1::Store {
                            work_id: completion.work_id(),
                            replay: error.into_store(),
                        },
                    );
                    debug_assert!(previous.is_none());
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "Proposal Store completion changed its exact durable body".to_owned(),
                        ),
                        services,
                    ));
                }
            };
            if !stored.exactly_retains_owned_store(&receipt, pending.task.ownership()) {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "Proposal Store completion changed its exact retained runtime owner"
                            .to_owned(),
                    ),
                    services,
                ));
            }
            if !coalesces_published_store_terminal {
                let previous = self.remote_proposal_replay.insert(
                    key,
                    RemoteProposalReplayStageV1::Stored {
                        replay: stored,
                        ownership: pending.task.ownership().clone(),
                    },
                );
                debug_assert!(previous.is_none());
            }
        }
        if completes_authenticated_genesis_store {
            self.commit_authenticated_genesis_store_completion(
                key,
                completion.work_id(),
                receipt.clone(),
                pending.task.ownership().clone(),
            )
            .map_err(|error| self.close(error, services))?;
            if coalesces_published_store_terminal {
                let removed = self.authenticated_genesis_replay.remove(&key);
                debug_assert!(matches!(
                    removed,
                    Some(AuthenticatedGenesisReplayStageV1::Stored { .. })
                ));
            }
        }
        match &pending.consumer {
            Some(StoreConsumer::Reducer { tag, ownership }) => {
                if !coalesces_published_store_terminal {
                    self.runtime
                        .enqueue_body_stored_with_owner(
                            *tag,
                            manifest.round,
                            manifest.subject,
                            receipt.clone(),
                            ownership,
                        )
                        .map_err(runtime_enqueue_error)
                        .map_err(|error| self.close(error, services))?;
                }
            }
            Some(StoreConsumer::LocalProposal { .. }) => {}
            None => {}
        }
        let projected_local_admission = if coalesces_local_store_replay {
            debug_assert!(coalesces_published_store_terminal);
            let removed = self.local_store_replay.remove(&completion.work_id());
            debug_assert!(removed.is_some());
            None
        } else if let Some((store_effect, validate_effect, validate_ownership, store_terminal)) =
            local_replay_projection
        {
            let replay = self
                .local_store_replay
                .remove(&completion.work_id())
                .expect("preflighted local Store replay authority remains installed");
            let validate_replay = match replay.project_exact_validate(
                &store_effect,
                &manifest,
                &receipt,
                &validate_effect,
                &validate_ownership,
            ) {
                Ok(replay) => replay,
                Err(replay) => {
                    let Entry::Vacant(slot) = self.local_store_replay.entry(completion.work_id())
                    else {
                        unreachable!(
                            "the exact Store replay entry was removed immediately before restoration"
                        )
                    };
                    slot.insert(replay);
                    return Err(self.close(
                        EffectExecutorError::Contract(
                            "preflighted local Validate replay projection changed before commit"
                                .to_owned(),
                        ),
                        services,
                    ));
                }
            };
            let prepared = PreparedLocalBodyValidateReplayPreAdmission::seal_exact_validate(
                validate_effect.clone(),
                validate_ownership.clone(),
                receipt.clone(),
                validate_replay,
            )
            .map_err(|_| {
                self.close(
                    EffectExecutorError::Contract(
                        "local Store completion could not seal its exact lifecycle Validate owner"
                            .to_owned(),
                    ),
                    services,
                )
            })?;
            Some((
                validate_effect,
                validate_ownership,
                prepared.into_pending_durable_validate_admission(),
                store_terminal,
            ))
        } else {
            None
        };
        self.pending_stores.remove(&completion.work_id());
        self.pending_store_bytes = pending_store_bytes;
        if coalesces_published_store_terminal {
            // The published lifecycle row is now the sole body-stage owner.
            // The legacy pipeline token cannot authorize any later terminal
            // and must not survive solely because its physical Store finished.
            let removed = self.body_pipeline_owners.remove(&key);
            debug_assert_eq!(removed, coalesced_pipeline_owner);
        }
        if let PublishedLifecycleStoreCompletionPlanV1::ActiveStore(projected) =
            published_store_completion_plan
        {
            *self
                .published_lifecycle_store_retry_markers
                .get_mut(&key)
                .expect("preflighted published Store marker remains serialized") = projected;
        }
        self.recovered_bodies
            .insert(key, (manifest.clone(), receipt.clone()));
        self.durable_bodies.insert(key, receipt.clone());
        if let Some((effect, ownership, pending, store_terminal)) = projected_local_admission {
            self.install_pending_durable_validate_admission(
                key,
                &effect,
                &ownership,
                pending,
                Some(store_terminal),
            )
            .map_err(|error| self.close(error, services))?;
        }
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Accepted)
    }
    /// Retry every retained Apply task waiting for one exact certified merge entry.
    ///
    /// The complete matching owner set is preflighted before callbacks. Work
    /// identifiers are reused verbatim, and deferred entries are removed only
    /// after every enqueue succeeds.
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
        let plans = work_ids
            .iter()
            .map(|work_id| {
                let pending = self.pending_applications.get(work_id).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "deferred merge sidecar has no pending Apply task".to_owned(),
                    )
                })?;
                self.preflight_pending_application_owner(*work_id, pending)?;
                Ok(pending.task.clone())
            })
            .collect::<Result<Vec<_>, EffectExecutorError>>()
            .map_err(|error| self.close(error, services))?;
        for task in plans {
            if let Err(error) = services.enqueue_apply(task) {
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
    /// Fail closed when a decided Apply references a uniquely invalid merge entry.
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
            let pending = self.pending_applications.get(work_id).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "deferred merge sidecar has no pending Apply task".to_owned(),
                )
            });
            let pending = match pending {
                Ok(pending) => pending,
                Err(error) => return Err(self.close(error, services)),
            };
            if let Err(error) = self.preflight_pending_application_owner(*work_id, pending) {
                return Err(self.close(error, services));
            }
        }
        let Some(pending) = work_ids
            .iter()
            .find_map(|work_id| self.pending_applications.get(work_id))
        else {
            return Ok(0);
        };
        let certificate = pending.task.certificate().clone();
        let subject = pending.task.subject();
        if let Err(error) = services.report_invalid_certified_body(subject, certificate) {
            return Err(self.close(service_error(error), services));
        }
        Err(self.close(
            EffectExecutorError::BodyStore(format!(
                "decided body references an invalid certified merge sidecar: {}",
                reason.into()
            )),
            services,
        ))
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
        let round = pending.task.validated_receipt().durable().round();
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
    /// Fail closed when a decided Apply cannot register its exact merge sidecar.
    pub(crate) fn reject_deferred_merge_sidecar_work<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        _reason: impl Into<String>,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        if !self.deferred_merge_work.contains_key(&work_id) {
            return Ok(CompletionDisposition::Stale);
        }
        let pending = self.pending_applications.get(&work_id).ok_or_else(|| {
            EffectExecutorError::Contract(
                "deferred merge sidecar has no pending Apply task".to_owned(),
            )
        });
        let pending = match pending {
            Ok(pending) => pending,
            Err(error) => return Err(self.close(error, services)),
        };
        if let Err(error) = self.preflight_pending_application_owner(work_id, pending) {
            return Err(self.close(error, services));
        }
        Err(self.close(
            EffectExecutorError::BodyStore(
                "decided Apply task could not register its certified merge sidecar".to_owned(),
            ),
            services,
        ))
    }
    /// Fail closed when the asynchronous body-store worker cannot complete a pending task.
    #[cfg(test)]
    pub(crate) fn body_service_failed<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        reason: impl fmt::Display,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        if !self.pending_stores.contains_key(&work_id) {
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
    fn has_exact_manifest_chunk_fetch(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
        token: &FairV2IngressLeaderWireToken,
    ) -> bool {
        self.pending_fetches.values().any(|pending| {
            pending.task.manifest.as_ref().is_some_and(|manifest| {
                HashOf::new(manifest) == manifest_hash
                    && token.matches_exact_body(manifest.round, manifest.subject, manifest_hash)
            })
        })
    }
    /// Keep a productive chunk in durable fair ingress until an exact
    /// manifest-bearing fetch can authenticate it or retained body state can
    /// terminalize it without orphan storage.
    fn payload_chunk_ingress_can_drain(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        if !ingress_ownership.validate_exact()
            || ingress_ownership.leader_wire_runtime_receipt().is_some()
        {
            // Corrupt ownership must reach the existing mutating fail-closed
            // seam instead of becoming an immortal queue head.
            return true;
        }
        let Some(token) = ingress_ownership.leader_wire_token() else {
            // Proofless reordering remains bounded by the ejectable orphan
            // partition; it carries no protected runtime lifecycle.
            return true;
        };
        if !token.matches_chunk_manifest(manifest_hash) {
            return true;
        }
        match self.classify_payload_chunk_lifecycle_for_token(manifest_hash, token) {
            Ok(
                PayloadChunkLifecycleDisposition::Durable(_)
                | PayloadChunkLifecycleDisposition::Volatile,
            ) => true,
            Ok(PayloadChunkLifecycleDisposition::Retain) => {
                self.has_exact_manifest_chunk_fetch(manifest_hash, token)
            }
            // Internal disagreement must reach the existing fail-closed
            // classifier rather than permanently pinning ingress.
            Err(_) => true,
        }
    }
    /// Classify one exact productive chunk after the service has established
    /// that no active manifest fetch can consume it immediately.
    ///
    /// The executor is the sole authority for current-view and protected-lock
    /// ownership. Durable receipts take precedence over process-local stages;
    /// otherwise exact retained bytes make a chunk volatile-terminal. A
    /// strictly older unprotected chunk is obsolete even when its body was
    /// never retained. Current, future, protected, and still-fetching bodies
    /// remain retryable.
    pub(crate) fn classify_payload_chunk_lifecycle(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> Result<PayloadChunkLifecycleDisposition, EffectExecutorError> {
        if !ingress_ownership.validate_exact() {
            return Err(EffectExecutorError::Contract(
                "payload chunk lifecycle classification received invalid ingress ownership"
                    .to_owned(),
            ));
        }
        let runtime = ingress_ownership
            .leader_wire_runtime_receipt()
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "productive payload chunk lost its leader-wire runtime receipt".to_owned(),
                )
            })?;
        let token = runtime.token();
        if !token.matches_chunk_manifest(manifest_hash) {
            return Err(EffectExecutorError::Contract(
                "payload chunk lifecycle classification changed its exact manifest".to_owned(),
            ));
        }
        self.classify_payload_chunk_lifecycle_for_token(manifest_hash, token)
    }
    fn classify_payload_chunk_lifecycle_for_token(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
        token: &FairV2IngressLeaderWireToken,
    ) -> Result<PayloadChunkLifecycleDisposition, EffectExecutorError> {
        let mut durable = None::<DurableBodyReceipt>;
        let mut observe_durable =
            |candidate: &DurableBodyReceipt| -> Result<(), EffectExecutorError> {
                if !token.matches_exact_body(
                    candidate.round(),
                    candidate.subject(),
                    candidate.manifest_hash(),
                ) {
                    return Ok(());
                }
                if let Some(existing) = durable.as_ref()
                    && existing != candidate
                {
                    return Err(EffectExecutorError::Contract(
                        "payload chunk matched conflicting durable body receipts".to_owned(),
                    ));
                }
                durable.get_or_insert_with(|| candidate.clone());
                Ok(())
            };
        // Recovery receipts are authoritative before FetchBody promotes them
        // into the live durable catalogue. Later pipeline stages also carry
        // the same non-forgeable receipt and therefore provide stable terminal
        // evidence even if an internal catalogue projection is temporarily
        // sparse.
        for (manifest, receipt) in self.recovered_bodies.values() {
            if token.matches_exact_body(receipt.round(), receipt.subject(), receipt.manifest_hash())
                && (manifest.round != receipt.round()
                    || manifest.subject != receipt.subject()
                    || HashOf::new(manifest) != receipt.manifest_hash())
            {
                return Err(EffectExecutorError::Contract(
                    "recovered payload body differs from its durable receipt".to_owned(),
                ));
            }
            observe_durable(receipt)?;
        }
        for receipt in self.durable_bodies.values() {
            observe_durable(receipt)?;
        }
        for receipt in self.validated_bodies.values() {
            observe_durable(receipt.durable())?;
        }
        for receipt in self.rejected_bodies.values() {
            observe_durable(receipt)?;
        }
        for pending in self.pending_applications.values() {
            observe_durable(pending.task.validated_receipt.durable())?;
        }
        if let Some(receipt) = durable {
            return Ok(PayloadChunkLifecycleDisposition::Durable(receipt));
        }
        let exact_ready = self.ready_bodies.values().any(|ready| {
            token.matches_exact_body(
                ready.manifest.round,
                ready.manifest.subject,
                HashOf::new(&ready.manifest),
            )
        });
        let exact_store = self.pending_stores.values().any(|pending| {
            token.matches_exact_body(
                pending.task.manifest.round,
                pending.task.manifest.subject,
                HashOf::new(&pending.task.manifest),
            )
        });
        if exact_ready || exact_store {
            return Ok(PayloadChunkLifecycleDisposition::Volatile);
        }
        // A pipeline owner can also denote acquisition which has not produced
        // bytes yet. Preserve that case for the active-fetch replay below;
        // never turn ownership alone into evidence that the bytes exist.
        let exact_pending_fetch = self.pending_fetches.values().any(|pending| {
            token.matches_body_coordinates(pending.task.round, pending.task.subject)
                && pending
                    .task
                    .manifest
                    .as_ref()
                    .is_none_or(|manifest| HashOf::new(manifest) == manifest_hash)
        });
        if exact_pending_fetch {
            return Ok(PayloadChunkLifecycleDisposition::Retain);
        }
        let exact_pipeline_owner = self.body_pipeline_owners.iter().any(|(key, owner)| {
            token.matches_body_coordinates(key.0, key.1)
                && owner
                    .manifest_hash
                    .is_some_and(|owner_hash| owner_hash == manifest_hash)
        });
        if exact_pipeline_owner {
            return Err(EffectExecutorError::Contract(
                "payload chunk pipeline owner has no exact retained body stage".to_owned(),
            ));
        }
        let installed_protected = self
            .protected_lock
            .is_some_and(|key| token.matches_body_coordinates(key.0, key.1));
        // The serialized runtime publishes its new tag before the executor
        // dispatches the retained EnterView effect. Preserve that effect's
        // exact protected body across this bounded pre-dispatch seam without
        // duplicating protection state in the service.
        let mut retained_protected = None;
        for batch in [
            self.retained_effect_batch.as_ref(),
            self.parked_effect_batch.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            for owned in &batch.effects {
                let AdapterEffect::EnterView { protected_lock, .. } = &owned.effect else {
                    continue;
                };
                let Some(candidate) = protected_lock_body(protected_lock.as_ref()) else {
                    continue;
                };
                if retained_protected.is_some_and(|existing| existing != candidate) {
                    return Err(EffectExecutorError::Contract(
                        "retained EnterView effects claimed conflicting protected bodies"
                            .to_owned(),
                    ));
                }
                retained_protected = Some(candidate);
            }
        }
        let pending_protected =
            retained_protected.is_some_and(|key| token.matches_body_coordinates(key.0, key.1));
        let is_older_view = self
            .runtime
            .authoritative_tag()
            .is_some_and(|tag| token.view() < tag.view());
        if is_older_view && !installed_protected && !pending_protected {
            return Ok(PayloadChunkLifecycleDisposition::Volatile);
        }
        Ok(PayloadChunkLifecycleDisposition::Retain)
    }
    /// Authenticate a chunk before handing it to the reconstruction adapter,
    /// retaining its exact fair-ingress ownership until this consumer seam.
    pub(crate) fn accept_payload_chunk_with_ingress_ownership<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        chunk: wire::PayloadChunk,
        authenticated_sender: &PeerId,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
        services: &mut S,
    ) -> Result<(), EffectTransportError> {
        let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone()),
        ));
        if !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(&message)
            || !ingress_ownership.matches_semantic_origin(authenticated_sender)
        {
            return Err(self.fail_closed_transport(
                "payload chunk lost or altered its fair-ingress ownership",
                services,
            ));
        }
        self.accept_payload_chunk_inner(work_id, chunk, authenticated_sender, services)
    }
    /// Test-only direct chunk helper. Production must preserve the ownership
    /// carrier produced by fair authenticated ingress.
    #[cfg(test)]
    pub(crate) fn accept_payload_chunk<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        chunk: wire::PayloadChunk,
        authenticated_sender: &PeerId,
        services: &mut S,
    ) -> Result<(), EffectTransportError> {
        self.accept_payload_chunk_inner(work_id, chunk, authenticated_sender, services)
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
        let _retirement = self
            .plan_pending_fetch_retirement(&pending)
            .map_err(|error| self.fail_closed_transport(error, services))?;
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
        // Reset the rejected reconstruction attempt without retiring its
        // authenticated Proposal lineage. The reducer still records the body
        // as Missing, so its next periodic Fetch is only a rediscovery and
        // cannot mint a replacement replay owner. Re-enqueueing the same task
        // is the service's idempotent retry seam: it opens a clean chunk
        // session while preserving the exact work ID, lifecycle owner,
        // Proposal replay, and any certified-request upgrade.
        if let Err(error) = services.enqueue_body_fetch(pending.task) {
            return Err(self.fail_closed_transport(error, services));
        }
        Ok(CompletionDisposition::Rejected)
    }
    /// Authenticate and bind one certified response without claiming or
    /// reserving any executor, runtime, tracker, or service state.
    ///
    /// `PreflightRequired` is intentionally weaker than claimed-response
    /// priority: the composite selector transaction must still plan the exact
    /// fetch completion, acquire or coalesce the family claim, and reserve the
    /// runtime and body-fetch service handoff. No unavailable capacity is
    /// represented here by a boolean, zero, or default value.
    // The one-cut composite lifecycle selector consumes this probe while it
    // owns the matching runtime and service reservations.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn probe_certified_response_priority(
        &self,
        response: &wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> Result<CertifiedResponsePriorityProbe, EffectTransportError> {
        self.validate_lifecycle_ingress_selector_authority()?;
        let request_hash = response.request_hash;
        if let Some(key) = self
            .recovered_decision_fetch_by_request
            .get(&request_hash)
            .copied()
        {
            if self.certified_work.contains_key(&request_hash)
                || self.outstanding_requests.contains(request_hash)
            {
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(request_hash),
                ));
            }
            let owner = self.recovered_decision_fetches.get(&key).ok_or(
                EffectTransportError::Authentication(V2TransportError::InconsistentRequestIndex(
                    request_hash,
                )),
            )?;
            if owner.request_hash() != request_hash
                || !owner.validates_exact_executor_context(&self.context, &self.requester)
            {
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(request_hash),
                ));
            }
            let authenticated = owner
                .authenticate_response(&self.context, response.clone(), authenticated_responder)
                .map_err(EffectTransportError::Authentication)?;
            let authenticated_response = authenticated.response();
            let projection = owner.candidate_projection();
            let ready_body = ReadyBody::derive(
                &self.context,
                projection.round,
                projection.subject,
                authenticated_response.body.as_slice(),
            )
            .map_err(|_| {
                EffectTransportError::BodyMismatch(
                    "recovered certified body cannot reproduce its canonical chunk manifest",
                )
            })?;
            if ready_body.manifest != authenticated_response.manifest {
                return Err(EffectTransportError::BodyMismatch(
                    "recovered certified response manifest is not canonical for its body",
                ));
            }
            let response_hash = HashOf::new(authenticated_response);
            let canonical_manifest_hash = HashOf::new(&ready_body.manifest);
            let body_payload_hash = Hash::new(&authenticated_response.body);
            let claim_preflight = match projection.response_claim {
                None => CertifiedBodyResponseClaimPreflight::Vacant,
                Some(claimed) if claimed == response_hash => {
                    CertifiedBodyResponseClaimPreflight::ExactRetransmission
                }
                Some(claimed) => {
                    return Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(
                        CertifiedResponsePriorityNonPriority::ConflictingFamilyClaim {
                            request_hash,
                            claimed_response_hash: claimed,
                            incoming_response_hash: response_hash,
                        },
                    ));
                }
            };
            return Ok(CertifiedResponsePriorityProbe::RecoveredPreflightRequired(
                Box::new(RecoveredDecisionFetchResponseCandidateV1 {
                    context_id: self.context.id(),
                    height: self.context.height,
                    key,
                    request_hash,
                    response_hash,
                    authenticated_responder: authenticated_responder.clone(),
                    authenticated_response: authenticated,
                    fetch_tag: projection.tag,
                    round: projection.round,
                    subject: projection.subject,
                    canonical_manifest_hash,
                    body_payload_hash,
                    claim_preflight,
                }),
            ));
        }
        let Some(work_id) = self.certified_work.get(&request_hash).copied() else {
            let reverse_pending_owner = self.pending_fetches.values().any(|pending| {
                pending.request_hash == Some(request_hash)
                    || pending.task.certified_request().map(HashOf::new) == Some(request_hash)
            });
            if self.outstanding_requests.contains(request_hash) || reverse_pending_owner {
                return Err(EffectTransportError::Authentication(
                    V2TransportError::InconsistentRequestIndex(request_hash),
                ));
            }
            return Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(
                CertifiedResponsePriorityNonPriority::Unsolicited { request_hash },
            ));
        };
        let pending = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?;
        if pending.task.id() != work_id
            || pending.request_hash != Some(request_hash)
            || pending.task.certified_request().map(HashOf::new) != Some(request_hash)
        {
            return Err(EffectTransportError::Authentication(
                V2TransportError::InconsistentRequestIndex(request_hash),
            ));
        }
        let pending_effect = pending.task.adapter_effect();
        let pending_effect_binding = pending
            .task
            .ownership()
            .exact_pending_adapter_effect_binding(&pending_effect)
            .map_err(|_| {
                EffectTransportError::Authentication(V2TransportError::InconsistentRequestIndex(
                    request_hash,
                ))
            })?;
        if !pending
            .task
            .matches_reconstructed_manifest(&response.manifest)
        {
            return Err(EffectTransportError::BodyMismatch(
                "certified response manifest differs from proposal authority",
            ));
        }
        let authenticated = self
            .outstanding_requests
            .authenticate_response(&self.context, response.clone(), authenticated_responder)
            .map_err(EffectTransportError::Authentication)?;
        let authenticated_response = authenticated.response();
        let ready_body = ReadyBody::derive(
            &self.context,
            pending.task.round,
            pending.task.subject,
            authenticated_response.body.as_slice(),
        )
        .map_err(|_| {
            EffectTransportError::BodyMismatch(
                "certified body cannot reproduce its canonical chunk manifest",
            )
        })?;
        if ready_body.manifest != authenticated_response.manifest {
            return Err(EffectTransportError::BodyMismatch(
                "certified response manifest is not canonical for its body",
            ));
        }
        let claim_preflight = match self
            .outstanding_requests
            .preflight_authenticated_response_claim(&authenticated)
        {
            Ok(preflight) => preflight,
            Err(V2TransportError::ConflictingCertifiedBodyResponseClaim {
                request,
                claimed,
                incoming,
            }) => {
                return Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(
                    CertifiedResponsePriorityNonPriority::ConflictingFamilyClaim {
                        request_hash: request,
                        claimed_response_hash: claimed,
                        incoming_response_hash: incoming,
                    },
                ));
            }
            Err(error) => return Err(EffectTransportError::Authentication(error)),
        };
        Ok(CertifiedResponsePriorityProbe::PreflightRequired(Box::new(
            CertifiedResponsePriorityCandidate {
                context_id: self.context.id(),
                height: self.context.height,
                request_hash,
                response_hash: HashOf::new(authenticated_response),
                authenticated_responder: authenticated_responder.clone(),
                work_id,
                fetch_tag: pending.task.tag,
                round: pending.task.round,
                subject: pending.task.subject,
                proposal_manifest_hash: pending.task.manifest.as_ref().map(HashOf::new),
                pending_effect_binding,
                canonical_manifest_hash: HashOf::new(&ready_body.manifest),
                body_payload_hash: Hash::new(&authenticated_response.body),
                claim_preflight,
                authenticated_response: authenticated,
            },
        )))
    }
    /// Re-probe one opaque response candidate and require exact equality.
    ///
    /// This is still read-only preparation: equality neither claims the
    /// request family nor reserves runtime, service, or fair-ingress capacity.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn revalidate_certified_response_priority_candidate(
        &self,
        expected: &CertifiedResponsePriorityCandidate,
        response: &wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> Result<bool, EffectTransportError> {
        match self.probe_certified_response_priority(response, authenticated_responder)? {
            CertifiedResponsePriorityProbe::DefinitelyNonPriority(_) => Ok(false),
            CertifiedResponsePriorityProbe::PreflightRequired(actual) => {
                Ok(actual.as_ref() == expected)
            }
            CertifiedResponsePriorityProbe::RecoveredPreflightRequired(_) => Ok(false),
        }
    }
    /// Re-probe one dedicated recovered response candidate and require exact equality.
    pub(in crate::sumeragi) fn revalidate_recovered_decision_fetch_response_candidate(
        &self,
        expected: &RecoveredDecisionFetchResponseCandidateV1,
        response: &wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> Result<bool, EffectTransportError> {
        match self.probe_certified_response_priority(response, authenticated_responder)? {
            CertifiedResponsePriorityProbe::RecoveredPreflightRequired(actual) => {
                Ok(actual.as_ref() == expected)
            }
            CertifiedResponsePriorityProbe::DefinitelyNonPriority(_)
            | CertifiedResponsePriorityProbe::PreflightRequired(_) => Ok(false),
        }
    }
    /// Preflight exact recovered request-owner retirement without mutation.
    pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_owner_retirement(
        &self,
        key: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
        response_hash: HashOf<wire::CertifiedBodyResponse>,
    ) -> Result<PreparedRecoveredDecisionFetchOwnerRetirementV1, EffectTransportError> {
        self.validate_lifecycle_ingress_selector_authority()?;
        if !self.recovered_decision_fetch_request_index_is_exact() {
            return Err(EffectTransportError::FailClosed(
                "recovered Decision Fetch request indexes are not exact".to_owned(),
            ));
        }
        let owner = self.recovered_decision_fetches.get(&key).ok_or_else(|| {
            EffectTransportError::FailClosed(
                "recovered Decision Fetch lost its dedicated request owner".to_owned(),
            )
        })?;
        let request_hash = owner.request_hash();
        if self.recovered_decision_fetch_by_request.get(&request_hash) != Some(&key)
            || !owner.matches_settlement(key, response_hash)
        {
            return Err(EffectTransportError::FailClosed(
                "recovered Decision Fetch changed its exact claimed response".to_owned(),
            ));
        }
        Ok(PreparedRecoveredDecisionFetchOwnerRetirementV1 {
            key,
            request_hash,
            response_hash,
        })
    }
    /// Infallibly remove the exact dedicated request indexes after publication.
    pub(in crate::sumeragi) fn commit_recovered_decision_fetch_owner_retirement(
        &mut self,
        prepared: PreparedRecoveredDecisionFetchOwnerRetirementV1,
    ) {
        let owner = self
            .recovered_decision_fetches
            .remove(&prepared.key)
            .expect("published recovered Decision Store retains its request owner");
        assert_eq!(owner.request_hash(), prepared.request_hash);
        assert!(owner.matches_settlement(prepared.key, prepared.response_hash));
        let reverse = self
            .recovered_decision_fetch_by_request
            .remove(&prepared.request_hash)
            .expect("published recovered Decision Store retains its request reverse index");
        assert_eq!(reverse, prepared.key);
    }
    /// Prepare retirement of the exact executor and request owners which move
    /// into one coordinator-owned certified-Fetch completion.
    ///
    /// This read-only plan deliberately does not reserve `BodyAvailable`,
    /// enqueue a runtime command, or allocate a lifecycle ordinal. The durable
    /// body receipt and queue identity remain sealed in the lifecycle modules.
    pub(in crate::sumeragi) fn prepare_lifecycle_certified_fetch_completion(
        &self,
        candidate: &CertifiedResponsePriorityCandidate,
        authenticated: &AuthenticatedCertifiedBodyResponse,
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<PreparedLifecycleCertifiedFetchCompletion, EffectTransportError> {
        self.validate_lifecycle_ingress_selector_authority()?;
        let response = authenticated.response();
        if !candidate.matches_authenticated_response(response, &candidate.authenticated_responder)
            || candidate.response_hash != HashOf::new(response)
        {
            return Err(EffectTransportError::BodyMismatch(
                "persisted response differs from fresh selector authority",
            ));
        }
        let work_id = candidate.work_id;
        let pending = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?;
        if pending.task.id() != work_id
            || pending.request_hash != Some(candidate.request_hash)
            || pending.task.certified_request().map(HashOf::new) != Some(candidate.request_hash)
            || pending.task.round != candidate.round
            || pending.task.subject != candidate.subject
            || !pending
                .task
                .matches_reconstructed_manifest(&response.manifest)
        {
            return Err(EffectTransportError::BodyMismatch(
                "fresh selector differs from exact pending certified Fetch",
            ));
        }
        let effect = pending.task.adapter_effect();
        let binding = pending
            .task
            .ownership()
            .exact_pending_adapter_effect_binding(&effect)
            .map_err(|_| {
                EffectTransportError::FailClosed(
                    "pending certified Fetch lost its exact effect binding".to_owned(),
                )
            })?;
        if &binding != candidate.pending_effect_binding() {
            return Err(EffectTransportError::BodyMismatch(
                "fresh selector changed the pending Fetch binding",
            ));
        }
        let key = (pending.task.round, pending.task.subject);
        if durable_receipt.context_id() != self.context.id()
            || durable_receipt.round() != key.0
            || durable_receipt.subject() != key.1
            || durable_receipt.manifest_hash() != HashOf::new(&response.manifest)
        {
            return Err(EffectTransportError::BodyMismatch(
                "persisted certified body receipt differs from its exact response manifest",
            ));
        }
        if self.ready_bodies.contains_key(&key)
            || self.durable_bodies.contains_key(&key)
            || self.recovered_bodies.contains_key(&key)
            || self.validated_bodies.contains_key(&key)
            || self.rejected_bodies.contains_key(&key)
        {
            return Err(EffectTransportError::FailClosed(
                "pending certified Fetch overlaps a later executor body stage".to_owned(),
            ));
        }
        let body_pipeline_owner =
            self.body_pipeline_owners
                .get(&key)
                .copied()
                .ok_or_else(|| {
                    EffectTransportError::FailClosed(
                        "pending certified Fetch lost its body-pipeline owner".to_owned(),
                    )
                })?;
        // A certified-only Fetch legitimately starts without proposal manifest
        // metadata. Its authenticated response supplies the canonical manifest at
        // this atomic retirement-to-durable-body transition; until then the exact
        // pipeline owner must still match the frozen pending Fetch, including None.
        if body_pipeline_owner.tag != candidate.fetch_tag()
            || body_pipeline_owner.manifest_hash != candidate.proposal_manifest_hash()
        {
            return Err(EffectTransportError::BodyMismatch(
                "pending certified Fetch differs from the exact body-pipeline owner",
            ));
        }
        let claim_preflight = self
            .outstanding_requests
            .preflight_authenticated_response_claim(authenticated)
            .map_err(EffectTransportError::Authentication)?;
        if &claim_preflight != candidate.claim_preflight() {
            return Err(EffectTransportError::BodyMismatch(
                "response-family claim changed after fresh selector capture",
            ));
        }
        let certified = self
            .plan_certified_fetch_retirement(work_id, candidate.request_hash)
            .map_err(|error| EffectTransportError::FailClosed(error.to_string()))?;
        Ok(PreparedLifecycleCertifiedFetchCompletion {
            pending: pending.clone(),
            certified,
            body_pipeline_key: key,
            body_pipeline_owner,
            manifest: response.manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            response_hash: candidate.response_hash,
            claim_preflight,
        })
    }
    /// Infallibly retire one preflighted executor owner after exact dequeue.
    ///
    /// Every assertion is inside the caller's fail-stop output operation. A
    /// violated assertion therefore closes process output rather than exposing
    /// a retry after the physical carrier was consumed.
    pub(in crate::sumeragi) fn commit_lifecycle_certified_fetch_completion(
        &mut self,
        prepared: PreparedLifecycleCertifiedFetchCompletion,
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) {
        let work_id = prepared.pending.task.id();
        assert_eq!(self.pending_fetches.get(&work_id), Some(&prepared.pending));
        assert_eq!(
            self.body_pipeline_owners.get(&prepared.body_pipeline_key),
            Some(&prepared.body_pipeline_owner)
        );
        assert_eq!(
            HashOf::new(authenticated.response()),
            prepared.response_hash
        );
        assert_eq!(authenticated.response().manifest, prepared.manifest);
        assert_eq!(prepared.durable_receipt.context_id(), self.context.id());
        assert_eq!(
            prepared.durable_receipt.round(),
            prepared.body_pipeline_key.0
        );
        assert_eq!(
            prepared.durable_receipt.subject(),
            prepared.body_pipeline_key.1
        );
        assert_eq!(
            prepared.durable_receipt.manifest_hash(),
            HashOf::new(&prepared.manifest)
        );
        assert_eq!(
            self.outstanding_requests
                .preflight_authenticated_response_claim(authenticated)
                .expect("preflighted response family remains outstanding"),
            prepared.claim_preflight
        );
        let claim = self
            .outstanding_requests
            .prepare_authenticated_response_claim(authenticated)
            .expect("exclusive executor retains the preflighted response family");
        let _disposition = claim.commit();
        let removed = self
            .pending_fetches
            .remove(&work_id)
            .expect("preflighted pending Fetch remains installed");
        assert_eq!(removed, prepared.pending);
        self.commit_certified_fetch_retirement(prepared.certified);
        let removed_owner = self
            .body_pipeline_owners
            .remove(&prepared.body_pipeline_key)
            .expect("preflighted body-pipeline owner remains installed");
        assert_eq!(removed_owner, prepared.body_pipeline_owner);
        let previous = self.recovered_bodies.insert(
            prepared.body_pipeline_key,
            (prepared.manifest, prepared.durable_receipt.clone()),
        );
        assert!(previous.is_none());
        let previous = self
            .durable_bodies
            .insert(prepared.body_pipeline_key, prepared.durable_receipt);
        assert!(previous.is_none());
    }
    /// Accept a durable application completion only when its typed Kura receipt
    /// and canonical finality artifact exactly match the Apply effect.
    pub(crate) fn complete_application<S: V2EffectServices>(
        &mut self,
        completion: DurableApplyCompletion,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        if !self.pending_applications.contains_key(&completion.work_id) {
            return Ok(CompletionDisposition::Stale);
        }
        if let Err(error) = {
            let pending = self
                .pending_applications
                .get(&completion.work_id)
                .expect("the pending Apply was checked above");
            self.preflight_pending_application_owner(completion.work_id, pending)
        } {
            return Err(self.close(error, services));
        }
        let pending = self
            .pending_applications
            .get(&completion.work_id)
            .expect("the owner preflight cannot remove the pending Apply");
        let task = &pending.task;
        let valid_artifact = completion.artifact().validate().is_ok()
            && completion.artifact().height_context == self.context
            && completion.artifact().subject == task.subject
            && completion.artifact().commit_qc == task.certificate;
        let valid_receipt = completion.receipt().height() == self.context.height
            && completion.receipt().context_id() == self.context.id()
            && completion.receipt().block_hash() == task.subject.block_hash
            && completion.receipt().subject() == task.subject
            && completion.receipt().certificate() == task.certificate.as_ref()
            && completion.receipt().artifact_hash() == HashOf::new(completion.artifact());
        let valid_recovery_stage = self.pending_tip_recovery.as_ref().is_none_or(|evidence| {
            evidence.is_exact(&self.context)
                && evidence.stage() == PendingKuraApplyRecoveryStage::ApplicationDispatched
                && task.tag == evidence.replay_tag()
                && task.subject == evidence.commit_subject()
                && &task.certificate == evidence.commit_qc()
                && &task.validated_receipt == evidence.validated_receipt()
        });
        if !valid_artifact
            || !valid_receipt
            || !valid_recovery_stage
            || self.finality_completion.is_some()
            || !self.recovered_decision_fetch_request_index_is_exact_and_empty()
        {
            return Err(self.close(EffectExecutorError::InvalidApplyCompletion, services));
        }
        let tag = task.tag;
        let subject = task.subject;
        let ownership = pending.ownership.clone();
        if let Err(error) = self
            .runtime
            .enqueue_application_completed_with_owner(tag, subject, &ownership)
        {
            return Err(self.close(runtime_enqueue_error(error), services));
        }
        self.pending_applications.remove(&completion.work_id);
        self.deferred_merge_work.remove(&completion.work_id);
        if let Some(evidence) = self.pending_tip_recovery.as_mut() {
            evidence.stage = PendingKuraApplyRecoveryStage::Completed;
            self.pending_tip_recovery_last_result =
                Some(PendingTipRecoveryAttemptResult::Completed);
        }
        self.finality_completion = Some(FinalityCompletion {
            tag,
            receipt: completion.receipt,
            artifact: completion.artifact,
            ownership: FinalityCompletionOwner::Runtime(ownership),
        });
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Accepted)
    }
    /// Current bounded operational status.
    pub(crate) fn status(&self) -> EffectExecutorStatus {
        let restart_required = self.output_guard.restart_required();
        let captured_at = Instant::now();
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
            pending_tip_recovery_stage: self
                .pending_tip_recovery
                .as_ref()
                .map(PendingKuraApplyRecoveryEvidence::stage),
            pending_tip_recovery_attempts: self.pending_tip_recovery_attempts,
            pending_tip_recovery_last_result: self.pending_tip_recovery_last_result,
            pending_signatures: self
                .pending_signatures
                .len()
                .saturating_add(usize::from(self.pending_live_wal_sign_admission.is_some())),
            // The production service overlays its height-local disk acquisition
            // ownership when this executor snapshot crosses that boundary.
            pending_candidate_loads: 0,
            pending_fetches: self.pending_fetches.len(),
            pending_stores: self.pending_stores.len(),
            pending_validations: self
                .pending_durable_validate_admissions
                .len()
                .saturating_add(usize::from(
                    self.pending_released_lifecycle_validate_apply.is_some(),
                )),
            pending_outputs: self.pending_lifecycle_output_admissions.len(),
            deferred_application_merge_work,
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
    /// Return whether the executor still owns this exact deferred Apply dependency.
    pub(crate) fn retains_deferred_merge_sidecar(
        &self,
        work_id: EffectWorkId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        entry_hash: HashOf<MergeLedgerEntry>,
    ) -> bool {
        self.deferred_merge_work.get(&work_id) == Some(&entry_hash)
            && self
                .pending_applications
                .get(&work_id)
                .is_some_and(|pending| {
                    pending.task.validated_receipt().durable().round() == round
                        && pending.task.subject() == subject
                })
    }
    /// Return whether this retained missing-sidecar dependency belongs to the
    /// uniquely decided Apply task rather than speculative validation work.
    pub(crate) fn deferred_merge_sidecar_is_decided(&self, work_id: EffectWorkId) -> bool {
        self.deferred_merge_work.contains_key(&work_id)
            && self.pending_applications.contains_key(&work_id)
    }
    /// Borrow the durable finality values returned by Kura after application.
    pub(crate) fn durable_finality(
        &self,
    ) -> Option<(&KuraV2CommitReceipt, &wire::finality::V2FinalityArtifact)> {
        self.finality_completion
            .as_ref()
            .map(|completion| (&completion.receipt, &completion.artifact))
    }
    /// Capture the exact global application-mode rank at the executor owner.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn lifecycle_mode_rank_snapshot(&self) -> LifecycleModeRankSnapshot {
        LifecycleModeRankSnapshot {
            context_id: self.context.id(),
            height: self.context.height,
            debt: u64::from(self.finality_completion.is_none()),
        }
    }
    /// Return the complete recovered retry key set without exposing owners.
    #[cfg(test)]
    pub(in crate::sumeragi) fn recovered_durable_validate_retry_keys_for_test(
        &self,
    ) -> Vec<(wire::ConsensusRound, wire::BlockSubject)> {
        self.durable_validate_retry_seals
            .iter()
            .filter_map(|(key, seal)| {
                matches!(seal, DurableValidateRetrySealV1::Recovered { .. }).then_some(*key)
            })
            .collect()
    }

    /// Inspect whether one exact retry authority exists and still owns a lifecycle row.
    #[cfg(test)]
    pub(in crate::sumeragi) fn validate_retry_lifecycle_ordinal_for_test(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
    ) -> Option<Option<u128>> {
        match (
            self.durable_validate_retry_seals.get(&key),
            self.published_lifecycle_validate_retry_markers.get(&key),
        ) {
            (Some(seal), None) => Some(seal.lifecycle_ordinal()),
            (None, Some(marker)) => Some(marker.lifecycle_ordinal),
            (None, None) | (Some(_), Some(_)) => None,
        }
    }

    /// Drive the production Decision cleanup over recovered retry seals in tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn reconcile_recovered_validate_retry_decision_for_test<
        S: V2EffectServices,
    >(
        &mut self,
        decision: DurableDecision,
        drain_decision_body: bool,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.reconcile_decision_work(decision, drain_decision_body, services)
    }

    /// Capture only stable recovered-owner and monotonic-frontier identity.
    #[cfg(test)]
    pub(in crate::sumeragi) fn recovered_durable_validate_retry_snapshot_for_test(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
    ) -> Option<RecoveredDurableValidateRetrySnapshotV1> {
        let seal = self.durable_validate_retry_seals.get(&key)?;
        let DurableValidateRetrySealV1::Recovered {
            owner, frontier, ..
        } = seal
        else {
            return None;
        };
        Some(RecoveredDurableValidateRetrySnapshotV1 {
            owner_identity: Arc::as_ptr(owner) as usize,
            causal_lifecycle_key: owner.causal_lifecycle_key_for_test(),
            effect_tag: frontier.effect_tag_for_test(),
            phase: frontier.phase_for_test(),
            commitment_ceiling: frontier.commitment_ceiling_for_test(),
        })
    }

    /// Root of the latest independently authenticated retry trace consumed as
    /// a recovered-seal stutter.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn last_recovered_validate_retry_trace_root_for_test(
        &self,
    ) -> Option<Hash> {
        self.last_recovered_validate_retry_trace_root
    }

    /// Actor-global position of the latest independently authenticated retry trace.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn last_recovered_validate_retry_trace_ordinal_for_test(
        &self,
    ) -> Option<u128> {
        self.last_recovered_validate_retry_trace_ordinal
    }

    /// Raw scheduler/effect classification from the latest serialized step.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn last_runtime_step_observation_for_test(
        &self,
    ) -> Option<RuntimeStepObservationV1> {
        self.last_runtime_step_observation
    }

    /// Whether no executable or retained ordinary work escaped the retry seam.
    #[cfg(test)]
    pub(in crate::sumeragi) fn recovered_validate_retry_corridor_is_inert_for_test(&self) -> bool {
        self.retained_effect_batch.is_none()
            && self.parked_effect_batch.is_none()
            && self.pending_durable_validate_admissions.is_empty()
            && self.pending_work() == 0
    }

    /// Inspect the exact live lifecycle Apply retransmit owner without
    /// exposing its retained CommitQC or validated receipt.
    #[cfg(test)]
    pub(in crate::sumeragi) fn live_lifecycle_decision_apply_key_for_test(
        &self,
    ) -> Option<LifecycleDecisionApplyDispatchKeyV1> {
        self.live_lifecycle_decision_apply
            .as_ref()
            .map(|owner| owner.dispatch_key)
    }
    /// Inspect only whether pending-Kura Apply collided with lifecycle/batch owners.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn pending_kura_apply_owner_flags_for_test(
        &self,
    ) -> (bool, bool, bool, bool, bool) {
        (
            self.live_lifecycle_decision_apply.is_some(),
            self.live_lifecycle_validate_successor.is_some(),
            self.retained_effect_batch.is_some(),
            self.parked_effect_batch.is_some(),
            self.finality_completion.is_some(),
        )
    }
    /// Route one exact reducer Apply rediscovery through the same private
    /// admission function used by a live Runtime turn.
    #[cfg(test)]
    pub(in crate::sumeragi) fn coalesce_live_lifecycle_apply_retransmit_for_test<
        S: V2EffectServices,
    >(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let ordinal = self
            .live_lifecycle_decision_apply
            .as_ref()
            .map_or(1, |owner| owner.dispatch_key.lifecycle_ordinal());
        let effect = AdapterEffect::Apply {
            tag,
            subject,
            certificate: certificate.clone(),
        };
        let ownership = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .map_err(EffectExecutorError::Contract)?
        .pop()
        .ok_or_else(|| {
            EffectExecutorError::Contract(
                "test Apply retransmit omitted its exact effect owner".to_owned(),
            )
        })?;
        self.begin_apply(tag, subject, certificate, ownership, services)
    }
    /// Return whether a lifecycle service uses this executor's canonical output gate.
    pub(in crate::sumeragi) fn matches_lifecycle_output_guard(
        &self,
        candidate: &Arc<ConsensusOutputGuard>,
    ) -> bool {
        Arc::ptr_eq(&self.output_guard, candidate)
    }
    fn consume_one<S: V2EffectServices>(
        &mut self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
        highest_prepare_retention: Option<wire::QuorumCertificateRef>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        if highest_prepare_retention.is_some()
            && !matches!(&effect, AdapterEffect::EnterView { .. })
        {
            return Err(EffectExecutorError::Contract(
                "cleanup-only highest Prepare sidecar escaped its EnterView".to_owned(),
            ));
        }
        let mut validated_apply_successor = None;
        let recovery_transition = self
            .pending_tip_recovery
            .as_ref()
            .map(|evidence| {
                if !evidence.is_exact(&self.context) {
                    return Err(EffectExecutorError::Contract(
                        "interrupted-tip recovery evidence lost its exact native identity"
                            .to_owned(),
                    ));
                }
                evidence.transition_for_effect(&effect)
            })
            .transpose()?;
        let result = match effect {
            AdapterEffect::Sign { tag, request } => {
                if let SignRequest::Vote(vote) = &request {
                    if vote.round != vote.proposal_round {
                        return Err(EffectExecutorError::Contract(
                            "vote signing requires same-round proposal authority".to_owned(),
                        ));
                    }
                    let body_round = vote.proposal_round;
                    let validated = self
                        .validated_bodies
                        .get(&(body_round, vote.subject))
                        .ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "vote signing requires an exact fsynced validation marker"
                                    .to_owned(),
                            )
                        })?;
                    if validated.durable().context_id() != self.context.id()
                        || validated.durable().round() != body_round
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
                        ownership: ownership.clone(),
                    },
                );
                services
                    .enqueue_consensus_sign(ConsensusSignTask {
                        id,
                        tag,
                        request,
                        ownership,
                    })
                    .map_err(service_error)
            }
            effect @ AdapterEffect::Broadcast(_) => {
                self.park_lifecycle_output_admission(effect, ownership)
            }
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                manifest,
                certified_sources,
                certificate,
            } => {
                let (proposal_replay, rediscovery_stutters) = if certificate.is_none() {
                    let exact_effect = AdapterEffect::FetchBody {
                        tag,
                        round,
                        subject,
                        manifest: manifest.clone(),
                        certified_sources: certified_sources.clone(),
                        certificate: None,
                    };
                    let key = (round, subject);
                    match self.remote_proposal_replay.get(&key) {
                        Some(stage) => {
                            if !stage.exactly_authenticates_fetch_rediscovery(&exact_effect) {
                                return Err(EffectExecutorError::Contract(
                                    "ordinary Proposal FetchBody changed its retained authenticated replay origin"
                                        .to_owned(),
                                ));
                            }
                            match stage {
                                RemoteProposalReplayStageV1::Fetch { work_id, .. } => {
                                    if !self.pending_fetches.get(work_id).is_some_and(|pending| {
                                        pending.task.round == round
                                            && pending.task.subject == subject
                                    }) {
                                        return Err(EffectExecutorError::Contract(
                                            "ordinary Proposal Fetch replay lost its exact in-flight task"
                                                .to_owned(),
                                        ));
                                    }
                                    (None, false)
                                }
                                RemoteProposalReplayStageV1::StoreAdmission(_) => {
                                    return Err(EffectExecutorError::Contract(
                                        "ordinary Proposal Fetch rediscovery observed a transient Store admission"
                                            .to_owned(),
                                    ));
                                }
                                RemoteProposalReplayStageV1::BodyAvailable(_)
                                | RemoteProposalReplayStageV1::Store { .. }
                                | RemoteProposalReplayStageV1::Stored { .. } => (None, true),
                            }
                        }
                        None => (
                            Some(
                            PreparedRemoteProposalFetchReplayPreAdmission::seal_exact_fetch(
                                exact_effect,
                                ownership.clone(),
                            )
                            .map_err(|_| {
                                EffectExecutorError::Contract(
                                    "ordinary Proposal FetchBody omitted its authenticated replay owner"
                                    .to_owned(),
                                )
                            })?,
                            ),
                            false,
                        ),
                    }
                } else {
                    (None, false)
                };
                if rediscovery_stutters {
                    Ok(())
                } else {
                    self.begin_fetch(
                        tag,
                        round,
                        subject,
                        manifest,
                        certified_sources,
                        certificate,
                        ownership,
                        proposal_replay,
                        services,
                    )
                }
            }
            AdapterEffect::StoreBody {
                tag,
                round,
                subject,
            } => self.store_body(tag, round, subject, ownership, services),
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } => {
                validated_apply_successor =
                    self.validate_body(tag, round, subject, ownership, services)?;
                Ok(())
            }
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            } => self.begin_apply(tag, subject, certificate, ownership, services),
            AdapterEffect::EnterView {
                tag,
                certificate,
                protected_lock,
            } => self.install_view(
                tag,
                certificate,
                protected_lock,
                highest_prepare_retention,
                services,
            ),
            effect @ (AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. }) => {
                self.park_lifecycle_output_admission(effect, ownership)
            }
        };
        result?;
        if let Some(stage) = recovery_transition {
            self.pending_tip_recovery
                .as_mut()
                .expect("recovery transition was derived from this evidence")
                .stage = stage;
        }
        if let Some(successor) = validated_apply_successor {
            match successor {
                DirectValidatedApplySuccessorV1::PendingKura(successor) => {
                    // The direct validation commit above emitted this exact
                    // child while the outer stage still owned deterministic
                    // validation. Record Apply first, then consume only the
                    // predecessor-projected local recovery child.
                    let (effect, ownership) = successor
                        .consume_for_executor(PendingKuraApplySuccessorExecutorPermitV1::new());
                    self.ensure_pending_tip_recovery_effect_is_local(&effect)?;
                    self.consume_one(effect, ownership, None, services)
                        .map_err(|error| {
                            EffectExecutorError::Contract(format!(
                                "committed pending-Kura validation could not dispatch its exact Apply child: {error}"
                            ))
                        })?;
                }
            }
        }
        Ok(())
    }
    /// Reject any interrupted-tip recovery effect that cannot be satisfied
    /// entirely from the exact process-local recovery catalogs.
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
            AdapterEffect::ValidateBody { round, subject, .. } => self
                .validated_bodies
                .contains_key(&(*round, *subject))
                .then_some(())
                .ok_or_else(local_only_error),
            AdapterEffect::Apply {
                subject,
                certificate,
                ..
            } => self
                .pending_tip_recovery
                .as_ref()
                .filter(|evidence| {
                    evidence.commit_qc() == certificate
                        && evidence.commit_subject() == *subject
                        && self
                            .validated_bodies
                            .get(&(evidence.durable_round(), evidence.durable_subject()))
                            == Some(evidence.validated_receipt())
                })
                .map(|_| ())
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
        let ownership_trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_OWNER,
            relation_exact: plan_exact_body_owner_binding(current, incoming) == Some(binding),
            protected_before: u64::from(current.is_some_and(|owner| owner.manifest_hash.is_some())),
            protected_after: u64::from(binding.owner.manifest_hash.is_some()),
            owner_before: u64::from(current.is_some()),
            owner_after: 1,
            owner_reused: binding.already_owned,
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
        let Some(checked_effective_lock) =
            check_production_body_ownership_effective_lock_transition(ownership_trace)
        else {
            return Err(EffectExecutorError::Contract(
                "exact body ownership did not refine the effective-lock trace".to_owned(),
            ));
        };
        Ok(BodyPipelineOwnerBindingPlan {
            key,
            owner: BodyPipelineOwner {
                tag,
                manifest_hash: binding.owner.manifest_hash,
            },
            already_owned: binding.already_owned,
            checked_effective_lock,
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
        let BodyPipelineOwnerBindingPlan {
            key,
            owner,
            already_owned: _,
            checked_effective_lock,
        } = plan;
        let _authorized_ownership = checked_effective_lock.into_projection();
        self.body_pipeline_owners.insert(key, owner);
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
        ownership: RuntimeEffectOwnership,
        mut proposal_replay: Option<PreparedRemoteProposalFetchReplayPreAdmission>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let incoming_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: manifest.clone(),
            certified_sources: sources.clone(),
            certificate: certificate.clone(),
        };
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
            let expected_sources = self.frozen_archive_sources();
            if sources != expected_sources
                || certificate.validate(&self.context).is_err()
                || certificate.proposal_round != round
                || certificate.subject != subject
            {
                return Err(EffectExecutorError::Contract(
                    "certified FetchBody origin is not authorized by the verified QC and canonical frozen-roster archive sequence"
                        .to_owned(),
                ));
            }
        } else if manifest.is_none() || !sources.is_empty() {
            return Err(EffectExecutorError::Contract(
                "uncertified FetchBody requires a proposal manifest and no certified sources"
                    .to_owned(),
            ));
        }
        if self
            .recovered_decision_fetches
            .values()
            .any(|owner| owner.matches_body_coordinates(round, subject))
        {
            return Err(EffectExecutorError::Contract(
                "body-fetch coordinates already have a recovered Decision Fetch owner".to_owned(),
            ));
        }
        let key = (round, subject);
        if let Some(stage) = self.authenticated_genesis_replay.get(&key) {
            if proposal_replay.is_some()
                || !stage.exactly_authenticates_fetch_rediscovery(&incoming_effect)
            {
                return Err(EffectExecutorError::Contract(
                    "certified genesis Fetch rediscovery changed its authenticated origin"
                        .to_owned(),
                ));
            }
            if matches!(stage, AuthenticatedGenesisReplayStageV1::StoreAdmission(_)) {
                return Err(EffectExecutorError::Contract(
                    "certified genesis Fetch rediscovery observed transient Store admission"
                        .to_owned(),
                ));
            }
            return Ok(());
        }
        let existing_id = self.pending_fetches.iter().find_map(|(id, pending)| {
            (pending.task.round == round && pending.task.subject == subject).then_some(*id)
        });
        if let Some(existing_id) = existing_id {
            let existing = self
                .pending_fetches
                .get(&existing_id)
                .expect("pending fetch ID came from this map")
                .clone();
            match self.remote_proposal_replay.get(&key) {
                Some(RemoteProposalReplayStageV1::Fetch { work_id, replay })
                    if *work_id == existing_id =>
                {
                    if proposal_replay.is_some()
                        && !replay.exactly_matches_retry(&incoming_effect, &ownership)
                    {
                        return Err(EffectExecutorError::Contract(
                            "ordinary Proposal Fetch retry changed its authenticated replay owner"
                                .to_owned(),
                        ));
                    }
                    proposal_replay = None;
                }
                Some(_) => {
                    return Err(EffectExecutorError::Contract(
                        "body Fetch retry conflicts with a later Proposal replay stage".to_owned(),
                    ));
                }
                None if proposal_replay.is_some() && existing.task.certified_request.is_some() => {
                    return Err(EffectExecutorError::Contract(
                        "ordinary Proposal replay cannot attach to certified Fetch work".to_owned(),
                    ));
                }
                None => {}
            }
            let same_lifecycle = existing.task.ownership == ownership;
            if existing.task.tag != tag {
                return Err(EffectExecutorError::Contract(
                    "conflicting retransmission for one body-fetch round/subject".to_owned(),
                ));
            }
            if !same_lifecycle {
                return Err(EffectExecutorError::Contract(
                    "body-fetch retry or authority upgrade changed its exact lifecycle owner"
                        .to_owned(),
                ));
            }
            // A periodic producer can rediscover the same exact acquisition
            // while the first fetch is still live. The admission gate has
            // already required the immutable incumbent owner; the tag, round,
            // subject, manifest, and certified authority below must agree too.
            let merged_manifest = match (&existing.task.manifest, manifest) {
                (Some(existing), Some(incoming)) if existing != &incoming => {
                    return Err(EffectExecutorError::Contract(
                        "conflicting retransmission changed a body-fetch manifest".to_owned(),
                    ));
                }
                (Some(existing), _) => Some(existing.clone()),
                (None, incoming) => incoming,
            };
            let (merged_sources, merged_request, request_hash, request_plan) = if let Some(
                request,
            ) =
                existing.task.certified_request.clone()
            {
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
                let plan = match self.plan_certified_fetch_request(
                    existing_id,
                    round,
                    subject,
                    certificate,
                    services,
                ) {
                    Ok(plan) => plan,
                    Err(EffectExecutorError::CertifiedRequestCapacity { capacity }) => {
                        // The existing acquisition and this exact owned upgrade remain live.
                        // The retained effect FIFO retries the incumbent owner after request
                        // capacity changes without replacing its admission age.
                        iroha_logger::debug!(
                            height = round.height,
                            view = round.view,
                            capacity,
                            "deferred certified Sumeragi v2 body-fetch authority upgrade at request capacity"
                        );
                        return Err(EffectExecutorError::CertifiedRequestCapacity { capacity });
                    }
                    Err(error) => return Err(error),
                };
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
            let merged_effect = AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                manifest: merged_manifest.clone(),
                certified_sources: merged_sources.clone(),
                certificate: merged_request
                    .as_ref()
                    .map(|request| request.certificate.clone()),
            };
            let merged_ownership = existing
                .task
                .ownership
                .rebind_same_adapter_effect(&merged_effect)
                .map_err(EffectExecutorError::Contract)?;
            let merged = BodyFetchTask {
                id: existing_id,
                tag,
                round,
                subject,
                manifest: merged_manifest,
                sources: merged_sources,
                certified_request: merged_request,
                ownership: merged_ownership,
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
            if merged == existing.task {
                // Re-enter the idempotent service seam with the incumbent
                // task ID and owner. This gives an exact retry another fanout
                // opportunity without acquiring capacity or a second
                // completion owner.
                services.enqueue_body_fetch(merged).map_err(service_error)?;
                if let Some(replay) = proposal_replay {
                    let previous = self.remote_proposal_replay.insert(
                        key,
                        RemoteProposalReplayStageV1::Fetch {
                            work_id: existing_id,
                            replay,
                        },
                    );
                    debug_assert!(previous.is_none());
                }
                return Ok(());
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
            if let Some(replay) = proposal_replay {
                let previous = self.remote_proposal_replay.insert(
                    key,
                    RemoteProposalReplayStageV1::Fetch {
                        work_id: existing_id,
                        replay,
                    },
                );
                debug_assert!(previous.is_none());
            }
            return Ok(());
        }
        if proposal_replay.is_some() && self.remote_proposal_replay.contains_key(&key) {
            return Err(EffectExecutorError::Contract(
                "ordinary Proposal Fetch conflicts with retained replay lineage".to_owned(),
            ));
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
                // This fast path has no later fallible service boundary.
                let reservation = self
                    .runtime
                    .reserve_body_available_with_owner(tag, staged_manifest, &ownership)
                    .map_err(runtime_enqueue_error)?;
                self.runtime
                    .commit_body_available(reservation)
                    .map_err(runtime_enqueue_error)?;
                self.commit_body_pipeline_owner(owner_plan);
                self.commit_remote_proposal_body_available_replay(key, proposal_replay);
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
            && self.context.height == 1
            && self.context.parent_commit_qc.is_none()
            && self.context.snapshot_bootstrap.is_none()
            && self.pending_tip_recovery.is_none()
            && !self.recovered_bodies.contains_key(&key)
            && let Some(authenticated_genesis) = self.authenticated_genesis_body.as_ref()
            && authenticated_genesis.subject() == subject
        {
            let genesis = ReadyBody::derive(
                &self.context,
                round,
                subject,
                Arc::clone(authenticated_genesis.canonical_wire()),
            )
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
            // The staged genesis bytes were authenticated before consensus
            // started, and `ReadyBody::derive` above binds them to the exact
            // certified subject and proposal round. A lagging validator may
            // learn the Decision after every signer has already rolled to the
            // successor height, so a manifest-less certified fetch must be
            // able to consume this local authority instead of depending on a
            // historical network response. An explicitly supplied manifest
            // must still match exactly.
            if manifest
                .as_ref()
                .is_none_or(|manifest| manifest == &genesis.manifest)
            {
                let genesis_manifest = genesis.manifest.clone();
                let genesis_replay = certificate
                    .is_some()
                    .then(|| {
                        PreparedAuthenticatedGenesisFetchReplayPreAdmission::seal_exact_fetch(
                            authenticated_genesis,
                            incoming_effect.clone(),
                            ownership.clone(),
                            genesis_manifest.clone(),
                        )
                    })
                    .transpose()
                    .map_err(|_| {
                        EffectExecutorError::Contract(
                            "certified local genesis Fetch omitted its authenticated replay owner"
                                .to_owned(),
                        )
                    })?;
                let ready_plan =
                    self.plan_ready_body_install(key, genesis, staged_release.clone())?;
                let owner_plan = self.plan_body_pipeline_owner(tag, &genesis_manifest)?;
                let reservation = self
                    .runtime
                    .reserve_body_available_with_owner(tag, genesis_manifest, &ownership)
                    .map_err(runtime_enqueue_error)?;
                self.runtime
                    .commit_body_available(reservation)
                    .map_err(runtime_enqueue_error)?;
                self.commit_body_pipeline_owner(owner_plan);
                self.commit_ready_body_install(ready_plan);
                self.commit_remote_proposal_body_available_replay(key, proposal_replay);
                if let Some(replay) = genesis_replay {
                    let previous = self.authenticated_genesis_replay.insert(
                        key,
                        AuthenticatedGenesisReplayStageV1::BodyAvailable(replay),
                    );
                    debug_assert!(previous.is_none());
                }
                return Ok(());
            }
        }
        if !self.body_pipeline_owners.contains_key(&key)
            && let Some((retained_subject, retained_bytes)) = self.retained_locked_body.as_ref()
            && *retained_subject == subject
            && self.protected_lock == Some(key)
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
                let reservation = self
                    .runtime
                    .reserve_body_available_with_owner(tag, retained_manifest, &ownership)
                    .map_err(runtime_enqueue_error)?;
                self.runtime
                    .commit_body_available(reservation)
                    .map_err(runtime_enqueue_error)?;
                self.commit_body_pipeline_owner(owner_plan);
                self.commit_ready_body_install(ready_plan);
                self.commit_remote_proposal_body_available_replay(key, proposal_replay);
                return Ok(());
            }
        }
        if self.body_pipeline_owners.contains_key(&key) {
            debug_assert!(staged_release.is_none());
            if proposal_replay.is_some() {
                return Err(EffectExecutorError::Contract(
                    "ordinary Proposal Fetch reached an owned body stage without its replay lineage"
                        .to_owned(),
                ));
            }
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
            self.commit_remote_proposal_body_available_replay(key, proposal_replay);
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
            let reservation = self
                .runtime
                .reserve_body_available_with_owner(tag, stored_manifest, &ownership)
                .map_err(runtime_enqueue_error)?;
            self.runtime
                .commit_body_available(reservation)
                .map_err(runtime_enqueue_error)?;
            if let Some(release) = staged_release {
                self.commit_ready_body_release(release);
            }
            self.commit_body_pipeline_owner(owner_plan);
            self.commit_remote_proposal_body_available_replay(key, proposal_replay);
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
            let reservation = self
                .runtime
                .reserve_body_available_with_owner(tag, recovered_manifest, &ownership)
                .map_err(runtime_enqueue_error)?;
            self.runtime
                .commit_body_available(reservation)
                .map_err(runtime_enqueue_error)?;
            if let Some(release) = staged_release {
                self.commit_ready_body_release(release);
            }
            self.commit_body_pipeline_owner(owner_plan);
            self.durable_bodies.insert(key, receipt);
            self.commit_remote_proposal_body_available_replay(key, proposal_replay);
            return Ok(());
        }
        if self.pending_work() > self.config.max_pending_work {
            return Err(EffectExecutorError::Contract(
                "pending effect work exceeded its configured capacity".to_owned(),
            ));
        }
        if self.pending_work() == self.config.max_pending_work {
            // Retain this exact effect and lifecycle owner at the causal FIFO
            // head. Capacity release retries it directly; periodic producer
            // ticks may coalesce with it but may not replace its admission age.
            iroha_logger::debug!(
                height = round.height,
                view = round.view,
                certified = certificate.is_some(),
                "deferred reconstructible Sumeragi v2 body fetch at pending-work capacity"
            );
            return Err(EffectExecutorError::PendingWorkCapacity {
                capacity: self.config.max_pending_work,
            });
        }
        let work = self.plan_work_id()?;
        let request_plan = if let Some(certificate) = certificate {
            match self.plan_certified_fetch_request(work.id, round, subject, certificate, services)
            {
                Ok(plan) => Some(plan),
                Err(EffectExecutorError::CertifiedRequestCapacity { capacity }) => {
                    // Retain and retry this exact owned request after capacity release. A later
                    // periodic producer may coalesce with it but cannot replace the incumbent
                    // service owner's admission age.
                    iroha_logger::debug!(
                        height = round.height,
                        view = round.view,
                        capacity,
                        "deferred certified Sumeragi v2 body fetch at request capacity"
                    );
                    return Err(EffectExecutorError::CertifiedRequestCapacity { capacity });
                }
                Err(error) => return Err(error),
            }
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
            ownership,
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
        if let Some(replay) = proposal_replay {
            let previous = self.remote_proposal_replay.insert(
                key,
                RemoteProposalReplayStageV1::Fetch {
                    work_id: work.id,
                    replay,
                },
            );
            debug_assert!(previous.is_none());
        }
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
        if let Some(receipt) = self.validated_bodies.get(&key) {
            hashes.push(receipt.durable().manifest_hash());
        }
        hashes.extend(
            self.pending_applications
                .values()
                .filter(|pending| {
                    pending.task.validated_receipt.durable().round() == key.0
                        && pending.task.subject == key.1
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
        if self
            .outstanding_requests
            .len()
            .checked_add(self.recovered_decision_fetches.len())
            .is_none_or(|owned| owned >= self.config.max_certified_requests)
        {
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
        let authenticated = self
            .runtime
            .authenticate_certified_body_request(&self.context, request.clone(), &self.requester)
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        let request_hash = authenticated.request_hash();
        if self.certified_work.contains_key(&request_hash)
            || self
                .recovered_decision_fetch_by_request
                .contains_key(&request_hash)
            || self
                .recovered_decision_fetches
                .values()
                .any(|owner| owner.has_same_logical_identity(authenticated.request()))
        {
            return Err(EffectExecutorError::Contract(
                "certified body request already has an ordinary or recovered owner".to_owned(),
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
    fn commit_pending_fetch_retirement(
        &mut self,
        plan: PendingFetchRetirementPlan,
    ) -> Result<(), EffectExecutorError> {
        let key = (plan.pending.task.round, plan.pending.task.subject);
        let retires_proposal_replay = match self.remote_proposal_replay.get(&key) {
            Some(RemoteProposalReplayStageV1::Fetch { work_id, .. }) => {
                if *work_id != plan.pending.task.id() {
                    return Err(EffectExecutorError::Contract(
                        "body-fetch retirement changed its Proposal replay work ID".to_owned(),
                    ));
                }
                true
            }
            Some(_) => {
                return Err(EffectExecutorError::Contract(
                    "body-fetch retirement conflicts with a later Proposal replay stage".to_owned(),
                ));
            }
            None => false,
        };
        let retired_completion = self
            .runtime
            .retire_unpublished_body_available(
                plan.pending.task.tag,
                plan.pending.task.round,
                plan.pending.task.subject,
            )
            .map_err(EffectExecutorError::Runtime)?;
        if !retired_completion {
            let effect = plan.pending.task.adapter_effect();
            self.runtime
                .retire_restored_body_fetch_parent(&effect, plan.pending.task.ownership())
                .map_err(EffectExecutorError::Runtime)?;
        }
        let work_id = plan.pending.task.id();
        let removed = self.pending_fetches.remove(&work_id);
        debug_assert_eq!(removed.as_ref(), Some(&plan.pending));
        if let Some(certified) = plan.certified {
            self.commit_certified_fetch_retirement(certified);
        }
        if retires_proposal_replay {
            self.remote_proposal_replay.remove(&key);
        }
        Ok(())
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
    /// disjoint residual subset during stale-view cleanup. Direct locked-origin
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
        highest_prepare_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    ) -> Result<CertifiedViewBodyCleanupPlan, EffectExecutorError> {
        let stale_stores = self
            .pending_stores
            .iter()
            .filter_map(|(id, pending)| {
                let key = (pending.task.manifest.round, pending.task.manifest.subject);
                let stale_incarnation = pending
                    .consumer
                    .as_ref()
                    .map_or(Some(key) != protected_body, |consumer| {
                        tag.strictly_advances(consumer.tag())
                    });
                ((pending.task.manifest.round.view < tag.view() || stale_incarnation)
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
            if Some(key) == protected_body || Some(key) == highest_prepare_body {
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
            .filter(|key| {
                key.0.view < tag.view()
                    || self
                        .body_pipeline_owners
                        .get(key)
                        .is_some_and(|owner| tag.strictly_advances(owner.tag))
            })
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
        let cleanup_trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_RETIRE,
            relation_exact: plan_exact_body_retirement_accounting(
                self.ready_body_bytes,
                0,
                retired_ready_bytes,
                self.pending_store_bytes,
                retired_store_bytes,
            ) == Some(accounting),
            protected_before: 0,
            protected_after: 0,
            owner_before: 0,
            owner_after: 0,
            owner_reused: false,
            ready_before: self.ready_body_bytes,
            retired_retained: 0,
            retired_ready: retired_ready_bytes,
            ready_after: accounting.ready_after,
            store_before: self.pending_store_bytes,
            retired_store: retired_store_bytes,
            store_after: accounting.store_after,
            cursor_before: 0,
            completion_ready: false,
            progress_ready: false,
            normal_ready: false,
            selected: 0,
            cursor_after: 0,
        };
        let Some(checked_effective_lock) =
            check_production_body_capacity_retirement_effective_lock_transition(cleanup_trace)
        else {
            return Err(EffectExecutorError::Contract(
                "certified-view cleanup did not refine exact effective-lock capacity".to_owned(),
            ));
        };
        Ok(CertifiedViewBodyCleanupPlan {
            stale_stores,
            stale_ready,
            protected_ready_rebinds,
            accounting,
            checked_effective_lock,
        })
    }
    fn store_body<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        mut ownership: RuntimeEffectOwnership,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let key = (round, subject);
        let effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        if !self.remote_proposal_replay.contains_key(&key)
            && !self.authenticated_genesis_replay.contains_key(&key)
            && let Some(adopted) =
                self.stored_replay_incumbent_store_ownership(key, &effect, &ownership)?
        {
            // Retention performs this projection before querying a queued
            // terminal. Recheck the surviving post-Validate seal at dispatch
            // so direct internal consumption cannot substitute a fresh owner.
            ownership = adopted;
        }
        let genesis_disposition =
            self.prepare_authenticated_genesis_store_replay(key, &effect, &ownership)?;
        let advances_genesis_replay = matches!(
            &genesis_disposition,
            AuthenticatedGenesisStoreReplayDispositionV1::Advance
        );
        if let AuthenticatedGenesisStoreReplayDispositionV1::Retry(adopted) = genesis_disposition {
            return self.store_body_inner(tag, round, subject, adopted, services);
        }
        match self.remote_proposal_replay.get(&key) {
            Some(RemoteProposalReplayStageV1::Fetch { .. }) => {
                return Err(EffectExecutorError::Contract(
                    "Proposal StoreBody arrived before its exact Fetch completion".to_owned(),
                ));
            }
            Some(RemoteProposalReplayStageV1::StoreAdmission(replay)) => {
                if !replay.exactly_matches_retry(&effect, &ownership) {
                    return Err(EffectExecutorError::Contract(
                        "Proposal StoreBody retry changed its projected replay owner".to_owned(),
                    ));
                }
            }
            Some(RemoteProposalReplayStageV1::Store { work_id, replay }) => {
                ownership = self
                    .pending_stores
                    .get(work_id)
                    .filter(|pending| {
                        (pending.task.manifest.round, pending.task.manifest.subject) == key
                    })
                    .and_then(|pending| {
                        replay.project_retry_ownership(
                            pending.task.ownership(),
                            &effect,
                            &ownership,
                        )
                    })
                    .ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "Proposal StoreBody retry changed its exact replay owner".to_owned(),
                        )
                    })?;
                return self.store_body_inner(tag, round, subject, ownership, services);
            }
            Some(RemoteProposalReplayStageV1::Stored {
                replay: stored,
                ownership: stored_ownership,
            }) => {
                let receipt = self.durable_bodies.get(&key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "durable Proposal replay lost its exact body receipt".to_owned(),
                    )
                })?;
                ownership = stored
                    .project_retry_ownership(receipt, stored_ownership, &effect, &ownership)
                    .ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "durable Proposal StoreBody retry changed its exact replay owner"
                                .to_owned(),
                        )
                    })?;
                return self.store_body_inner(tag, round, subject, ownership, services);
            }
            Some(RemoteProposalReplayStageV1::BodyAvailable(_)) | None => {}
        }
        if matches!(
            self.remote_proposal_replay.get(&key),
            Some(RemoteProposalReplayStageV1::BodyAvailable(_))
        ) {
            let Some(RemoteProposalReplayStageV1::BodyAvailable(fetch_replay)) =
                self.remote_proposal_replay.remove(&key)
            else {
                unreachable!("preflighted Proposal BodyAvailable replay remains installed")
            };
            let store_replay = match fetch_replay.project_store(effect.clone(), ownership.clone()) {
                Ok(replay) => replay,
                Err(error) => {
                    let previous = self.remote_proposal_replay.insert(
                        key,
                        RemoteProposalReplayStageV1::BodyAvailable(error.into_fetch()),
                    );
                    debug_assert!(previous.is_none());
                    return Err(EffectExecutorError::Contract(
                        "Proposal Fetch replay could not project its exact Store successor"
                            .to_owned(),
                    ));
                }
            };
            let previous = self.remote_proposal_replay.insert(
                key,
                RemoteProposalReplayStageV1::StoreAdmission(store_replay),
            );
            debug_assert!(previous.is_none());
        }
        let advances_proposal_replay = matches!(
            self.remote_proposal_replay.get(&key),
            Some(RemoteProposalReplayStageV1::StoreAdmission(_))
        );
        self.store_body_inner(tag, round, subject, ownership.clone(), services)?;
        if !advances_proposal_replay && !advances_genesis_replay {
            return Ok(());
        }
        if advances_proposal_replay {
            let Some(RemoteProposalReplayStageV1::StoreAdmission(store_replay)) =
                self.remote_proposal_replay.remove(&key)
            else {
                unreachable!("serialized Store keeps the preflighted Proposal replay stage")
            };
            let stage = if let Some(receipt) = self.durable_bodies.get(&key).cloned() {
                let stored = match store_replay.bind_durable_body(receipt.clone()) {
                    Ok(stored) => stored,
                    Err(error) => {
                        let previous = self.remote_proposal_replay.insert(
                            key,
                            RemoteProposalReplayStageV1::StoreAdmission(error.into_store()),
                        );
                        debug_assert!(previous.is_none());
                        return Err(EffectExecutorError::Contract(
                            "Proposal Store replay could not bind its exact durable body"
                                .to_owned(),
                        ));
                    }
                };
                if !stored.exactly_retains_owned_store(&receipt, &ownership) {
                    return Err(EffectExecutorError::Contract(
                        "Proposal Store replay changed its exact retained runtime owner".to_owned(),
                    ));
                }
                RemoteProposalReplayStageV1::Stored {
                    replay: stored,
                    ownership: ownership.clone(),
                }
            } else {
                let Some(work_id) = self.pending_stores.iter().find_map(|(work_id, pending)| {
                    (pending.task.manifest.round == round
                        && pending.task.manifest.subject == subject)
                        .then_some(*work_id)
                }) else {
                    let previous = self.remote_proposal_replay.insert(
                        key,
                        RemoteProposalReplayStageV1::StoreAdmission(store_replay),
                    );
                    debug_assert!(previous.is_none());
                    return Err(EffectExecutorError::Contract(
                        "Proposal Store admission installed neither durable nor pending work"
                            .to_owned(),
                    ));
                };
                RemoteProposalReplayStageV1::Store {
                    work_id,
                    replay: store_replay,
                }
            };
            let previous = self.remote_proposal_replay.insert(key, stage);
            debug_assert!(previous.is_none());
        }
        if advances_genesis_replay {
            self.commit_authenticated_genesis_store_replay(key, ownership)?;
        }
        Ok(())
    }
    fn store_body_inner<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        ownership: RuntimeEffectOwnership,
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
                .enqueue_body_stored_with_owner(tag, round, subject, receipt, &ownership)
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
                ownership,
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
            ownership,
            services,
        )
    }
    fn begin_store<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        canonical_wire: Arc<[u8]>,
        purpose: StorePurpose,
        ownership: RuntimeEffectOwnership,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.begin_store_with_release(
            tag,
            manifest,
            canonical_wire,
            purpose,
            None,
            ownership,
            services,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn begin_store_with_release<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        canonical_wire: Arc<[u8]>,
        purpose: StorePurpose,
        ready_release: Option<ReadyBodyReleasePlan>,
        ownership: RuntimeEffectOwnership,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.begin_store_with_plans(
            tag,
            manifest,
            canonical_wire,
            purpose,
            LocalProposalBodyOrigin::Fresh,
            ready_release,
            None,
            ownership,
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
        local_origin: LocalProposalBodyOrigin,
        ready_release: Option<ReadyBodyReleasePlan>,
        supplied_owner_plan: Option<BodyPipelineOwnerBindingPlan>,
        ownership: RuntimeEffectOwnership,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let key = (manifest.round, manifest.subject);
        let mut consumer = StoreConsumer::new(tag, purpose, ownership, local_origin);
        if let Some(release) = &ready_release
            && (release.key != key
                || release.body.manifest != manifest
                || release.body.bytes.as_ref() != canonical_wire.as_ref())
        {
            return Err(EffectExecutorError::Contract(
                "ready-body release differs from its planned body-store admission".to_owned(),
            ));
        }
        // A physical Store task is immutable, but its reducer consumer belongs
        // to the currently installed runtime incarnation. A late certified
        // carrier can legitimately reach this exact body after EnterView has
        // advanced the reducer tag. Rebind the consumer and exact pipeline
        // owner together only after proving that the incoming Store capability
        // retains the task's lifecycle owner and strictly advances its tag.
        // Local-proposal ownership never crosses this reducer-only corridor.
        if purpose == StorePurpose::Reducer
            && let Some((existing_id, existing)) = self
                .pending_stores
                .iter()
                .find(|(_, pending)| {
                    pending.task.manifest.round == manifest.round
                        && pending.task.manifest.subject == manifest.subject
                })
                .map(|(id, pending)| (*id, pending.clone()))
        {
            let previous_tag = match &existing.consumer {
                Some(StoreConsumer::Reducer { tag, .. }) => Some(*tag),
                Some(StoreConsumer::LocalProposal { .. }) => None,
                None => self.body_pipeline_owners.get(&key).map(|owner| owner.tag),
            };
            if previous_tag.is_some_and(|previous| tag.strictly_advances(previous)) {
                if existing.task.manifest != manifest
                    || existing.task.canonical_wire.as_ref() != canonical_wire.as_ref()
                    || existing.task.ownership() != consumer.ownership()
                {
                    return Err(EffectExecutorError::Contract(
                        "body-store incarnation handoff changed exact work or its lifecycle owner"
                            .to_owned(),
                    ));
                }
                if ready_release.is_some() {
                    return Err(EffectExecutorError::Contract(
                        "ready-body transfer found an existing body-store owner".to_owned(),
                    ));
                }
                let incoming_effect = AdapterEffect::StoreBody {
                    tag,
                    round: manifest.round,
                    subject: manifest.subject,
                };
                if !consumer
                    .ownership()
                    .exactly_binds_adapter_effect(&incoming_effect)
                {
                    return Err(EffectExecutorError::Contract(
                        "body-store incarnation handoff omitted its exact Store capability"
                            .to_owned(),
                    ));
                }
                let previous_tag = previous_tag.expect("strict advancement has one source tag");
                let rebound_owner = self.plan_body_pipeline_owner_rebind(
                    key,
                    previous_tag,
                    tag,
                    Some(HashOf::new(&manifest)),
                )?;
                if existing.consumer.is_some() {
                    services
                        .enqueue_body_store(existing.task.clone())
                        .map_err(service_error)?;
                }
                self.pending_stores
                    .get_mut(&existing_id)
                    .expect("preflighted body store remains serialized")
                    .consumer = Some(consumer);
                self.body_pipeline_owners.insert(key, rebound_owner);
                return Ok(());
            }
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
        if local_origin != LocalProposalBodyOrigin::RecoveredPreIntent
            && let Some(receipt) = self.durable_bodies.get(&key).cloned()
        {
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
                    .enqueue_body_stored_with_owner(
                        tag,
                        manifest.round,
                        manifest.subject,
                        receipt,
                        consumer.ownership(),
                    )
                    .map_err(runtime_enqueue_error)?,
                StorePurpose::LocalProposal => {
                    self.commit_body_pipeline_owner(owner_plan);
                    if let Some(release) = ready_release {
                        self.commit_ready_body_release(release);
                    }
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
                    "body-store retry changed exact work or its lifecycle owner".to_owned(),
                ));
            }
            if let (
                StoreConsumer::Reducer {
                    tag,
                    ownership: incoming_ownership,
                },
                Some((decision_round, proposal_round, decision_subject, commitment)),
            ) = (&mut consumer, self.protected_decision)
            {
                let store_effect = AdapterEffect::StoreBody {
                    tag: *tag,
                    round: manifest.round,
                    subject: manifest.subject,
                };
                *incoming_ownership = existing
                    .task
                    .ownership()
                    .adopt_incumbent_body_stage_for_durable_decision(
                        incoming_ownership,
                        &store_effect,
                        decision_round,
                        proposal_round,
                        decision_subject,
                        commitment,
                    )
                    .map_err(|reason| {
                        EffectExecutorError::Contract(format!(
                            "body-store Decision retry changed exact work or authority: {reason}"
                        ))
                    })?;
            } else if existing.task.ownership() != consumer.ownership() {
                return Err(EffectExecutorError::Contract(
                    "body-store retry changed exact work or its lifecycle owner".to_owned(),
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
            ownership: consumer.ownership().clone(),
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
    fn preflight_pending_application_owner(
        &self,
        work_id: EffectWorkId,
        pending: &PendingApply,
    ) -> Result<(), EffectExecutorError> {
        let task = &pending.task;
        let certificate = task.certificate();
        let validated = task.validated_receipt();
        let durable = validated.durable();
        let decision_key = (
            certificate.round,
            certificate.proposal_round,
            task.subject(),
        );
        let body_key = (durable.round(), task.subject());
        if task.id() != work_id
            || task.tag().height() != self.context.height
            || task.tag() != task.authorized_owner_tag()
            || task.lifecycle_ordinal() != pending.ownership.owner().lifecycle_ordinal()
            || self.runtime.authoritative_tag() != Some(task.authorized_owner_tag())
            || certificate.phase != wire::GlobalPhase::Commit
            || certificate.round.context_id != self.context.id()
            || certificate.round.height != self.context.height
            || certificate.subject != task.subject()
            || durable.context_id() != self.context.id()
            || durable.round() != certificate.proposal_round
            || durable.subject() != task.subject()
            || validated.execution_commitment() != certificate.execution_commitment
            || self.protected_decision
                != Some((
                    decision_key.0,
                    decision_key.1,
                    decision_key.2,
                    certificate.execution_commitment,
                ))
            || !self.decision_body_drained
            || self.durable_bodies.get(&body_key) != Some(durable)
            || self.validated_bodies.get(&body_key) != Some(validated)
        {
            return Err(EffectExecutorError::Contract(
                "pending application differs from its exact decided-body owner".to_owned(),
            ));
        }
        Ok(())
    }
    fn begin_apply<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
        ownership: RuntimeEffectOwnership,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        if tag.height() != self.context.height
            || certificate.validate(&self.context).is_err()
            || certificate.phase != wire::GlobalPhase::Commit
            || certificate.subject != subject
            || certificate.round.context_id != self.context.id()
            || certificate.round.height != self.context.height
        {
            return Err(EffectExecutorError::Contract(
                "Apply is not authorized by the frozen height's exact CommitQC".to_owned(),
            ));
        }
        if let Some(finality) = self.finality_completion.as_ref() {
            if !finality.matches_apply(tag, &self.context, subject, &certificate, &ownership) {
                return Err(EffectExecutorError::Contract(
                    "conflicting Apply retransmission after durable finality".to_owned(),
                ));
            }
            // Kura is already durable and the exact ApplicationCompleted
            // command was admitted before `finality_completion` was latched.
            // A timer or queued CommitQC can rediscover Apply before that
            // command reaches the reducer. Coalesce against the immutable
            // terminal instead of scheduling a second physical application.
            return Ok(());
        }
        if self.runtime.authoritative_tag() != Some(tag) {
            return Err(EffectExecutorError::Contract(
                "Apply is not authorized by the frozen height's exact CommitQC".to_owned(),
            ));
        }
        if let Some(lifecycle_owner) = self.live_lifecycle_decision_apply.as_ref() {
            if lifecycle_owner.exactly_matches_retransmit(tag, subject, &certificate) {
                // The registry-owned worker already retains the sole physical
                // Apply. A periodic or queued reducer retransmit may rediscover
                // that exact decision while lifecycle capacity or completion
                // is pending, but it must not allocate generic work or enqueue
                // a second Kura application.
                return Ok(());
            }
            return Err(EffectExecutorError::Contract(
                "Apply retransmission conflicts with the exact live lifecycle owner".to_owned(),
            ));
        }
        if let Some(existing) = self.pending_applications.values().next() {
            let same_decision = existing.task.tag == tag
                && existing.task.subject == subject
                && existing.ownership == ownership
                && existing
                    .task
                    .certificate
                    .as_ref()
                    .same_commit_decision(certificate.as_ref());
            if !same_decision {
                return Err(EffectExecutorError::Contract(
                    "conflicting Apply retransmission for one height".to_owned(),
                ));
            }
            // A periodic retransmit can rediscover the exact durable Apply
            // while the first task is still in flight. Coalesce on immutable
            // application authority and the already-admitted lifecycle owner.
            if self.deferred_merge_work.contains_key(&existing.task.id()) {
                return Ok(());
            }
            return services
                .enqueue_apply(existing.task.clone())
                .map_err(service_error);
        }
        let (_, durable_receipt, validated_receipt) = select_recovered_decision_body(
            &self.context,
            certificate.round,
            certificate.proposal_round,
            subject,
            certificate.execution_commitment,
            &self.recovered_bodies,
            &self.validated_bodies,
            None,
        )
        .map_err(|reason| EffectExecutorError::Contract(reason.to_owned()))?;
        let body_key = (durable_receipt.round(), subject);
        if self.durable_bodies.get(&body_key) != Some(&durable_receipt)
            || validated_receipt.durable() != &durable_receipt
        {
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
            (
                certificate.round,
                certificate.proposal_round,
                subject,
                certificate.execution_commitment,
            ),
            true,
            services,
        )?;
        self.ensure_pending_slot()?;
        let id = self.allocate_work_id()?;
        let task = ApplyTask {
            id,
            tag,
            authorized_owner_tag: tag,
            subject,
            certificate,
            validated_receipt,
            lifecycle_ordinal: ownership.owner().lifecycle_ordinal(),
        };
        self.pending_applications.insert(
            id,
            PendingApply {
                task: task.clone(),
                ownership,
            },
        );
        services.enqueue_apply(task).map_err(service_error)
    }
    fn reconcile_runtime_decision<S: V2EffectServices>(
        &mut self,
        services: &mut S,
    ) -> Result<Option<DurableDecision>, EffectExecutorError> {
        let decision = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)?;
        if let Some(decision) = decision {
            self.reconcile_decision_work(decision, false, services)?;
        }
        Ok(decision)
    }
    fn finish_runtime_step_reconciliation<S: V2EffectServices>(
        &mut self,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let decided_subject = match self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)?
        {
            Some((decision_round, proposal_round, decision_subject, _)) => {
                if decision_round.context_id != self.context.id()
                    || decision_round.height != self.context.height
                    || proposal_round.context_id != self.context.id()
                    || proposal_round.height != self.context.height
                    || proposal_round != decision_round
                {
                    return Err(EffectExecutorError::Contract(
                        "post-step durable Decision is outside the frozen height context"
                            .to_owned(),
                    ));
                }
                Some(decision_subject)
            }
            None => None,
        };
        services
            .finish_runtime_step_reconciliation(decided_subject)
            .map_err(service_error)
    }
    /// Reconcile volatile ownership immediately after the reducer installs a
    /// durable Decision, before dispatching the Decision's body-recovery
    /// effect. The exact decided pipeline remains live until Apply begins;
    /// every competing owner is retired so it cannot consume the capacity
    /// needed to recover, validate, and apply the decision.
    fn reconcile_decision_work<S: V2EffectServices>(
        &mut self,
        durable_decision: DurableDecision,
        drain_decision_body: bool,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let (decision_round, proposal_round, decision_subject, decision_commitment) =
            durable_decision;
        let decision_body = (proposal_round, decision_subject);
        if decision_round.context_id != self.context.id()
            || decision_round.height != self.context.height
            || proposal_round.context_id != self.context.id()
            || proposal_round.height != self.context.height
            || proposal_round != decision_round
        {
            return Err(EffectExecutorError::Contract(
                "durable Decision is outside the frozen height context".to_owned(),
            ));
        }
        let projected_recovered_decision_seal = self
            .durable_validate_retry_seals
            .get(&decision_body)
            .map(|seal| seal.project_recovered_commitment_ceiling(decision_commitment))
            .transpose()
            .map_err(EffectExecutorError::Contract)?
            .flatten();
        // A protected Prepare lock constrains voting; it does not outrank the
        // first durable quorum Decision. The retirement plan below removes
        // every non-decision owner, including a different protected lock, and
        // then rebinds protection to the exact decided body. A second distinct
        // Decision remains a fail-closed conflict.
        match self.protected_decision {
            Some(existing) if existing != durable_decision => {
                return Err(EffectExecutorError::Contract(
                    "one height installed two different durable Decision identities".to_owned(),
                ));
            }
            Some(_) if !drain_decision_body || self.decision_body_drained => return Ok(()),
            _ => {}
        }
        if drain_decision_body
            && (!self.pending_applications.is_empty()
                || !self.recovered_decision_fetch_request_index_is_exact_and_empty())
        {
            return Err(EffectExecutorError::Contract(
                "terminal body cleanup began while an application or recovered Fetch owner remained"
                    .to_owned(),
            ));
        }
        if !self.pending_durable_validate_admissions.is_empty()
            || self.pending_live_wal_sign_admission.is_some()
            || !self.pending_lifecycle_output_admissions.is_empty()
        {
            return Err(EffectExecutorError::Contract(
                "Decision cleanup overtook a lifecycle admission owner".to_owned(),
            ));
        }
        self.preflight_remote_proposal_replay_indexes()?;
        let first_install = self.protected_decision.is_none();
        let retire_key = |key: (wire::ConsensusRound, wire::BlockSubject)| {
            drain_decision_body || key != decision_body
        };
        self.preflight_exact_body_byte_accounting()?;
        let exact_local_stores = self
            .pending_stores
            .iter()
            .filter_map(|(id, pending)| {
                ((pending.task.manifest.round, pending.task.manifest.subject) == decision_body
                    && matches!(
                        &pending.consumer,
                        Some(StoreConsumer::LocalProposal { .. }) | None
                    ))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        if exact_local_stores.len() > 1 {
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
                    decision_body.0,
                    decision_subject,
                    decision_commitment,
                )
                .map_err(EffectExecutorError::Runtime)?;
            services
                .retire_all_outbound_payloads()
                .map_err(service_error)?;
            services
                .retire_candidate_work_after_decision(proposal_round, decision_subject)
                .map_err(service_error)?;
        }
        if usize::from(proposal_retirement.retained_local_proposal().is_some())
            .saturating_add(exact_local_stores.len())
            > 1
        {
            return Err(EffectExecutorError::Contract(
                "decided body has duplicate local-proposal completion ownership".to_owned(),
            ));
        }
        if let Some(retained_tag) = proposal_retirement.retained_local_proposal() {
            let owner = self
                .body_pipeline_owners
                .get(&decision_body)
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "retained decided local completion has no executor pipeline owner"
                            .to_owned(),
                    )
                })?;
            let retained_hash = self
                .retained_body_manifest_hash(decision_body)?
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "retained decided local completion has no durable body evidence".to_owned(),
                    )
                })?;
            let validated = self.validated_bodies.get(&decision_body).ok_or_else(|| {
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
            || !exact_local_stores.is_empty();
        let detached_decision_owner = detach_decision_pipeline
            .then(|| self.body_pipeline_owners.get(&decision_body).copied())
            .flatten();
        let pipeline_keys = self
            .body_pipeline_owners
            .iter()
            .filter_map(|(key, owner)| {
                (retire_key(*key) && !(detach_decision_pipeline && *key == decision_body))
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
        for id in &exact_local_stores {
            self.pending_stores
                .get_mut(id)
                .expect("preflighted decided local store remains serialized")
                .consumer = None;
            self.local_store_replay.remove(id);
        }
        if detach_decision_pipeline {
            self.body_pipeline_owners.remove(&decision_body);
            if let Some(owner) = detached_decision_owner {
                self.retire_local_proposal_ready_replay(
                    owner.tag,
                    decision_body.0,
                    decision_body.1,
                );
            }
        }
        for plan in fetches {
            self.commit_pending_fetch_retirement(plan)?;
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
            self.local_store_replay.remove(&id);
        }
        for (key, owner) in pipeline_keys {
            self.retire_local_proposal_ready_replay(owner.tag, key.0, key.1);
        }
        if first_install && let Some(owner) = self.body_pipeline_owners.get(&decision_body).copied()
        {
            self.retire_local_proposal_ready_replay(owner.tag, decision_body.0, decision_body.1);
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
        // A durable Decision retires every competing signed-Proposal origin.
        // Preserve the selected body's exact Store lineage before or after
        // fsync whenever terminal cleanup is not draining that body. The
        // selected immutable persistence task remains live above, so its
        // replay owner must advance monotonically into Stored before the
        // Commit-refined Validate consumes it. Earlier physical stages still
        // restart from Decision recovery.
        self.remote_proposal_replay.retain(|key, stage| {
            !drain_decision_body
                && *key == decision_body
                && matches!(
                    stage,
                    RemoteProposalReplayStageV1::Store { .. }
                        | RemoteProposalReplayStageV1::Stored { .. }
                )
        });
        self.authenticated_genesis_replay.retain(|key, stage| {
            !drain_decision_body
                && *key == decision_body
                && matches!(
                    stage,
                    AuthenticatedGenesisReplayStageV1::Store { .. }
                        | AuthenticatedGenesisReplayStageV1::Stored { .. }
                )
        });
        // A resolved Validate owner may leave an inert idempotence tombstone,
        // while an ordinal-bound seal still denotes a live registry row.
        // Preserve every live row across Decision cleanup; exact lifecycle
        // settlement releases it later and discards losing or drained rows.
        // Only the selected ordinary-body tombstone may survive unbound.
        if let Some(seal) = projected_recovered_decision_seal
            && (!drain_decision_body || seal.lifecycle_ordinal().is_some())
        {
            self.durable_validate_retry_seals
                .insert(decision_body, seal);
        }
        self.durable_validate_retry_seals.retain(|key, seal| {
            seal.lifecycle_ordinal().is_some() || (!drain_decision_body && *key == decision_body)
        });
        self.published_lifecycle_validate_retry_markers
            .retain(|key, marker| {
                marker.owns_live_lifecycle_row() || (!drain_decision_body && *key == decision_body)
            });
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
            && (!self.certified_work.is_empty()
                || !self.outstanding_requests.is_empty()
                || !self.recovered_decision_fetch_request_index_is_exact_and_empty())
        {
            return Err(EffectExecutorError::Contract(
                "terminal cleanup left an unowned certified-body request".to_owned(),
            ));
        }
        self.protected_decision = Some(durable_decision);
        self.protected_lock = Some(decision_body);
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
        let (reuses_existing_stage, ready_release) =
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
                let ready_release = if self.ready_bodies.contains_key(&key) {
                    Some(
                        self.plan_ready_body_release(key)
                            .map_err(|error| self.fail_closed_transport(error, services))?,
                    )
                } else {
                    None
                };
                (true, ready_release)
            } else if let Some(ready) = self.ready_bodies.get(&key) {
                if &ready.manifest != manifest || ready.bytes.as_ref() != bytes.as_ref() {
                    return Err(self.fail_closed_transport(
                        "completed fetch conflicts with retained ready body identity",
                        services,
                    ));
                }
                (true, None)
            } else {
                (false, None)
            };
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
        let runtime_reservation = match self.runtime.reserve_body_available_with_owner(
            tag,
            runtime_manifest,
            task.ownership(),
        ) {
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
    fn commit_fetch_completion(&mut self, plan: FetchCompletionPlan) -> Result<(), EnqueueError> {
        let key = plan.owner.key;
        let advances_proposal_replay = match self.remote_proposal_replay.get(&key) {
            Some(RemoteProposalReplayStageV1::Fetch { work_id, .. }) => {
                if *work_id != plan.work_id {
                    return Err(EnqueueError::FailClosed);
                }
                true
            }
            Some(_) => return Err(EnqueueError::FailClosed),
            None => false,
        };
        // Publish the already-reserved reducer successor before retiring its
        // external fetch owner. A mismatched token leaves all local ownership
        // intact and forces the serialized runtime into fail-closed state.
        self.runtime
            .commit_body_available(plan.runtime_reservation)?;
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
        if advances_proposal_replay {
            let Some(RemoteProposalReplayStageV1::Fetch { replay, .. }) =
                self.remote_proposal_replay.remove(&key)
            else {
                unreachable!("preflighted Proposal Fetch replay remains installed")
            };
            let previous = self
                .remote_proposal_replay
                .insert(key, RemoteProposalReplayStageV1::BodyAvailable(replay));
            debug_assert!(previous.is_none());
        }
        Ok(())
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
        if let Err(error) = self.commit_fetch_completion(plan) {
            return Err(self.fail_closed_transport(runtime_enqueue_error(error), services));
        }
        Ok(CompletionDisposition::Accepted)
    }
    fn install_view<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        certificate: wire::TimeoutCertificate,
        protected_lock: Option<wire::QuorumCertificate>,
        highest_prepare_retention: Option<wire::QuorumCertificateRef>,
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
        self.preflight_highest_prepare_frontier(Some(tag), highest_prepare_retention)?;
        if highest_prepare_retention.is_some_and(|highest| highest.round.view >= tag.view()) {
            return Err(EffectExecutorError::Contract(
                "EnterView cleanup frontier retained a non-historical highest Prepare".to_owned(),
            ));
        }
        if let Some(protected) = protected_lock.as_ref() {
            protected.validate(&self.context).map_err(|error| {
                EffectExecutorError::Contract(format!(
                    "EnterView protected lock is invalid: {error}"
                ))
            })?;
            if protected.phase != wire::GlobalPhase::Prepare
                || protected.proposal_round.context_id != self.context.id()
                || protected.proposal_round.height != self.context.height
                || protected.proposal_round.view >= tag.view()
            {
                return Err(EffectExecutorError::Contract(
                    "EnterView protected lock is outside the installed height/view".to_owned(),
                ));
            }
        }
        if let Some(highest) = certificate.highest_prepare_qc() {
            let Some(protected) = protected_lock.as_ref() else {
                return Err(EffectExecutorError::Contract(
                    "EnterView omitted the lock selected by its highest PrepareQC".to_owned(),
                ));
            };
            if protected.round.view < highest.round.view
                || (protected.round.view == highest.round.view
                    && (protected.round != highest.round
                        || protected.proposal_round != highest.proposal_round
                        || protected.phase != highest.phase
                        || protected.subject != highest.subject
                        || protected.execution_commitment != highest.execution_commitment))
            {
                return Err(EffectExecutorError::Contract(
                    "EnterView protected lock is lower than or conflicts with its highest PrepareQC"
                        .to_owned(),
                ));
            }
        }
        let protected_body = protected_lock_body(protected_lock.as_ref());
        let highest_prepare_body = highest_prepare_body(highest_prepare_retention);
        // The typed pacemaker path may install this TC while an ordinary
        // reducer suffix is parked behind adapter backpressure. Effects from
        // the superseded incarnation have not acquired service ownership yet,
        // so retire them here. The reducer has already rebuilt every active
        // control and protected-body successor in this EnterView batch;
        // diagnostics are view-independent and remain parked.
        if let Some(batch) = self.parked_effect_batch.as_mut() {
            batch
                .effects
                .retain(|owned| !Self::parked_effect_is_retired_by_view(owned, tag));
        }
        if self
            .parked_effect_batch
            .as_ref()
            .is_some_and(|batch| batch.effects.is_empty())
        {
            self.parked_effect_batch = None;
        }
        // A certified-request index mismatch must be diagnosed before lock
        // reconciliation, which can itself retire runtime/service ownership.
        // Protected fetch rebinding is also fully checked here so no fallible
        // executor lookup remains after its service callback acknowledges.
        self.preflight_certified_fetch_indexes()?;
        self.preflight_exact_body_byte_accounting()?;
        if !self.pending_durable_validate_admissions.is_empty()
            || self.pending_live_wal_sign_admission.is_some()
            || !self.pending_lifecycle_output_admissions.is_empty()
        {
            return Err(EffectExecutorError::Contract(
                "EnterView overtook a lifecycle admission owner".to_owned(),
            ));
        }
        self.preflight_remote_proposal_replay_indexes()?;
        for pending in self
            .pending_fetches
            .values()
            .filter(|pending| tag.strictly_advances(pending.task.tag))
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
        self.reconcile_protected_lock(tag, protected_body, highest_prepare_body, services)?;
        let stale_body_cleanup =
            self.plan_certified_view_body_cleanup(tag, protected_body, highest_prepare_body)?;
        let stale_fetches = self
            .pending_fetches
            .values()
            .filter(|pending| tag.strictly_advances(pending.task.tag))
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
                    // A typed Retryable certified-response handoff retains an
                    // unpublished BodyAvailable token in runtime ingress. The
                    // protected FetchBody task and that token are one logical
                    // pipeline: move both to the installed incarnation before
                    // publishing the local task mutation. Backpressure before
                    // reservation legitimately leaves no token to move.
                    self.runtime
                        .rebind_unpublished_body_available(
                            pending.task.tag,
                            tag,
                            pending.task.round,
                            pending.task.subject,
                        )
                        .map_err(EffectExecutorError::Runtime)?;
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
                    self.commit_pending_fetch_retirement(retirement)?;
                }
            }
        }
        let stale = self
            .pending_signatures
            .iter()
            .filter_map(|(id, pending)| tag.strictly_advances(pending.tag).then_some(*id))
            .collect::<Vec<_>>();
        for id in stale {
            services.cancel_consensus_sign(id).map_err(service_error)?;
            self.pending_signatures.remove(&id);
        }
        // Byte residuals for the complete store/ready cleanup were checked
        // before any cancellation. A corrupt counter therefore cannot retire
        // worker or runtime ownership and only then discover the underflow.
        let _authorized_body_cleanup = stale_body_cleanup.checked_effective_lock.into_projection();
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
            if Some(key) != protected_body && Some(key) != highest_prepare_body {
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
        let protected_ready_rebind_keys = stale_body_cleanup
            .protected_ready_rebinds
            .iter()
            .map(|rebind| rebind.key)
            .collect::<BTreeSet<_>>();
        let stale_pipeline_owners = self
            .body_pipeline_owners
            .iter()
            .filter_map(|(key, owner)| {
                (tag.strictly_advances(owner.tag) && !protected_ready_rebind_keys.contains(key))
                    .then_some((*key, *owner))
            })
            .collect::<Vec<_>>();
        for (key, owner) in &stale_pipeline_owners {
            // Once an EnterView supersedes a physical pipeline incarnation,
            // every queued completion under that exact owner must retire before
            // the owner can be released below. Ready-body cleanup already took
            // BodyAvailable when present; this exact all-stage pass also drains
            // BodyStored/LocalProposalReady after their ready bytes disappeared.
            // Protected ready/fetch pipelines were rebound to `tag` above and
            // are deliberately absent from this retirement census.
            self.runtime
                .retire_body_pipeline_completions(owner.tag, key.0, key.1)
                .map_err(EffectExecutorError::Runtime)?;
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
            if Some(key) == protected_body || Some(key) == highest_prepare_body {
                // Persistence work and its canonical bytes are immutable. A
                // timeout may replace the reducer consumer, but it must not
                // restart the exact durable-lock store or race cancellation
                // against a completion already minted by the worker.
                self.pending_stores
                    .get_mut(id)
                    .expect("preflighted protected store remains serialized")
                    .consumer = None;
                self.local_store_replay.remove(id);
            } else {
                self.pending_stores
                    .remove(id)
                    .expect("preflighted retired store remains serialized");
                self.local_store_replay.remove(id);
            }
        }
        for key in &stale_body_cleanup.stale_ready {
            if Some(*key) != protected_body {
                if let Some(owner) = self.body_pipeline_owners.get(key).copied() {
                    self.retire_local_proposal_ready_replay(owner.tag, key.0, key.1);
                }
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
            .map(|pending| {
                (
                    pending.task.validated_receipt.durable().round(),
                    pending.task.subject,
                )
            })
            .collect::<BTreeSet<_>>();
        let retained_store_owners = self
            .pending_stores
            .values()
            .map(|pending| (pending.task.manifest.round, pending.task.manifest.subject))
            .collect::<BTreeSet<_>>();
        self.body_pipeline_owners.retain(|key, owner| {
            !tag.strictly_advances(owner.tag)
                || retained_apply_owners.contains(key)
                || retained_store_owners.contains(key)
        });
        // Unprotected Proposal families retire with the superseded view.
        // Preserve the protected body's exact Store lineage both before and
        // after fsync. The in-flight persistence task above is deliberately
        // detached instead of cancelled, so its move-only replay owner must
        // advance monotonically from Store to Stored when that same task
        // completes. A later Prepare/Commit Validate carrier then adopts the
        // incumbent causal root before consuming it. Earlier physical stages
        // still restart from the certified pipeline.
        self.remote_proposal_replay.retain(|key, stage| {
            (Some(*key) == protected_body || Some(*key) == highest_prepare_body)
                && matches!(
                    stage,
                    RemoteProposalReplayStageV1::Store { .. }
                        | RemoteProposalReplayStageV1::Stored { .. }
                )
        });
        self.authenticated_genesis_replay.retain(|key, stage| {
            (Some(*key) == protected_body || Some(*key) == highest_prepare_body)
                && matches!(
                    stage,
                    AuthenticatedGenesisReplayStageV1::Store { .. }
                        | AuthenticatedGenesisReplayStageV1::Stored { .. }
                )
        });
        // Retry authorities own no service work. Ordinal-bound entries still
        // represent live registry rows and survive view cleanup; among
        // resolved tombstones, only the exact protected body or cleanup-only
        // durable high can still emit a legitimate duplicate. Active published
        // Store markers remain executable lifecycle rows and are retired only
        // by their atomic Store-to-Validate handoff.
        self.durable_validate_retry_seals.retain(|key, seal| {
            seal.lifecycle_ordinal().is_some()
                || Some(*key) == protected_body
                || Some(*key) == highest_prepare_body
        });
        self.published_lifecycle_validate_retry_markers
            .retain(|key, marker| {
                marker.owns_live_lifecycle_row()
                    || Some(*key) == protected_body
                    || Some(*key) == highest_prepare_body
            });
        let retain_local_producer = self.local_validator == Some(self.context.leader(tag.view()));
        self.runtime
            .reconcile_active_view_producer(tag, retain_local_producer)
            .map_err(EffectExecutorError::Runtime)?;
        services
            .entered_view(tag, certificate, protected_body)
            .map_err(service_error)?;
        self.reconciled_tag = Some(tag);
        Ok(())
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
                if self
                    .protected_decision
                    .is_some_and(|(_, proposal_round, subject, _)| {
                        subject == key.1 && key.0 == proposal_round
                    })
                {
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
            || self.pending_durable_validate_admissions.contains_key(&key)
            || self.pending_applications.values().any(|application| {
                (
                    application.task.validated_receipt.durable().round(),
                    application.task.subject,
                ) == key
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
        self.commit_pending_fetch_retirement(retirement)?;
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
        let depth = self
            .retained_effect_batch
            .as_ref()
            .map_or(0, |batch| batch.effects.len())
            .saturating_add(
                self.parked_effect_batch
                    .as_ref()
                    .map_or(0, |batch| batch.effects.len()),
            );
        let oldest_at = self
            .retained_effect_batch
            .as_ref()
            .map(|batch| batch.oldest_at)
            .into_iter()
            .chain(
                self.parked_effect_batch
                    .as_ref()
                    .map(|batch| batch.oldest_at),
            )
            .min();
        RuntimeQueueLaneSnapshot {
            depth,
            capacity: MAX_EFFECTS_PER_STEP.saturating_mul(2),
            oldest_age: oldest_at.map(|oldest| now.saturating_duration_since(oldest)),
            // A parked suffix can lose one bounded dispatch to typed control;
            // it is restored before any ordinary runtime transition.
            max_service_debt: u64::from(self.parked_effect_batch.is_some()),
        }
    }
    /// Validate the dedicated recovered-Fetch owner/reverse index and require
    /// it to be empty at a height-finalization boundary. These owners are not
    /// ordinary `EffectWorkId` capacity and therefore deliberately stay out of
    /// [`Self::pending_work`].
    fn recovered_decision_fetch_request_index_is_exact_and_empty(&self) -> bool {
        self.recovered_decision_fetch_request_index_is_exact()
            && self.recovered_decision_fetches.is_empty()
            && self.recovered_decision_fetch_by_request.is_empty()
    }
    fn recovered_decision_fetch_request_index_is_exact(&self) -> bool {
        self.recovered_decision_fetches.len() == self.recovered_decision_fetch_by_request.len()
            && self.recovered_decision_fetches.iter().all(|(key, owner)| {
                owner.dispatch_key() == *key
                    && self
                        .recovered_decision_fetch_by_request
                        .get(&owner.request_hash())
                        == Some(key)
            })
            && self
                .recovered_decision_fetch_by_request
                .iter()
                .all(|(request_hash, key)| {
                    self.recovered_decision_fetches
                        .get(key)
                        .is_some_and(|owner| owner.request_hash() == *request_hash)
                })
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
        // The retained relay may terminate the process as soon as the guard
        // closes, so preserve the precise reason before publishing that edge.
        iroha_logger::error!(%reason, "Sumeragi v2 effect transport failed closed");
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
        // Log before guard activation. The retained relay observes that state
        // concurrently and may exit before `services.fail_closed` can report
        // the originating executor error.
        iroha_logger::error!(%error, "Sumeragi v2 effect executor failed closed");
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
        // The compact carrier authenticates the immutable block header. A
        // later same-body reproposal therefore retains its earlier carrier
        // view, while a future-view carrier cannot authenticate older work.
        && certificate.view <= round.view
        && subject.parent_block_hash == Some(certificate.carrier_parent_hash)
}
fn manifests_identify_same_body(
    left: &wire::PayloadManifest,
    right: &wire::PayloadManifest,
) -> bool {
    left.subject == right.subject
        && left.payload_size_bytes == right.payload_size_bytes
        && left.layout == right.layout
        && left.chunk_hashes == right.chunk_hashes
        && left.chunk_root == right.chunk_root
}
fn select_recovered_decision_body(
    context: &wire::HeightContext,
    decision_round: wire::ConsensusRound,
    proposal_round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
    recovered_bodies: &BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    validated_bodies: &BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    advertised_manifest: Option<&wire::PayloadManifest>,
) -> Result<
    (
        wire::PayloadManifest,
        DurableBodyReceipt,
        ValidatedBodyReceipt,
    ),
    &'static str,
> {
    if decision_round.context_id != context.id()
        || decision_round.height != context.height
        || proposal_round.context_id != context.id()
        || proposal_round.height != context.height
        || proposal_round != decision_round
    {
        return Err("recovered body frame differs from the replayed Decision key");
    }
    let key = (proposal_round, subject);
    let (selected_manifest, selected_durable) = recovered_bodies
        .get(&key)
        .ok_or("replayed Decision has no matching checksummed durable body frame")?;
    if selected_manifest.round != proposal_round
        || selected_manifest.subject != subject
        || !store_completion_matches(context, selected_manifest, selected_durable)
    {
        return Err("recovered body frame differs from the replayed Decision key");
    }
    if advertised_manifest.is_some_and(|advertised| advertised != selected_manifest) {
        return Err("recovered body frame differs from the replayed Decision key");
    }
    let validated = validated_bodies
        .get(&key)
        .ok_or("replayed Decision has no matching durable validation marker")?;
    if validated.durable() != selected_durable {
        return Err("durable validation marker differs from the recovered exact body frame");
    }
    if validated.execution_commitment() != execution_commitment {
        return Err("durable Decision commitment differs from the recovered validation marker");
    }
    for (key, (alias_manifest, alias_durable)) in recovered_bodies {
        let (origin_round, origin_subject) = *key;
        if origin_subject != subject
            || origin_round == proposal_round
            || origin_round.context_id != decision_round.context_id
            || origin_round.height != decision_round.height
            || origin_round.view > decision_round.view
        {
            continue;
        }
        if alias_manifest.round != origin_round
            || alias_manifest.subject != subject
            || !store_completion_matches(context, alias_manifest, alias_durable)
        {
            return Err("recovered body frame differs from the replayed Decision key");
        }
        if !manifests_identify_same_body(selected_manifest, alias_manifest) {
            return Err("recovered aliases conflict on the exact decided body identity");
        }
        let Some(alias_validated) = validated_bodies.get(key) else {
            continue;
        };
        if alias_validated.durable() != alias_durable {
            return Err("durable validation marker differs from the recovered exact body frame");
        }
        if alias_validated.execution_commitment() != execution_commitment {
            return Err("durable Decision commitment differs from the recovered validation marker");
        }
    }
    Ok((
        selected_manifest.clone(),
        selected_durable.clone(),
        validated.clone(),
    ))
}
fn verify_pending_kura_apply_parts_inner(
    context: &wire::HeightContext,
    decision: Option<DurableDecision>,
    recovered_bodies: &BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    validated_bodies: &BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    expected: PendingKuraApply,
    replay_tag: EventTag,
    owner_tag: EventTag,
    certificate: wire::QuorumCertificate,
    advertised_manifest: Option<&wire::PayloadManifest>,
    deferred_validated_marker: Option<super::v2::DeferredPendingKuraValidatedMarkerV1>,
) -> Result<
    (
        Option<VerifiedPendingGenesisNexusAmxContext>,
        PendingKuraApplyRecoveryEvidence,
    ),
    EffectExecutorError,
> {
    let mismatch =
        |reason: &'static str| EffectExecutorError::PendingApplyRecoveryMismatch(reason.to_owned());
    if expected.context_id() != context.id() || expected.height() != context.height {
        return Err(mismatch(
            "recovered Kura tip belongs to a different frozen height context",
        ));
    }
    let (round, proposal_round, subject, execution_commitment) = decision.ok_or_else(|| {
        mismatch("canonical Kura tip has no complete durable Decision WAL record")
    })?;
    if certificate.validate(context).is_err()
        || certificate.phase != wire::GlobalPhase::Commit
        || certificate.round != round
        || certificate.proposal_round != proposal_round
        || certificate.subject != subject
        || certificate.execution_commitment != execution_commitment
    {
        return Err(mismatch(
            "replayed certified FetchBody does not carry the exact durable CommitQC",
        ));
    }
    if replay_tag.height() != context.height || replay_tag != owner_tag {
        return Err(mismatch(
            "replayed Decision effect does not belong to the frozen reducer incarnation",
        ));
    }
    if round.context_id != context.id()
        || round.height != context.height
        || subject.block_hash != expected.block_hash()
    {
        return Err(mismatch(
            "replayed Decision does not identify the canonical pending Kura tip",
        ));
    }
    let (manifest, durable, validated) = select_recovered_decision_body(
        context,
        round,
        proposal_round,
        subject,
        execution_commitment,
        recovered_bodies,
        validated_bodies,
        advertised_manifest,
    )
    .map_err(mismatch)?;
    let deferred_validated_marker = match deferred_validated_marker {
        Some(marker) => marker,
        None => {
            #[cfg(test)]
            {
                super::v2::DeferredPendingKuraValidatedMarkerV1::for_test(
                    replay_tag,
                    &manifest,
                    &durable,
                    &validated,
                    &certificate,
                )
            }
            #[cfg(not(test))]
            {
                return Err(mismatch(
                    "pending Kura replay omitted its exact deferred validation marker",
                ));
            }
        }
    };
    if !deferred_validated_marker.exactly_matches_recovery(
        context,
        expected,
        replay_tag,
        &manifest,
        &durable,
        &validated,
        &certificate,
    ) {
        return Err(mismatch(
            "pending Kura deferred marker differs from its exact recovered Decision body",
        ));
    }
    let genesis_context = (context.height == 1).then_some(VerifiedPendingGenesisNexusAmxContext {
        hash: context.nexus_amx_context_hash,
    });
    let evidence = PendingKuraApplyRecoveryEvidence {
        expected,
        frozen_context_id: context.id(),
        frozen_height: context.height,
        replay_tag,
        owner_tag,
        replay_generation: replay_tag.generation().get(),
        commit_qc: certificate,
        manifest_hash: HashOf::new(&manifest),
        manifest,
        durable_frame_hash: durable.frame_hash(),
        durable_receipt: durable,
        validated_receipt: validated,
        deferred_validated_marker: Some(deferred_validated_marker),
        stage: PendingKuraApplyRecoveryStage::CertifiedFetch,
    };
    if !evidence.is_exact(context) {
        return Err(mismatch(
            "replayed Decision recovery evidence lost an exact native identity field",
        ));
    }
    let recovery_trace = evidence.recovery_refinement_projection().ok_or_else(|| {
        mismatch("replayed Decision recovery evidence cannot be represented losslessly")
    })?;
    let Some(checked_recovery) = check_production_decision_recovery_transition(recovery_trace)
    else {
        return Err(mismatch(
            "replayed Decision recovery evidence failed the shared exact-identity kernel",
        ));
    };
    let _authorized_recovery = checked_recovery.into_projection();
    Ok((genesis_context, evidence))
}

fn verify_pending_kura_apply_parts_with_marker(
    context: &wire::HeightContext,
    decision: Option<DurableDecision>,
    recovered_bodies: &BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    validated_bodies: &BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    expected: PendingKuraApply,
    replay_tag: EventTag,
    owner_tag: EventTag,
    certificate: wire::QuorumCertificate,
    advertised_manifest: Option<&wire::PayloadManifest>,
    deferred_validated_marker: super::v2::DeferredPendingKuraValidatedMarkerV1,
) -> Result<
    (
        Option<VerifiedPendingGenesisNexusAmxContext>,
        PendingKuraApplyRecoveryEvidence,
    ),
    EffectExecutorError,
> {
    verify_pending_kura_apply_parts_inner(
        context,
        decision,
        recovered_bodies,
        validated_bodies,
        expected,
        replay_tag,
        owner_tag,
        certificate,
        advertised_manifest,
        Some(deferred_validated_marker),
    )
}

#[cfg(test)]
fn verify_pending_kura_apply_parts(
    context: &wire::HeightContext,
    decision: Option<DurableDecision>,
    recovered_bodies: &BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    validated_bodies: &BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    expected: PendingKuraApply,
    replay_tag: EventTag,
    owner_tag: EventTag,
    certificate: wire::QuorumCertificate,
    advertised_manifest: Option<&wire::PayloadManifest>,
) -> Result<
    (
        Option<VerifiedPendingGenesisNexusAmxContext>,
        PendingKuraApplyRecoveryEvidence,
    ),
    EffectExecutorError,
> {
    verify_pending_kura_apply_parts_inner(
        context,
        decision,
        recovered_bodies,
        validated_bodies,
        expected,
        replay_tag,
        owner_tag,
        certificate,
        advertised_manifest,
        None,
    )
}
#[cfg(test)]
mod tests {
    include!("tests/v2_effects_main_00.rs");
    include!("tests/v2_effects_main_01.rs");
    include!("tests/v2_effects_main_02.rs");
    include!("tests/v2_effects_main_03.rs");
    include!("tests/v2_effects_main_04.rs");
    include!("tests/v2_effects_main_05.rs");
    include!("tests/v2_effects_03_locked_body_and_sidecar.rs");
}
