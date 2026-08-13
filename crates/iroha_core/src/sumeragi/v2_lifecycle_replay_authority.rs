//! Closed, codec-only authority envelope for future lifecycle replay.
//!
//! This module deliberately performs structural matching only. Decoding this
//! envelope does not authenticate its consensus artifacts or make executable
//! work. A future admission transaction must first reauthenticate the retained
//! source against the verified height context and its owning durable store.

use std::{mem::size_of, sync::Arc};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
use norito::codec::{Decode, DecodeAll as _, Encode};

use crate::sumeragi::{
    v2::{
        AdapterEffect, ExactLiveWalPersistedContinuationCause, LiveWalFrameIdentity,
        PersistedWalFrameLocatorV1, RecoveredDecisionApplyCandidateProjectionPermit,
        RecoveredWalFrameIdentity, RegisteredPrepareInvalidBodyReportCapability, SignRequest,
        VerifiedHeightContext,
    },
    v2_body_store::{DurableBodyReceipt, DurableCertifiedFetchBodyReceipt, ValidatedBodyReceipt},
    v2_certified_serve_payload_store::{
        AuthenticatedRecoveredCertifiedServePayload,
        AuthenticatedRecoveredCertifiedServePayloadState, CertifiedServePayloadNegativeOutcome,
        DurableCertifiedServeAdmissionReceipt, DurableCertifiedServeCompletedReceipt,
        DurableCertifiedServeNegativeReceipt,
    },
    v2_core::EventTag,
    v2_runtime::{
        LocalBodyReplayMintPermit, LocalProposalReadyCommandIdentity, PendingRuntimeEffectBinding,
        RecoveredWalCandidateProjectionPermit, RecoveredWalDecisionFetchPendingMintPermit,
        RemoteProposalReplayMintPermit, RuntimeEffectOwnership, RuntimeIngressOwnershipEvidence,
    },
    v2_transport::AuthenticatedCertifiedBodyRequest,
};

use super::ledger::{
    DurableCertifiedFetchLedgerCensusPermit, DurableCertifiedFetchLedgerJoinPermit,
    LifecycleLedgerRecordV1,
};
use super::{
    body_pipeline_transition::{
        SealedInvalidBodyReportProjectionPermit, SealedValidateSignProjectionPermit,
    },
    projection::{
        AdapterEffectAdmissionError, AuthenticatedDurableBodyFrameRecovery,
        DurableBodyFrameRecoveryError, block_subject, certified_serve_key_subject,
        durable_body_frame_reference, execution_commitment,
    },
    schema::{
        CandidateAdmission, CausalRoot, DurableBodyFrameReference, DurablePayloadReference,
        DurableRecordMetadata, DurableServeNegativeOutcome, InitialLifecycleState,
        LifecycleContext, LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRecord,
        LifecycleRound, LifecycleStage, LifecycleStageKind, LifecycleWorkClass, OwnerId,
        PhysicalGeometry, PhysicalSlot, PhysicalSlotId, PredecessorScope, ProducerTurnAdmission,
        TerminalOutcome, serve_and_producer_keys_match,
    },
    selector::CertifiedFetchCompletionAuthority,
    work_registry::{
        CertifiedFetchCompletion, ConcreteLifecycleWorkRegistry,
        InstalledBodyCandidateProjectionPermit, LiveValidateSignWorkProjectionPermit,
        PreparedLiveValidateSignRegistryWork, SealedBodySuccessorProjectionPermit,
    },
};

const REPLAY_AUTHORITY_FORMAT_VERSION: u16 = 1;
const MAX_REPLAY_AUTHORITY_BYTES: usize = 4 * 1024 * 1024;
const EQUIVOCATION_SUBJECT_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:equivocation-subject:v1";
const PRODUCER_TURN_PHYSICAL_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:lifecycle:producer-turn-physical:v1";

/// Version-one replay envelope retained beside one lifecycle ledger row.
///
/// The fields are private so neither decoded wire values nor an arbitrary
/// source can become runtime authority through a parts API.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(in crate::sumeragi) struct LifecycleReplayAuthorityV1 {
    format_version: u16,
    payload: ReplayPayloadBindingV1,
    source: LifecycleReplaySourceV1,
}

impl LifecycleReplayAuthorityV1 {
    /// Compare one terminal recovered-Decision Apply replay envelope with the
    /// exact full Kura finality artifact retained by CompleteTip recovery.
    ///
    /// This is comparison-only authority. It releases neither the persisted
    /// WAL locator nor the certificate and accepts only the canonical
    /// `ApplyDecision` BodyFrame family for the same context, subject, and
    /// complete CommitQC.
    pub(in crate::sumeragi) fn exactly_matches_complete_tip_finality(
        &self,
        context: &wire::HeightContext,
        subject: &wire::BlockSubject,
        certificate: &wire::QuorumCertificate,
    ) -> bool {
        let Some((_locator, tag, retained, body_frame)) = recovered_decision_apply_parts(self)
        else {
            return false;
        };
        self.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
            && context.id() == certificate.round.context_id
            && context.height == certificate.round.height
            && tag.height == context.height
            && retained == certificate
            && retained.phase == wire::GlobalPhase::Commit
            && retained.subject == *subject
            && body_frame.matches_origin(
                LifecycleContext::new(digest_from_bytes(context.id().0.as_ref()), context.height),
                certificate.proposal_round,
                *subject,
            )
    }

    /// Rebind a persisted Fetch authority only when it is the canonical
    /// BodyFrame-backed Certified family used by the Ready completion.
    fn recover_durable_certified_fetch(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
    ) -> Option<CertifiedFetchReplayEvidenceV1> {
        if stage
            != LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent)
            || !matches!(payload, DurablePayloadReference::BodyFrame(_))
            || self
                .validate_record(
                    active_context,
                    key,
                    LifecycleWorkClass::Fetch,
                    stage,
                    payload,
                )
                .is_err()
        {
            return None;
        }
        let (
            LifecycleReplaySourceV1::BodyPipeline(source),
            ReplayPayloadBindingV1::BodyFrame(body_frame),
        ) = (&self.source, &self.payload)
        else {
            return None;
        };
        let family = CertifiedBodyPipelineReplayFamilyV1 {
            source: source.clone(),
            body_frame: *body_frame,
        };
        (family.is_exact_for_stage(LifecycleStageKind::FetchBody)
            && body_frame.durable_reference()
                == match payload {
                    DurablePayloadReference::BodyFrame(reference) => reference,
                    _ => unreachable!("BodyFrame payload checked above"),
                })
        .then_some(CertifiedFetchReplayEvidenceV1 { family })
    }

    /// Decode exactly one bounded canonical V1 envelope.
    fn decode_canonical(encoded: &[u8]) -> Result<Self, ReplayAuthorityCodecError> {
        if encoded.is_empty() || encoded.len() > MAX_REPLAY_AUTHORITY_BYTES {
            return Err(ReplayAuthorityCodecError::FrameBounds);
        }
        let mut cursor = encoded;
        let authority = Self::decode_all(&mut cursor)
            .map_err(|_| ReplayAuthorityCodecError::InvalidEncoding)?;
        if authority.encode() != encoded {
            return Err(ReplayAuthorityCodecError::NonCanonicalEncoding);
        }
        if authority.format_version != REPLAY_AUTHORITY_FORMAT_VERSION {
            return Err(ReplayAuthorityCodecError::UnsupportedVersion);
        }
        Ok(authority)
    }

    /// Match all durable record coordinates without minting replay authority.
    pub(super) fn structurally_matches_record(
        &self,
        context: LifecycleContext,
        key: LifecycleKey,
        work_class: LifecycleWorkClass,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
    ) -> bool {
        self.validate_record(context, key, work_class, stage, payload)
            .is_ok()
    }

    /// Rebind an already-retained Certified-Serve storage source to the exact
    /// terminal payload admitted by the coordinator's authenticated receipt
    /// boundary. The result remains inert persisted evidence.
    pub(super) fn terminalized_certified_serve(
        &self,
        context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
    ) -> Option<Self> {
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &self.source else {
            return None;
        };
        if !self
            .payload
            .durable_payload()?
            .same_admission_material(payload)
        {
            return None;
        }
        let candidate = Self {
            format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
            payload: ReplayPayloadBindingV1::from_payload(payload),
            source: LifecycleReplaySourceV1::CertifiedServeStorage(source.clone()),
        };
        candidate
            .structurally_matches_record(
                context,
                key,
                LifecycleWorkClass::CertifiedServe,
                stage,
                payload,
            )
            .then_some(candidate)
    }

    /// Return whether two authorities retain the same exact persisted source family.
    pub(super) fn same_persisted_family(&self, other: &Self) -> bool {
        self.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
            && other.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
            && self.source == other.source
    }

    /// Test whether this authority retains one exact Certified-Serve frame hash.
    pub(super) fn certified_serve_frame_hash_is(&self, expected: Hash) -> bool {
        matches!(
            &self.source,
            LifecycleReplaySourceV1::CertifiedServeStorage(source)
                if source.payload_hash == *expected.as_ref()
        )
    }

    /// Match the complete Certified-Serve storage origin against one sealed
    /// authenticated request and admission receipt.
    ///
    /// This comparison deliberately binds the local retainer as well as the
    /// request and frame identities. A valid quorum signer cannot therefore
    /// splice its own retention receipt onto another validator's terminal
    /// replay family.
    pub(super) fn exactly_matches_certified_serve_publication(
        &self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        receipt: DurableCertifiedServeAdmissionReceipt,
    ) -> bool {
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &self.source else {
            return false;
        };
        self.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
            && receipt.exactly_matches_authenticated_coordinates(authenticated)
            && source.request == *authenticated.request()
            && HashOf::new(&source.request) == authenticated.request_hash()
            && source.payload_hash == *receipt.payload_hash().as_ref()
            && source.local_retainer == receipt.local_retainer()
    }

    /// Match the signed request retained by one Certified-Serve storage
    /// family without accepting a separately supplied payload receipt.
    ///
    /// Live terminal settlement uses this before writing its terminal frame,
    /// so a foreign request fails before the payload store is mutated.
    pub(super) fn exactly_matches_certified_serve_request(
        &self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &self.source else {
            return false;
        };
        self.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
            && source.request == *authenticated.request()
            && HashOf::new(&source.request) == authenticated.request_hash()
    }

    #[cfg(test)]
    /// Replace only the Certified-Serve frame hash in a negative test fixture.
    pub(super) fn with_certified_serve_frame_hash_for_test(
        &self,
        frame_hash: Hash,
    ) -> Option<Self> {
        let mut changed = self.clone();
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut changed.source else {
            return None;
        };
        source.payload_hash = *frame_hash.as_ref();
        Some(changed)
    }

    #[cfg(test)]
    /// Change only an origin tag generation while preserving logical coordinates.
    pub(super) fn with_foreign_origin_generation_for_test(&self) -> Option<Self> {
        let mut changed = self.clone();
        let tag = match &mut changed.source {
            LifecycleReplaySourceV1::Wal(source) => &mut source.tag,
            LifecycleReplaySourceV1::BodyPipeline(source) => &mut source.tag,
            _ => return None,
        };
        tag.generation = tag.generation.wrapping_add(1);
        (changed != *self && changed.is_bounded_canonical()).then_some(changed)
    }

    /// Return whether this value has one bounded canonical V1 encoding.
    ///
    /// LedgerV1 decodes the envelope as a nested field, so its outer frame
    /// bound cannot stand in for the tighter per-authority bound. Structural
    /// record validation calls this oracle for both freshly constructed and
    /// nested-decoded values.
    fn is_bounded_canonical(&self) -> bool {
        let encoded = self.encode();
        encoded.len() <= MAX_REPLAY_AUTHORITY_BYTES
            && Self::decode_canonical(&encoded).is_ok_and(|decoded| decoded == *self)
    }

    fn validate_record(
        &self,
        context: LifecycleContext,
        key: LifecycleKey,
        work_class: LifecycleWorkClass,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
    ) -> Result<(), ReplayAuthorityValidationError> {
        if self.format_version != REPLAY_AUTHORITY_FORMAT_VERSION {
            return Err(ReplayAuthorityValidationError::UnsupportedVersion);
        }
        if !self.is_bounded_canonical() {
            return Err(ReplayAuthorityValidationError::InvalidEncoding);
        }
        if !self.payload.matches(payload) {
            return Err(ReplayAuthorityValidationError::PayloadMismatch);
        }
        let expected = self.source.project(context, stage.kind(), &self.payload)?;
        if expected.key != key
            || expected.work_class != work_class
            || expected.stage_kind != stage.kind()
            || !work_class.accepts_stage(key.phase(), stage)
        {
            return Err(ReplayAuthorityValidationError::RecordMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayAuthorityCodecError {
    FrameBounds,
    InvalidEncoding,
    NonCanonicalEncoding,
    UnsupportedVersion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayAuthorityValidationError {
    UnsupportedVersion,
    InvalidEncoding,
    InvalidSource,
    PayloadMismatch,
    RecordMismatch,
}

/// Fixed scalar projection of the process-local reducer tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct ReplayEventTagV1 {
    height: u64,
    view: u64,
    generation: u64,
}

impl ReplayEventTagV1 {
    const fn new(height: u64, view: u64, generation: u64) -> Self {
        Self {
            height,
            view,
            generation,
        }
    }

    fn matches_round(self, context: LifecycleContext, round: wire::ConsensusRound) -> bool {
        round_matches_context(context, round)
            && self.height == context.height()
            && self.view >= round.view
    }

    const fn generation(self) -> u64 {
        self.generation
    }
}

/// Fixed scalar code for the WAL record that owns a replay action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[repr(transparent)]
struct ReplayWalRoleV1(u8);

impl ReplayWalRoleV1 {
    const PROPOSAL_INTENT: Self = Self(0);
    const PREPARE_INTENT: Self = Self(1);
    const LOCK_AND_COMMIT: Self = Self(2);
    const TIMEOUT_INTENT: Self = Self(3);
    const DECISION: Self = Self(4);
    const INSTALL_TIMEOUT: Self = Self(5);

    const fn matches(self, expected: Self) -> bool {
        self.0 == expected.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum LifecycleReplaySourceV1 {
    #[codec(index = 0)]
    Wal(WalReplaySourceV1),
    #[codec(index = 1)]
    ConsensusBroadcast(wire::ConsensusMessageV2),
    #[codec(index = 2)]
    BodyPipeline(BodyPipelineReplaySourceV1),
    #[codec(index = 3)]
    Equivocation(wire::SumeragiV2Equivocation),
    #[codec(index = 4)]
    InvalidCertifiedBody(InvalidBodyReplaySourceV1),
    #[codec(index = 5)]
    CertifiedServeStorage(CertifiedServeStorageSourceV1),
}

impl LifecycleReplaySourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        match self {
            Self::Wal(source) => source.project(context, requested_stage, payload),
            Self::ConsensusBroadcast(message) => {
                project_broadcast(context, message, requested_stage, payload)
            }
            Self::BodyPipeline(source) => source.project(context, requested_stage, payload),
            Self::Equivocation(evidence) => {
                project_equivocation(context, evidence, requested_stage, payload)
            }
            Self::InvalidCertifiedBody(source) => source.project(context, requested_stage, payload),
            Self::CertifiedServeStorage(source) => {
                source.project(context, requested_stage, payload)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct WalReplaySourceV1 {
    locator: PersistedWalFrameLocatorV1,
    role: ReplayWalRoleV1,
    tag: ReplayEventTagV1,
    action: WalReplayActionV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum WalReplayActionV1 {
    #[codec(index = 0)]
    SignProposal(wire::Proposal),
    #[codec(index = 1)]
    SignVote(wire::Vote),
    #[codec(index = 2)]
    SignTimeoutVote(wire::TimeoutVote),
    #[codec(index = 3)]
    ApplyDecision(wire::QuorumCertificate),
    #[codec(index = 4)]
    EnterView {
        certificate: wire::TimeoutCertificate,
        protected_lock: Option<wire::QuorumCertificate>,
    },
    #[codec(index = 5)]
    FetchDecision {
        certificate: wire::QuorumCertificate,
        certified_sources: Vec<PeerId>,
    },
}

/// Canonical structural evidence attached to one authenticated recovered WAL vote.
///
/// This value is deliberately cloneable because it is inert evidence, not
/// executable authority. Its fields and encoded bytes remain private; callers
/// can only compare it with the exact opaque WAL identity, reducer tag, and
/// unsigned vote retained by the sealed recovery chain.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "recovered WAL replay evidence must remain attached to its sealed recovery chain"]
pub(crate) struct RecoveredWalVoteReplayEvidenceV1 {
    authority: LifecycleReplayAuthorityV1,
}

/// Canonical structural evidence for a recovered ProposalIntent or TimeoutIntent Sign.
///
/// This is inert, cloneable evidence; executable authority remains in the
/// non-clone recovered-frame token. Its source, action, locator, and encoded
/// bytes have no extraction API.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "recovered control replay evidence must remain attached to its WAL token"]
pub(crate) struct RecoveredWalControlReplayEvidenceV1 {
    authority: LifecycleReplayAuthorityV1,
}

/// Canonical structural evidence for a recovered Decision-owned certified Fetch.
///
/// The exact CommitQC and frozen ordered archive roster are persisted in the
/// private replay envelope. Executable authority remains in the non-clone WAL
/// token and the runtime-private pending/candidate projection.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "recovered Decision Fetch evidence must remain attached to its WAL token"]
pub(crate) struct RecoveredWalDecisionFetchReplayEvidenceV1 {
    authority: LifecycleReplayAuthorityV1,
}

/// Canonical body-backed lineage for one recovered Commit Decision.
///
/// The original payload-free Fetch authority remains unchanged. This seal
/// instead retains the exact WAL locator/QC-bound Store/Validate family and
/// the final `ApplyDecision` authority, both bound to one immutable BodyFrame.
/// It is inert evidence: only the private recovered-Decision composite may
/// project candidates from it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "recovered Decision body replay lineage must remain sealed through Apply recovery"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyReplayLineageV1 {
    fetch: LifecycleReplayAuthorityV1,
    body: RecoveredDecisionBodyPipelineReplayFamilyV1,
    apply: LifecycleReplayAuthorityV1,
}

/// Closed logical Store/Validate/Apply lineage derived by the fixed reducer preview.
///
/// The three candidates and their concrete bindings remain private. Ledger,
/// recovery, and registry code may only use the fixed record/splice/comparison
/// oracles below; no candidate or replay-authority parts accessor exists.
#[must_use = "recovered Decision Apply lineage must enter exact storage publication"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyCandidateLineageV1 {
    fetch: LifecycleReplayAuthorityV1,
    store: CandidateAdmission,
    validate: CandidateAdmission,
    apply: CandidateAdmission,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecoveredDecisionBodyPipelineReplayFamilyV1 {
    source: BodyPipelineReplaySourceV1,
    body_frame: BodyFrameBindingV1,
}

/// One-shot candidate projection minted by consuming the runtime permit.
///
/// The logical admission remains opaque outside the lifecycle recovery
/// module. In particular, this value has no candidate, key, ordinal, replay,
/// geometry, or parts accessor.
#[must_use = "the recovered control candidate projection must enter its sealed WAL recovery"]
pub(in crate::sumeragi) struct RecoveredWalControlCandidateProjectionV1 {
    candidate: CandidateAdmission,
}

/// One-shot candidate projection for an exact recovered Decision Fetch.
///
/// This wrapper exposes no candidate parts and can only be consumed by the
/// dedicated recovered-WAL storage carrier.
#[must_use = "the recovered Decision Fetch candidate must enter sealed WAL recovery"]
pub(in crate::sumeragi) struct RecoveredWalDecisionFetchCandidateProjectionV1 {
    candidate: CandidateAdmission,
}

impl RecoveredWalControlCandidateProjectionV1 {
    /// Consume the opaque projection inside the sealed WAL-recovery module.
    pub(super) fn into_candidate(self) -> CandidateAdmission {
        self.candidate
    }
}

impl RecoveredWalDecisionFetchCandidateProjectionV1 {
    /// Consume the opaque projection inside the sealed WAL-recovery module.
    pub(super) fn into_candidate(self) -> CandidateAdmission {
        self.candidate
    }
}

impl RecoveredWalControlReplayEvidenceV1 {
    /// Mint the canonical V1 authority for one already-authenticated control effect.
    pub(crate) fn from_sealed_recovered_control(
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        let authority = exact_recovered_wal_control_authority(locator, effect)?;
        let canonical = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode()).ok()?;
        (canonical == authority).then_some(Self {
            authority: canonical,
        })
    }

    /// Compare the complete canonical authority with one opaque frame and effect.
    pub(crate) fn exactly_matches_recovered_control(
        &self,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> bool {
        exact_recovered_wal_control_authority(locator, effect)
            .is_some_and(|expected| expected == self.authority)
            && LifecycleReplayAuthorityV1::decode_canonical(&self.authority.encode())
                .is_ok_and(|canonical| canonical == self.authority)
    }

    /// Consume the one-shot runtime permit into one opaque control candidate.
    pub(in crate::sumeragi) fn project_recovered_control_candidate(
        &self,
        _permit: RecoveredWalCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<RecoveredWalControlCandidateProjectionV1> {
        self.project_candidate(verified, locator, effect, pending)
            .map(|candidate| RecoveredWalControlCandidateProjectionV1 { candidate })
    }

    /// Recompute the admission and compare it without releasing either value.
    pub(in crate::sumeragi) fn project_recovered_control_candidate_for_comparison(
        &self,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        expected: &CandidateAdmission,
    ) -> bool {
        self.project_candidate(verified, locator, effect, pending)
            .is_some_and(|candidate| candidate == *expected)
    }

    fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<CandidateAdmission> {
        if !self.exactly_matches_recovered_control(locator, effect) {
            return None;
        }
        let active_context = super::projection::lifecycle_context(verified.context());
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            effect,
            pending,
        )
        .ok()?;
        candidate_from_authorized_projection(
            active_context,
            projected,
            DurablePayloadReference::None,
            self.authority.clone(),
        )
    }
}

impl RecoveredWalDecisionFetchReplayEvidenceV1 {
    /// Mint canonical V1 evidence for one authenticated Decision Fetch.
    pub(crate) fn from_sealed_recovered_decision_fetch(
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        let authority = exact_recovered_wal_decision_fetch_authority(verified, locator, effect)?;
        let canonical = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode()).ok()?;
        (canonical == authority).then_some(Self {
            authority: canonical,
        })
    }

    /// Compare the complete canonical authority with one verified frame/effect pair.
    pub(crate) fn exactly_matches_recovered_decision_fetch(
        &self,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> bool {
        exact_recovered_wal_decision_fetch_authority(verified, locator, effect)
            .is_some_and(|expected| expected == self.authority)
            && LifecycleReplayAuthorityV1::decode_canonical(&self.authority.encode())
                .is_ok_and(|canonical| canonical == self.authority)
    }

    /// Consume the private runtime permits into one opaque Fetch candidate.
    pub(in crate::sumeragi) fn project_recovered_decision_fetch_candidate(
        &self,
        _permit: RecoveredWalCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<RecoveredWalDecisionFetchCandidateProjectionV1> {
        self.project_candidate(verified, locator, effect, pending)
            .map(|candidate| RecoveredWalDecisionFetchCandidateProjectionV1 { candidate })
    }

    /// Recompute and compare the candidate without releasing either value.
    pub(in crate::sumeragi) fn project_recovered_decision_fetch_candidate_for_comparison(
        &self,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        expected: &CandidateAdmission,
    ) -> bool {
        self.project_candidate(verified, locator, effect, pending)
            .is_some_and(|candidate| candidate == *expected)
    }

    fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<CandidateAdmission> {
        if !self.exactly_matches_recovered_decision_fetch(verified, locator, effect) {
            return None;
        }
        let active_context = super::projection::lifecycle_context(verified.context());
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            effect,
            pending,
        )
        .ok()?;
        candidate_from_authorized_projection(
            active_context,
            projected,
            DurablePayloadReference::None,
            self.authority.clone(),
        )
    }

    /// Reconstruct the pending binding only behind the private one-shot permit.
    pub(in crate::sumeragi) fn reconstruct_pending(
        &self,
        permit: RecoveredWalDecisionFetchPendingMintPermit,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<PendingRuntimeEffectBinding> {
        if !self.exactly_matches_recovered_decision_fetch(verified, locator, effect) {
            return None;
        }
        PendingRuntimeEffectBinding::from_exact_recovered_wal_decision_fetch(
            permit, locator, effect,
        )
    }
}

impl RecoveredDecisionApplyReplayLineageV1 {
    /// Derive the closed body lineage from one exact recovered Decision Fetch.
    ///
    /// The caller cannot supply a role, action, locator parts, lifecycle key,
    /// or encoded replay envelope. The already-authenticated Fetch evidence
    /// fixes the Decision frame/QC, while the exact durable receipt and its
    /// manifest fix the sole BodyFrame accepted by Store, Validate, and Apply.
    pub(crate) fn from_sealed_recovered_decision(
        fetch: &RecoveredWalDecisionFetchReplayEvidenceV1,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        fetch_effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
    ) -> Option<Self> {
        let expected_fetch =
            exact_recovered_wal_decision_fetch_authority(verified, locator, fetch_effect)?;
        if fetch.authority != expected_fetch
            || LifecycleReplayAuthorityV1::decode_canonical(&fetch.authority.encode()).ok()?
                != fetch.authority
        {
            return None;
        }
        let AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            certificate: Some(certificate),
            ..
        } = fetch_effect
        else {
            return None;
        };
        let context = super::projection::lifecycle_context(verified.context());
        if certificate.phase != wire::GlobalPhase::Commit
            || certificate.proposal_round != *round
            || certificate.subject != *subject
            || manifest.round != *round
            || manifest.subject != *subject
            || receipt.context_id() != round.context_id
            || receipt.round() != *round
            || receipt.subject() != *subject
            || receipt.manifest_hash() != HashOf::new(manifest)
        {
            return None;
        }
        let frame = durable_body_frame_reference(context, receipt)?;
        let ReplayPayloadBindingV1::BodyFrame(body_frame) =
            ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame))
        else {
            unreachable!("one durable body receipt projects one BodyFrame binding")
        };
        let body = RecoveredDecisionBodyPipelineReplayFamilyV1 {
            source: BodyPipelineReplaySourceV1 {
                tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
                origin: BodyPipelineOriginV1::RecoveredDecision {
                    locator: locator.persisted_locator(),
                    certificate: certificate.clone(),
                    manifest: manifest.clone(),
                },
            },
            body_frame,
        };
        body.authority_for(context, LifecycleStageKind::StoreBody)?;
        body.authority_for(context, LifecycleStageKind::ValidateBody)?;

        let apply_source = LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator: locator.persisted_locator(),
            role: ReplayWalRoleV1::DECISION,
            tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
            action: WalReplayActionV1::ApplyDecision(certificate.clone()),
        });
        let apply = canonical_replay_authority(
            context,
            apply_source,
            LifecycleStageKind::ApplyDecision,
            ReplayPayloadBindingV1::BodyFrame(body_frame),
        )?;
        let lineage = Self {
            fetch: fetch.authority.clone(),
            body,
            apply,
        };
        lineage.is_stage_closed(context).then_some(lineage)
    }

    fn is_stage_closed(&self, context: LifecycleContext) -> bool {
        let Some(store) = self
            .body
            .authority_for(context, LifecycleStageKind::StoreBody)
        else {
            return false;
        };
        let Some(validate) = self
            .body
            .authority_for(context, LifecycleStageKind::ValidateBody)
        else {
            return false;
        };
        let Some(fetch_shape) = self
            .fetch
            .source
            .project(context, LifecycleStageKind::FetchBody, &self.fetch.payload)
            .ok()
        else {
            return false;
        };
        let Some(apply_shape) = self
            .apply
            .source
            .project(
                context,
                LifecycleStageKind::ApplyDecision,
                &self.apply.payload,
            )
            .ok()
        else {
            return false;
        };
        let Some(store_payload) = store.payload.durable_payload() else {
            return false;
        };
        let Some(validate_payload) = validate.payload.durable_payload() else {
            return false;
        };
        let Some(apply_payload) = self.apply.payload.durable_payload() else {
            return false;
        };
        fetch_shape.work_class == LifecycleWorkClass::Fetch
            && apply_shape.work_class == LifecycleWorkClass::Apply
            && recovered_decision_body_continuation_is_exact(
                super::schema::DurableContinuationEdge::FetchToStore,
                &self.fetch,
                DurablePayloadReference::None,
                &store,
                store_payload,
            ) == Some(true)
            && recovered_decision_body_continuation_is_exact(
                super::schema::DurableContinuationEdge::StoreToValidate,
                &store,
                store_payload,
                &validate,
                validate_payload,
            ) == Some(true)
            && recovered_decision_body_continuation_is_exact(
                super::schema::DurableContinuationEdge::ValidateToApply,
                &validate,
                validate_payload,
                &self.apply,
                apply_payload,
            ) == Some(true)
    }

    /// Consume the inert replay family into the sole fixed reducer-derived
    /// Store/Validate/Apply logical lineage.
    ///
    /// The one-shot permit is minted only while the staged adapter still owns
    /// all three exact effects and predecessor-derived bindings. The returned
    /// value exposes only fixed ledger/recovery/registry oracles.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn project_candidate_lineage(
        &self,
        _permit: RecoveredDecisionApplyCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        receipt: &DurableBodyReceipt,
        store_effect: &AdapterEffect,
        store_pending: &PendingRuntimeEffectBinding,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        apply_effect: &AdapterEffect,
        apply_pending: &PendingRuntimeEffectBinding,
    ) -> Option<RecoveredDecisionApplyCandidateLineageV1> {
        let context = super::projection::lifecycle_context(verified.context());
        if receipt.context_id() != verified.context().id() || !self.is_stage_closed(context) {
            return None;
        }
        let payload =
            DurablePayloadReference::BodyFrame(durable_body_frame_reference(context, receipt)?);
        let store_authority = self
            .body
            .authority_for(context, LifecycleStageKind::StoreBody)?;
        let validate_authority = self
            .body
            .authority_for(context, LifecycleStageKind::ValidateBody)?;
        let store = candidate_from_authorized_projection(
            context,
            super::projection::authority_free_admission_projection(
                context,
                verified,
                store_effect,
                store_pending,
            )
            .ok()?,
            payload,
            store_authority,
        )?;
        let validate = candidate_from_authorized_projection(
            context,
            super::projection::authority_free_admission_projection(
                context,
                verified,
                validate_effect,
                validate_pending,
            )
            .ok()?,
            payload,
            validate_authority,
        )?;
        let apply = candidate_from_authorized_projection(
            context,
            super::projection::authority_free_admission_projection(
                context,
                verified,
                apply_effect,
                apply_pending,
            )
            .ok()?,
            payload,
            self.apply.clone(),
        )?;
        let lineage = RecoveredDecisionApplyCandidateLineageV1 {
            fetch: self.fetch.clone(),
            store,
            validate,
            apply,
        };
        lineage.is_exact(context).then_some(lineage)
    }
}

impl RecoveredDecisionApplyCandidateLineageV1 {
    fn candidate_matches_record(
        candidate: &CandidateAdmission,
        record: &LifecycleLedgerRecordV1,
        owner: OwnerId,
        terminal: Option<TerminalOutcome>,
        continuation: super::schema::DurableContinuation,
    ) -> bool {
        record.key() == Some(candidate.key)
            && record.owner() == owner
            && record.work_class() == Some(candidate.work_class)
            && record.stage() == Some(candidate.stage)
            && record.terminal() == Some(terminal)
            && record.reconstruction_source() == candidate.reconstruction_source
            && record.durable_payload() == Some(candidate.payload)
            && record.continuation() == Some(continuation)
            && record.replay_matches_candidate(candidate)
    }

    /// Recheck the complete fixed body lineage without releasing a candidate.
    pub(super) fn is_exact(&self, context: LifecycleContext) -> bool {
        let candidates = [&self.store, &self.validate, &self.apply];
        let owner = self.store.causal_root;
        let payload = self.store.payload;
        candidates.iter().all(|candidate| {
            candidate.causal_root == owner
                && candidate.reconstruction_source == owner.digest()
                && candidate.initial_state == InitialLifecycleState::Ready
                && candidate.payload == payload
                && candidate.producer_turn.is_none()
                && candidate.replay_authority_is_exact(context)
        }) && self.store.work_class == LifecycleWorkClass::Store
            && self.store.stage.kind() == LifecycleStageKind::StoreBody
            && self.validate.work_class == LifecycleWorkClass::Validate
            && self.validate.stage.kind() == LifecycleStageKind::ValidateBody
            && self.apply.work_class == LifecycleWorkClass::Apply
            && self.apply.stage.kind() == LifecycleStageKind::ApplyDecision
            && matches!(payload, DurablePayloadReference::BodyFrame(_))
            && super::body_pipeline_transition::durable_continuation_successor_is_exact(
                super::schema::DurableContinuationEdge::StoreToValidate,
                self.store.work_class,
                self.store.key,
                self.store.stage,
                self.validate.work_class,
                self.validate.key,
                self.validate.stage,
            )
            && super::body_pipeline_transition::durable_continuation_successor_is_exact(
                super::schema::DurableContinuationEdge::ValidateToApply,
                self.validate.work_class,
                self.validate.key,
                self.validate.stage,
                self.apply.work_class,
                self.apply.key,
                self.apply.stage,
            )
            && recovered_decision_body_continuation_is_exact(
                super::schema::DurableContinuationEdge::StoreToValidate,
                &self.store.replay_authority,
                payload,
                &self.validate.replay_authority,
                payload,
            ) == Some(true)
            && recovered_decision_body_continuation_is_exact(
                super::schema::DurableContinuationEdge::ValidateToApply,
                &self.validate.replay_authority,
                payload,
                &self.apply.replay_authority,
                payload,
            ) == Some(true)
    }

    /// Bind the original payload-free Decision Fetch authority to the first
    /// body-backed Store successor without releasing either candidate.
    pub(super) fn exactly_follows_fetch_candidate(&self, fetch: &CandidateAdmission) -> bool {
        fetch.replay_authority == self.fetch
            && fetch.work_class == LifecycleWorkClass::Fetch
            && fetch.stage.kind() == LifecycleStageKind::FetchBody
            && fetch.payload == DurablePayloadReference::None
            && fetch.causal_root == self.store.causal_root
            && fetch.reconstruction_source == self.store.reconstruction_source
            && super::body_pipeline_transition::durable_continuation_successor_is_exact(
                super::schema::DurableContinuationEdge::FetchToStore,
                fetch.work_class,
                fetch.key,
                fetch.stage,
                self.store.work_class,
                self.store.key,
                self.store.stage,
            )
            && recovered_decision_body_continuation_is_exact(
                super::schema::DurableContinuationEdge::FetchToStore,
                &fetch.replay_authority,
                fetch.payload,
                &self.store.replay_authority,
                self.store.payload,
            ) == Some(true)
    }

    /// Build the only three durable successors permitted after the exact
    /// payload-free recovered Decision Fetch row.
    pub(super) fn successor_records(
        &self,
        owner: OwnerId,
        store_ordinal: u128,
        validate_ordinal: u128,
        apply_ordinal: u128,
    ) -> Option<[LifecycleLedgerRecordV1; 3]> {
        if owner.causal_root() != self.store.causal_root
            || store_ordinal.checked_add(1) != Some(validate_ordinal)
            || validate_ordinal.checked_add(1) != Some(apply_ordinal)
        {
            return None;
        }
        let store = LifecycleLedgerRecordV1::new(
            self.store.key,
            owner,
            store_ordinal,
            self.store.work_class,
            self.store.stage,
            Some(TerminalOutcome::Advanced),
            self.store.reconstruction_source,
            self.store.payload,
            self.store.replay_authority.clone(),
            super::schema::DurableContinuation::successor(
                super::schema::DurableContinuationEdge::StoreToValidate,
                validate_ordinal,
            ),
        )
        .ok()?;
        let validate = LifecycleLedgerRecordV1::new(
            self.validate.key,
            owner,
            validate_ordinal,
            self.validate.work_class,
            self.validate.stage,
            Some(TerminalOutcome::Advanced),
            self.validate.reconstruction_source,
            self.validate.payload,
            self.validate.replay_authority.clone(),
            super::schema::DurableContinuation::successor(
                super::schema::DurableContinuationEdge::ValidateToApply,
                apply_ordinal,
            ),
        )
        .ok()?;
        let apply = LifecycleLedgerRecordV1::new(
            self.apply.key,
            owner,
            apply_ordinal,
            self.apply.work_class,
            self.apply.stage,
            None,
            self.apply.reconstruction_source,
            self.apply.payload,
            self.apply.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        )
        .ok()?;
        Some([store, validate, apply])
    }

    /// Compare all three successor rows, including the final live Apply.
    pub(super) fn exactly_matches_successor_records(
        &self,
        owner: OwnerId,
        store: &LifecycleLedgerRecordV1,
        validate: &LifecycleLedgerRecordV1,
        apply: &LifecycleLedgerRecordV1,
    ) -> bool {
        store.ordinal().checked_add(1) == Some(validate.ordinal())
            && validate.ordinal().checked_add(1) == Some(apply.ordinal())
            && Self::candidate_matches_record(
                &self.store,
                store,
                owner,
                Some(TerminalOutcome::Advanced),
                super::schema::DurableContinuation::successor(
                    super::schema::DurableContinuationEdge::StoreToValidate,
                    validate.ordinal(),
                ),
            )
            && Self::candidate_matches_record(
                &self.validate,
                validate,
                owner,
                Some(TerminalOutcome::Advanced),
                super::schema::DurableContinuation::successor(
                    super::schema::DurableContinuationEdge::ValidateToApply,
                    apply.ordinal(),
                ),
            )
            && Self::candidate_matches_record(
                &self.apply,
                apply,
                owner,
                None,
                super::schema::DurableContinuation::None,
            )
    }

    /// Compare the complete recovered body chain after its Apply child was
    /// durably terminalized.
    ///
    /// This remains separate from [`Self::exactly_matches_successor_records`]:
    /// startup must never turn a terminal Apply back into a live carrier. The
    /// caller still has to authenticate the exact Kura artifact and receipt
    /// before this ledger shape can authorize predecessor retirement.
    pub(super) fn exactly_matches_terminal_successor_records(
        &self,
        owner: OwnerId,
        store: &LifecycleLedgerRecordV1,
        validate: &LifecycleLedgerRecordV1,
        apply: &LifecycleLedgerRecordV1,
    ) -> bool {
        store.ordinal().checked_add(1) == Some(validate.ordinal())
            && validate.ordinal().checked_add(1) == Some(apply.ordinal())
            && Self::candidate_matches_record(
                &self.store,
                store,
                owner,
                Some(TerminalOutcome::Advanced),
                super::schema::DurableContinuation::successor(
                    super::schema::DurableContinuationEdge::StoreToValidate,
                    validate.ordinal(),
                ),
            )
            && Self::candidate_matches_record(
                &self.validate,
                validate,
                owner,
                Some(TerminalOutcome::Advanced),
                super::schema::DurableContinuation::successor(
                    super::schema::DurableContinuationEdge::ValidateToApply,
                    apply.ordinal(),
                ),
            )
            && Self::candidate_matches_record(
                &self.apply,
                apply,
                owner,
                Some(TerminalOutcome::Advanced),
                super::schema::DurableContinuation::None,
            )
    }

    /// Insert only the final live Apply candidate after the complete durable
    /// body chain has been proven exact.
    pub(super) fn splice_apply_candidate_from_records(
        &self,
        owner: OwnerId,
        store: &LifecycleLedgerRecordV1,
        validate: &LifecycleLedgerRecordV1,
        apply: &LifecycleLedgerRecordV1,
        candidates: &mut std::collections::BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        self.exactly_matches_successor_records(owner, store, validate, apply)
            && !candidates.contains_key(&self.apply.key)
            && candidates
                .insert(self.apply.key, self.apply.clone())
                .is_none()
    }

    /// Compare one reconstructed logical candidate with the sole live Apply.
    pub(in crate::sumeragi) fn exactly_matches_apply_candidate(
        &self,
        candidate: &CandidateAdmission,
    ) -> bool {
        candidate == &self.apply
    }

    /// Bind the retained validation result to this lineage's exact body frame.
    ///
    /// The receipt remains opaque to registry callers.  This fixed comparison
    /// prevents a valid commitment for a different durable body from being
    /// paired with the recovered Decision Apply carrier.
    pub(in crate::sumeragi) fn exactly_matches_validated_receipt(
        &self,
        context: LifecycleContext,
        receipt: &ValidatedBodyReceipt,
    ) -> bool {
        durable_body_frame_reference(context, receipt.durable()).is_some_and(|frame| {
            self.is_exact(context)
                && self.apply.payload == DurablePayloadReference::BodyFrame(frame)
        })
    }

    /// Confirm that recovery retained the exact Apply and no substituted value.
    pub(super) fn owns_spliced_apply_candidate(
        &self,
        candidates: &std::collections::BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates.get(&self.apply.key) == Some(&self.apply)
    }
}

impl RecoveredDecisionBodyPipelineReplayFamilyV1 {
    fn authority_for(
        &self,
        context: LifecycleContext,
        stage: LifecycleStageKind,
    ) -> Option<LifecycleReplayAuthorityV1> {
        if !matches!(
            stage,
            LifecycleStageKind::StoreBody | LifecycleStageKind::ValidateBody
        ) {
            return None;
        }
        canonical_replay_authority(
            context,
            LifecycleReplaySourceV1::BodyPipeline(self.source.clone()),
            stage,
            ReplayPayloadBindingV1::BodyFrame(self.body_frame),
        )
    }
}

impl RecoveredWalVoteReplayEvidenceV1 {
    /// Build canonical evidence for one already-authenticated recovered phase vote.
    ///
    /// The caller supplies no role, action, locator parts, lifecycle key, or
    /// encoded bytes. Prepare versus `LockAndCommit` ownership is derived from
    /// the exact unsigned vote, while the locator remains the opaque WAL
    /// identity minted by verified append or recovery.
    pub(crate) fn from_sealed_recovered_vote(
        locator: RecoveredWalFrameIdentity,
        tag: EventTag,
        vote: &wire::Vote,
    ) -> Option<Self> {
        let authority = exact_recovered_wal_vote_authority(locator, tag, vote)?;
        let canonical = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode()).ok()?;
        if canonical != authority {
            return None;
        }
        let evidence = Self {
            authority: canonical,
        };
        evidence
            .exactly_matches_recovered_vote(locator, tag, vote)
            .then_some(evidence)
    }

    /// Compare the complete canonical envelope with one sealed recovered vote.
    ///
    /// This is structural equality only. It cannot authenticate decoded WAL
    /// bytes, a signature, or the source of `locator`.
    pub(crate) fn exactly_matches_recovered_vote(
        &self,
        locator: RecoveredWalFrameIdentity,
        tag: EventTag,
        vote: &wire::Vote,
    ) -> bool {
        let LifecycleReplaySourceV1::Wal(source) = &self.authority.source else {
            return false;
        };
        if !source.locator.exactly_matches_runtime(locator) {
            return false;
        }
        let Some(expected) = exact_recovered_wal_vote_authority(locator, tag, vote) else {
            return false;
        };
        self.authority == expected
            && LifecycleReplayAuthorityV1::decode_canonical(&self.authority.encode())
                .is_ok_and(|canonical| canonical == self.authority)
    }

    /// Project the exact recovered WAL vote into one replay-authorized Sign candidate.
    ///
    /// The opaque WAL locator and canonical authority remain inside the sealed
    /// runtime successor. Only this fixed join can attach them to the
    /// authority-free runtime projection.
    pub(in crate::sumeragi) fn project_recovered_vote_candidate(
        &self,
        _permit: RecoveredWalCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        locator: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<CandidateAdmission> {
        let AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(vote),
        } = effect
        else {
            return None;
        };
        if !self.exactly_matches_recovered_vote(locator, *tag, vote) {
            return None;
        }
        let active_context = super::projection::lifecycle_context(verified.context());
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            effect,
            pending,
        )
        .ok()?;
        candidate_from_authorized_projection(
            active_context,
            projected,
            DurablePayloadReference::None,
            self.authority.clone(),
        )
    }
}

/// Non-decodable live authority for one exact fsynced WAL continuation.
///
/// Payload-free stages retain their complete canonical V1 envelope. `Apply`
/// deliberately remains a source-only seal until the closed Validate registry
/// join supplies its store-authenticated body frame. Neither state exposes a
/// locator, action, encoded source, or parts API.
#[derive(PartialEq, Eq)]
#[must_use = "a live WAL replay seal must remain joined to its persisted continuation"]
struct LiveWalPersistedReplaySealV1 {
    wal_identity: LiveWalFrameIdentity,
    state: LiveWalPersistedReplayStateV1,
}

#[derive(PartialEq, Eq)]
enum LiveWalPersistedReplayStateV1 {
    Canonical {
        stage: LifecycleStageKind,
        authority: LifecycleReplayAuthorityV1,
    },
    ApplyPending {
        context: LifecycleContext,
        source: WalReplaySourceV1,
    },
}

/// Exact live WAL continuation kept inseparable from its adapter effect.
///
/// This move-only envelope is the linear transport returned by the adapter's
/// future persisted-continuation cut. It exposes only fixed pending-binding
/// and receipt-bound equality joins; neither the effect nor its replay seal
/// can be extracted.
#[must_use = "a sealed live WAL effect has not entered lifecycle pre-admission"]
pub(in crate::sumeragi) struct SealedLiveWalPersistedEffectV1 {
    effect: AdapterEffect,
    replay: LiveWalPersistedReplaySealV1,
    pending: LiveWalPersistedPendingV1,
}

#[derive(PartialEq, Eq)]
enum LiveWalPersistedPendingV1 {
    PayloadFree(PendingRuntimeEffectBinding),
    ValidateSignBound(PendingRuntimeEffectBinding),
    ApplyPending,
    ApplyBound(PendingRuntimeEffectBinding),
}

impl SealedLiveWalPersistedEffectV1 {
    /// Consume the adapter's one record-checked live continuation cause.
    pub(in crate::sumeragi) fn from_exact_live_append(
        cause: ExactLiveWalPersistedContinuationCause,
    ) -> Option<Self> {
        let (wal_identity, effect, pending) = match cause {
            ExactLiveWalPersistedContinuationCause::PayloadFree {
                wal_identity,
                effect,
                pending,
            } => (
                wal_identity,
                effect,
                LiveWalPersistedPendingV1::PayloadFree(pending),
            ),
            ExactLiveWalPersistedContinuationCause::Apply {
                wal_identity,
                effect,
            } => (
                wal_identity,
                effect,
                LiveWalPersistedPendingV1::ApplyPending,
            ),
        };
        let replay =
            LiveWalPersistedReplaySealV1::from_exact_persisted_effect(wal_identity, &effect)?;
        let sealed = Self {
            effect,
            replay,
            pending,
        };
        sealed.exactly_matches_effect().then_some(sealed)
    }

    /// Recheck the payload-free effect and pending owner minted at conversion.
    pub(super) fn exactly_binds_payload_free_pending(&self) -> bool {
        matches!(
            &self.pending,
            LiveWalPersistedPendingV1::PayloadFree(pending)
                if pending.exactly_binds_adapter_effect(&self.effect)
        ) && self
            .replay
            .exactly_matches_payload_free_effect(&self.effect)
    }

    /// Replace the frame-derived placeholder owner of one exact vote-sign
    /// continuation with the predecessor-derived binding sealed by the Ready
    /// Validate adapter preflight.
    ///
    /// The caller cannot provide a WAL identity or effect. Failure returns both
    /// move-only inputs intact; success keeps the bound pending value nested in
    /// this replay envelope.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_exact_validate_sign_pending(
        self,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, (Self, PendingRuntimeEffectBinding)> {
        if !matches!(&self.pending, LiveWalPersistedPendingV1::PayloadFree(_))
            || !matches!(
                &self.effect,
                AdapterEffect::Sign {
                    request: SignRequest::Vote(vote),
                    ..
                } if matches!(vote.phase, wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit)
            )
            || !pending.exactly_binds_adapter_effect(&self.effect)
            || !self
                .replay
                .exactly_matches_payload_free_effect(&self.effect)
        {
            return Err((self, pending));
        }
        let Self { effect, replay, .. } = self;
        Ok(Self {
            effect,
            replay,
            pending: LiveWalPersistedPendingV1::ValidateSignBound(pending),
        })
    }

    /// Recheck the sealed post-append Validate-to-Sign binding without
    /// releasing its effect or pending owner.
    pub(in crate::sumeragi) fn exactly_binds_validate_sign_pending(&self) -> bool {
        matches!(
            &self.pending,
            LiveWalPersistedPendingV1::ValidateSignBound(pending)
                if pending.exactly_binds_adapter_effect(&self.effect)
        ) && matches!(
            &self.effect,
            AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            } if matches!(vote.phase, wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit)
        ) && self
            .replay
            .exactly_matches_payload_free_effect(&self.effect)
    }

    /// Project the exact replay-authorized Sign child without releasing its
    /// nested effect or predecessor-derived pending owner.
    ///
    /// Only the body-transition module can mint `permit`. In particular this
    /// path does not repeat ordinary-to-Commit refinement after the opaque
    /// registered-Prepare capability was consumed before WAL append.
    pub(in crate::sumeragi) fn project_sealed_validate_sign_candidate(
        &self,
        _permit: &SealedValidateSignProjectionPermit,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_binds_validate_sign_pending() {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let LiveWalPersistedPendingV1::ValidateSignBound(pending) = &self.pending else {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        };
        let LiveWalPersistedReplayStateV1::Canonical { authority, stage } = &self.replay.state
        else {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        };
        if !matches!(
            stage,
            LifecycleStageKind::SignPrepareVote | LifecycleStageKind::SignCommitVote
        ) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = super::projection::lifecycle_context(verified.context());
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            &self.effect,
            pending,
        )?;
        candidate_from_authorized_projection(
            active_context,
            projected,
            DurablePayloadReference::None,
            authority.clone(),
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)
    }

    /// Consume the exact nested Validate-to-Sign continuation into one closed
    /// ordinary registry carrier.
    ///
    /// The caller receives neither effect nor pending parts. The one-shot
    /// permit is minted only after the fixed transaction has staged the exact
    /// child and is ready to reserve its concrete address.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_live_validate_sign_work(
        self,
        permit: LiveValidateSignWorkProjectionPermit,
    ) -> Result<PreparedLiveValidateSignRegistryWork, Self> {
        if !self.exactly_binds_validate_sign_pending() {
            return Err(self);
        }
        let Self {
            effect,
            replay,
            pending,
        } = self;
        let LiveWalPersistedPendingV1::ValidateSignBound(pending) = pending else {
            unreachable!("exact live Validate-to-Sign seal retains its bound pending owner")
        };
        match PreparedLiveValidateSignRegistryWork::from_exact(permit, effect, pending) {
            Ok(work) => Ok(work),
            Err((_error, effect, pending)) => Err(Self {
                effect,
                replay,
                pending: LiveWalPersistedPendingV1::ValidateSignBound(pending),
            }),
        }
    }

    /// Compare the complete test effect and expected inherited causal key
    /// without releasing either sealed value.
    #[cfg(test)]
    pub(in crate::sumeragi) fn exactly_matches_validate_sign_for_test(
        &self,
        effect: &AdapterEffect,
        causal_key: &iroha_crypto::Hash,
    ) -> bool {
        self.effect == *effect
            && matches!(
                &self.pending,
                LiveWalPersistedPendingV1::ValidateSignBound(pending)
                    if pending.causal_lifecycle_key() == causal_key
            )
            && self.exactly_binds_validate_sign_pending()
    }

    /// Complete `Apply` only from the exact retained Validate causal owner.
    ///
    /// TODO: Co-locate this seal with the work-registry join before production
    /// admission so the sibling-visible receipt seam can become private. The
    /// source guard pins its sole production caller in the retained Validate
    /// completion until then.
    #[allow(clippy::result_large_err)]
    pub(super) fn complete_exact_apply(
        self,
        predecessor_effect: &AdapterEffect,
        predecessor_pending: &PendingRuntimeEffectBinding,
        child_pending: PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Result<Self, (Self, PendingRuntimeEffectBinding)> {
        if !matches!(&self.pending, LiveWalPersistedPendingV1::ApplyPending)
            || !predecessor_pending
                .project_validate_apply_successor(predecessor_effect, &self.effect)
                .is_some_and(|expected| expected == child_pending)
        {
            return Err((self, child_pending));
        }
        let Self {
            effect,
            replay,
            pending: LiveWalPersistedPendingV1::ApplyPending,
        } = self
        else {
            unreachable!("preflight admitted only the pending Apply state")
        };
        match replay.complete_exact_apply(&effect, receipt) {
            Ok(replay) => Ok(Self {
                effect,
                replay,
                pending: LiveWalPersistedPendingV1::ApplyBound(child_pending),
            }),
            Err(replay) => Err((
                Self {
                    effect,
                    replay,
                    pending: LiveWalPersistedPendingV1::ApplyPending,
                },
                child_pending,
            )),
        }
    }

    /// Bind completed `Apply` evidence to its exact retained Validate predecessor.
    pub(super) fn exactly_binds_validated_apply_successor(
        &self,
        predecessor_effect: &AdapterEffect,
        predecessor_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        matches!(
            &self.pending,
            LiveWalPersistedPendingV1::ApplyBound(child_pending)
                if predecessor_pending
                    .project_validate_apply_successor(predecessor_effect, &self.effect)
                    .is_some_and(|expected| &expected == child_pending)
        ) && self
            .replay
            .exactly_matches_apply_effect(&self.effect, receipt)
    }

    fn exactly_matches_effect(&self) -> bool {
        match &self.pending {
            LiveWalPersistedPendingV1::PayloadFree(pending) => {
                pending.exactly_binds_adapter_effect(&self.effect)
                    && self
                        .replay
                        .exactly_matches_payload_free_effect(&self.effect)
            }
            LiveWalPersistedPendingV1::ValidateSignBound(pending) => {
                pending.exactly_binds_adapter_effect(&self.effect)
                    && self
                        .replay
                        .exactly_matches_payload_free_effect(&self.effect)
            }
            LiveWalPersistedPendingV1::ApplyPending => {
                self.replay.exactly_matches_persisted_effect(&self.effect)
            }
            LiveWalPersistedPendingV1::ApplyBound(_) => false,
        }
    }

    #[cfg(test)]
    /// Compare this sealed continuation with one exact test effect.
    pub(in crate::sumeragi) fn exactly_matches_effect_for_test(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.effect == *effect && self.exactly_matches_effect()
    }
}

impl LiveWalPersistedReplaySealV1 {
    /// Seal one exact effect released by an acknowledged live WAL append.
    ///
    /// This mint accepts the complete opaque runtime identity, never locator
    /// scalars. Its only production caller is the adapter's closed persisted-
    /// continuation conversion after matching the exact reducer WAL record.
    fn from_exact_persisted_effect(
        wal_identity: LiveWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        let LiveWalReplayProjectionV1 {
            context,
            stage,
            source,
        } = exact_live_wal_replay_projection(&wal_identity, effect)?;
        let state = if stage == LifecycleStageKind::ApplyDecision {
            canonical_wal_source(&source)
                .then_some(LiveWalPersistedReplayStateV1::ApplyPending { context, source })?
        } else {
            let authority = canonical_replay_authority(
                context,
                LifecycleReplaySourceV1::Wal(source),
                stage,
                ReplayPayloadBindingV1::None,
            )?;
            LiveWalPersistedReplayStateV1::Canonical { stage, authority }
        };
        let seal = Self {
            wal_identity,
            state,
        };
        seal.exactly_matches_persisted_effect(effect)
            .then_some(seal)
    }

    /// Recheck the exact locator, action, tag, and payload-free V1 envelope.
    fn exactly_matches_payload_free_effect(&self, effect: &AdapterEffect) -> bool {
        matches!(
            &self.state,
            LiveWalPersistedReplayStateV1::Canonical { stage, .. }
                if *stage != LifecycleStageKind::ApplyDecision
        ) && self.exactly_matches_persisted_effect(effect)
    }

    /// Complete only an exact pending `Apply` source with one durable body frame.
    ///
    /// The work registry calls this through its closed validated-completion
    /// join; no public constructor accepts a receipt or body-frame parts.
    #[allow(clippy::result_large_err)]
    fn complete_exact_apply(
        self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> Result<Self, Self> {
        let Self {
            wal_identity,
            state,
        } = self;
        let (context, source) = match state {
            LiveWalPersistedReplayStateV1::ApplyPending { context, source } => (context, source),
            other => {
                return Err(Self {
                    wal_identity,
                    state: other,
                });
            }
        };
        let Some(frame) = durable_body_frame_reference(context, receipt) else {
            return Err(Self {
                wal_identity,
                state: LiveWalPersistedReplayStateV1::ApplyPending { context, source },
            });
        };
        let payload =
            ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame));
        let Some(authority) = canonical_replay_authority(
            context,
            LifecycleReplaySourceV1::Wal(source.clone()),
            LifecycleStageKind::ApplyDecision,
            payload,
        ) else {
            return Err(Self {
                wal_identity,
                state: LiveWalPersistedReplayStateV1::ApplyPending { context, source },
            });
        };
        let completed = Self {
            wal_identity,
            state: LiveWalPersistedReplayStateV1::Canonical {
                stage: LifecycleStageKind::ApplyDecision,
                authority,
            },
        };
        if completed.exactly_matches_apply_effect(effect, receipt) {
            Ok(completed)
        } else {
            let Self { wal_identity, .. } = completed;
            Err(Self {
                wal_identity,
                state: LiveWalPersistedReplayStateV1::ApplyPending { context, source },
            })
        }
    }

    /// Recheck the exact live WAL source and receipt-bound `Apply` envelope.
    fn exactly_matches_apply_effect(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        if !matches!(
            &self.state,
            LiveWalPersistedReplayStateV1::Canonical {
                stage: LifecycleStageKind::ApplyDecision,
                ..
            }
        ) {
            return false;
        }
        let Some(LiveWalReplayProjectionV1 {
            context,
            stage: LifecycleStageKind::ApplyDecision,
            source,
        }) = exact_live_wal_replay_projection(&self.wal_identity, effect)
        else {
            return false;
        };
        let Some(frame) = durable_body_frame_reference(context, receipt) else {
            return false;
        };
        let payload =
            ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame));
        let Some(expected) = canonical_replay_authority(
            context,
            LifecycleReplaySourceV1::Wal(source),
            LifecycleStageKind::ApplyDecision,
            payload,
        ) else {
            return false;
        };
        matches!(
            &self.state,
            LiveWalPersistedReplayStateV1::Canonical {
                stage: LifecycleStageKind::ApplyDecision,
                authority,
            } if authority == &expected
        )
    }

    fn exactly_matches_persisted_effect(&self, effect: &AdapterEffect) -> bool {
        let Some(LiveWalReplayProjectionV1 {
            context,
            stage,
            source,
        }) = exact_live_wal_replay_projection(&self.wal_identity, effect)
        else {
            return false;
        };
        match &self.state {
            LiveWalPersistedReplayStateV1::Canonical {
                stage: retained_stage,
                authority,
            } => {
                *retained_stage == stage
                    && stage != LifecycleStageKind::ApplyDecision
                    && canonical_replay_authority(
                        context,
                        LifecycleReplaySourceV1::Wal(source),
                        stage,
                        ReplayPayloadBindingV1::None,
                    )
                    .is_some_and(|expected| &expected == authority)
            }
            LiveWalPersistedReplayStateV1::ApplyPending {
                context: retained_context,
                source: retained_source,
            } => {
                stage == LifecycleStageKind::ApplyDecision
                    && *retained_context == context
                    && retained_source == &source
                    && canonical_wal_source(retained_source)
            }
        }
    }
}

struct LiveWalReplayProjectionV1 {
    context: LifecycleContext,
    stage: LifecycleStageKind,
    source: WalReplaySourceV1,
}

/// Canonical inert replay evidence for one exact signed broadcast effect.
///
/// The complete message remains inside the private canonical envelope. The
/// pending fingerprint is runtime-only and non-decodable, so a decoded source
/// can never become authority for a different physical effect or causal root.
#[derive(PartialEq, Eq)]
#[must_use = "signed broadcast replay evidence must remain attached to its exact pending effect"]
pub(super) struct SignedBroadcastReplayEvidenceV1 {
    authority: LifecycleReplayAuthorityV1,
    pending: DirectSignedPendingBindingV1,
}

impl SignedBroadcastReplayEvidenceV1 {
    /// Seal one exact runtime-bound signed broadcast into canonical evidence.
    pub(super) fn from_exact_effect(
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        let pending_fingerprint = DirectSignedPendingBindingV1::from_exact_effect(effect, pending)?;
        let evidence = Self {
            authority: exact_signed_broadcast_authority(effect)?,
            pending: pending_fingerprint,
        };
        evidence
            .exactly_matches_effect(effect, pending)
            .then_some(evidence)
    }

    /// Compare the whole canonical envelope and runtime binding with one effect.
    pub(super) fn exactly_matches_effect(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.pending.exactly_matches(effect, pending)
            && exact_signed_broadcast_authority(effect)
                .is_some_and(|expected| expected == self.authority)
    }
}

/// Canonical inert replay evidence for one exact authenticated equivocation report.
///
/// The canonical wire pair remains private, while the runtime-only pending
/// fingerprint preserves observation order and signatures which canonical
/// equivocation normalization deliberately removes from the logical key.
#[derive(PartialEq, Eq)]
#[must_use = "signed equivocation replay evidence must remain attached to its exact pending effect"]
pub(super) struct SignedEquivocationReplayEvidenceV1 {
    authority: LifecycleReplayAuthorityV1,
    pending: DirectSignedPendingBindingV1,
}

impl SignedEquivocationReplayEvidenceV1 {
    /// Seal one exact runtime-bound authenticated conflict into canonical evidence.
    pub(super) fn from_exact_effect(
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        let pending_fingerprint = DirectSignedPendingBindingV1::from_exact_effect(effect, pending)?;
        let evidence = Self {
            authority: exact_signed_equivocation_authority(effect)?,
            pending: pending_fingerprint,
        };
        evidence
            .exactly_matches_effect(effect, pending)
            .then_some(evidence)
    }

    /// Compare the whole canonical envelope and runtime binding with one report.
    pub(super) fn exactly_matches_effect(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.pending.exactly_matches(effect, pending)
            && exact_signed_equivocation_authority(effect)
                .is_some_and(|expected| expected == self.authority)
    }
}

/// Seal the only raw adapter classes whose complete replay source and exact
/// pending ownership are both present at direct admission.
pub(super) fn exact_direct_signed_admission_authority(
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
) -> Option<LifecycleReplayAuthorityV1> {
    match effect {
        AdapterEffect::Broadcast(_) => {
            SignedBroadcastReplayEvidenceV1::from_exact_effect(effect, pending)
                .map(|evidence| evidence.authority)
        }
        AdapterEffect::ReportEquivocation { .. } => {
            SignedEquivocationReplayEvidenceV1::from_exact_effect(effect, pending)
                .map(|evidence| evidence.authority)
        }
        _ => None,
    }
}

fn candidate_from_authorized_projection(
    active_context: LifecycleContext,
    projected: super::projection::AuthorityFreeAdmissionProjection,
    payload: DurablePayloadReference,
    authority: LifecycleReplayAuthorityV1,
) -> Option<CandidateAdmission> {
    authority
        .validate_record(
            active_context,
            projected.key,
            projected.work_class,
            projected.stage,
            payload,
        )
        .ok()?;
    let candidate = CandidateAdmission::new(
        projected.key,
        projected.causal_root,
        projected.work_class,
        projected.stage,
        projected.initial_state,
        projected.reconstruction_source,
        payload,
        authority,
        projected.physical_geometry,
        None,
    );
    candidate
        .replay_authority_is_exact(active_context)
        .then_some(candidate)
}

/// Project one exact body-pipeline successor from canonical live-WAL evidence.
#[cfg(test)]
pub(super) fn exact_live_wal_body_successor_candidate_for_test(
    verified: &VerifiedHeightContext,
    predecessor_effect: &AdapterEffect,
    predecessor_pending: &PendingRuntimeEffectBinding,
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
    durable_receipt: Option<&DurableBodyReceipt>,
) -> Option<CandidateAdmission> {
    let successor_is_exact = match effect {
        AdapterEffect::Apply { .. } => {
            predecessor_pending
                .project_validate_apply_successor(predecessor_effect, effect)
                .as_ref()
                == Some(pending)
        }
        AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        } => match vote.phase {
            wire::GlobalPhase::Prepare => {
                predecessor_pending
                    .project_validate_sign_prepare_successor(predecessor_effect, effect)
                    .as_ref()
                    == Some(pending)
            }
            wire::GlobalPhase::Commit => {
                predecessor_pending
                    .project_validate_sign_commit_successor(predecessor_effect, effect)
                    .as_ref()
                    == Some(pending)
            }
        },
        _ => false,
    };
    if !successor_is_exact {
        return None;
    }

    let wal_identity = LiveWalFrameIdentity::for_test(17, 18, [0xB7; 32]);
    let LiveWalReplayProjectionV1 {
        context,
        stage,
        source,
    } = exact_live_wal_replay_projection(&wal_identity, effect)?;
    let payload = match stage {
        LifecycleStageKind::ApplyDecision => DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(context, durable_receipt?)?,
        ),
        LifecycleStageKind::SignPrepareVote | LifecycleStageKind::SignCommitVote
            if durable_receipt.is_none() =>
        {
            DurablePayloadReference::None
        }
        _ => return None,
    };
    let authority = canonical_replay_authority(
        context,
        LifecycleReplaySourceV1::Wal(source),
        stage,
        ReplayPayloadBindingV1::from_payload(payload),
    )?;
    let projected =
        super::projection::authority_free_admission_projection(context, verified, effect, pending)
            .ok()?;
    candidate_from_authorized_projection(context, projected, payload, authority)
}

/// Project one exact invalid-body report from its retained Validate evidence.
#[cfg(test)]
pub(super) fn exact_invalid_body_report_candidate_for_test(
    verified: &VerifiedHeightContext,
    validate_origin: &DurableValidateReplayEvidenceV1,
    validate_effect: &AdapterEffect,
    validate_pending: &PendingRuntimeEffectBinding,
    durable_receipt: &DurableBodyReceipt,
    report_effect: &AdapterEffect,
    report_pending: &PendingRuntimeEffectBinding,
) -> Option<CandidateAdmission> {
    if !validate_origin.exactly_matches_validate_pending(
        validate_effect,
        durable_receipt,
        validate_pending,
    ) || validate_pending
        .project_validate_report_invalid_certified_body_successor(validate_effect, report_effect)
        .as_ref()
        != Some(report_pending)
    {
        return None;
    }
    let context = replay_context(durable_receipt.round());
    let authority = exact_invalid_body_report_authority(
        validate_origin,
        validate_effect,
        durable_receipt,
        report_effect,
    )?;
    let projected = super::projection::authority_free_admission_projection(
        context,
        verified,
        report_effect,
        report_pending,
    )
    .ok()?;
    candidate_from_authorized_projection(
        context,
        projected,
        DurablePayloadReference::None,
        authority,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirectSignedPendingBindingV1 {
    causal_lifecycle_key: [u8; 32],
    effect_identity: [u8; 32],
}

impl DirectSignedPendingBindingV1 {
    fn from_exact_effect(
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        pending.exactly_binds_adapter_effect(effect).then(|| Self {
            causal_lifecycle_key: *pending.causal_lifecycle_key().as_ref(),
            effect_identity: *pending.exact_effect_identity().as_ref(),
        })
    }

    fn exactly_matches(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        pending.exactly_binds_adapter_effect(effect)
            && self.causal_lifecycle_key == *pending.causal_lifecycle_key().as_ref()
            && self.effect_identity == *pending.exact_effect_identity().as_ref()
    }
}

fn exact_signed_broadcast_authority(effect: &AdapterEffect) -> Option<LifecycleReplayAuthorityV1> {
    let AdapterEffect::Broadcast(message) = effect else {
        return None;
    };
    let (round, stage) = match &message.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => {
            (proposal.round, LifecycleStageKind::BroadcastProposal)
        }
        wire::ConsensusMessageV2Payload::Vote(vote) => (
            vote.round,
            match vote.phase {
                wire::GlobalPhase::Prepare => LifecycleStageKind::BroadcastPrepareVote,
                wire::GlobalPhase::Commit => LifecycleStageKind::BroadcastCommitVote,
            },
        ),
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => (
            certificate.round,
            match certificate.phase {
                wire::GlobalPhase::Prepare => LifecycleStageKind::BroadcastPrepareQc,
                wire::GlobalPhase::Commit => LifecycleStageKind::BroadcastCommitQc,
            },
        ),
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
            (vote.round, LifecycleStageKind::BroadcastTimeoutVote)
        }
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
            (certificate.round, LifecycleStageKind::BroadcastTc)
        }
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => return None,
    };
    canonical_replay_authority(
        replay_context(round),
        LifecycleReplaySourceV1::ConsensusBroadcast(message.clone()),
        stage,
        ReplayPayloadBindingV1::None,
    )
}

fn exact_signed_equivocation_authority(
    effect: &AdapterEffect,
) -> Option<LifecycleReplayAuthorityV1> {
    let AdapterEffect::ReportEquivocation { evidence } = effect else {
        return None;
    };
    let evidence = evidence.to_wire();
    let (round, stage) = match &evidence {
        wire::SumeragiV2Equivocation::Proposal { first, .. } => {
            (first.round, LifecycleStageKind::ReportProposalEquivocation)
        }
        wire::SumeragiV2Equivocation::PhaseVote { first, .. } => {
            (first.round, LifecycleStageKind::ReportVoteEquivocation)
        }
        wire::SumeragiV2Equivocation::TimeoutVote { first, .. } => {
            (first.round, LifecycleStageKind::ReportTimeoutEquivocation)
        }
    };
    canonical_replay_authority(
        replay_context(round),
        LifecycleReplaySourceV1::Equivocation(evidence),
        stage,
        ReplayPayloadBindingV1::None,
    )
}

fn replay_context(round: wire::ConsensusRound) -> LifecycleContext {
    LifecycleContext::new(digest_from_bytes(round.context_id.0.as_ref()), round.height)
}

/// Opaque replay evidence for one ordinary Fetch emitted by authenticated Proposal ingress.
///
/// This envelope is intentionally Clone because the signed Proposal and
/// receiver-authenticated carrier are cryptographically replayable. It is not
/// Decodable, exposes neither source nor ingress parts, and remains inert: its
/// only operations compare or project one fixed body-pipeline stage.
#[derive(Clone)]
#[must_use = "remote Proposal Fetch replay evidence must remain attached to its exact work"]
pub(in crate::sumeragi) struct RemoteProposalFetchReplayEvidenceV1 {
    authenticated: crate::sumeragi::v2::AuthenticatedConsensusMessage,
    ingress: RuntimeIngressOwnershipEvidence,
    source: BodyPipelineReplaySourceV1,
    fetch_pending: Arc<PendingRuntimeEffectBinding>,
}

impl core::fmt::Debug for RemoteProposalFetchReplayEvidenceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RemoteProposalFetchReplayEvidenceV1")
            .finish_non_exhaustive()
    }
}

/// Exact Proposal-origin seal waiting for the real durable Store receipt.
#[derive(Clone)]
#[must_use = "remote Proposal Store replay evidence must remain attached through durability"]
pub(in crate::sumeragi) struct RemoteProposalStoreReplayEvidenceV1 {
    authenticated: crate::sumeragi::v2::AuthenticatedConsensusMessage,
    ingress: RuntimeIngressOwnershipEvidence,
    source: BodyPipelineReplaySourceV1,
    store_pending: Arc<PendingRuntimeEffectBinding>,
}

impl core::fmt::Debug for RemoteProposalStoreReplayEvidenceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RemoteProposalStoreReplayEvidenceV1")
            .finish_non_exhaustive()
    }
}

/// Canonical Proposal+BodyFrame evidence waiting for its exact Validate successor.
#[derive(Clone)]
#[must_use = "stored remote Proposal replay evidence must project to exact Validate work"]
pub(in crate::sumeragi) struct RemoteProposalStoredReplayEvidenceV1 {
    family: RemoteProposalBodyPipelineReplayFamilyV1,
    store_pending: Arc<PendingRuntimeEffectBinding>,
}

impl core::fmt::Debug for RemoteProposalStoredReplayEvidenceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RemoteProposalStoredReplayEvidenceV1")
            .finish_non_exhaustive()
    }
}

/// Canonical Proposal+BodyFrame evidence retained beside exact Validate work.
#[derive(Clone)]
#[must_use = "remote Proposal Validate replay evidence must remain attached through completion"]
pub(in crate::sumeragi) struct RemoteProposalValidateReplayEvidenceV1 {
    family: RemoteProposalBodyPipelineReplayFamilyV1,
    validate_pending: Arc<PendingRuntimeEffectBinding>,
}

impl core::fmt::Debug for RemoteProposalValidateReplayEvidenceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RemoteProposalValidateReplayEvidenceV1")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct RemoteProposalBodyPipelineReplayFamilyV1 {
    authenticated: crate::sumeragi::v2::AuthenticatedConsensusMessage,
    ingress: RuntimeIngressOwnershipEvidence,
    source: BodyPipelineReplaySourceV1,
    body_frame: BodyFrameBindingV1,
}

impl RemoteProposalFetchReplayEvidenceV1 {
    /// Sole production mint from an exact authenticated Proposal dispatch.
    pub(in crate::sumeragi) fn from_exact_authenticated_proposal(
        _permit: RemoteProposalReplayMintPermit,
        authenticated: crate::sumeragi::v2::AuthenticatedConsensusMessage,
        ingress: RuntimeIngressOwnershipEvidence,
        effect: &AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        let proposal = exact_remote_proposal_fetch(&authenticated, &ingress, effect)?;
        if !pending.exactly_binds_adapter_effect(effect) {
            return None;
        }
        let AdapterEffect::FetchBody { tag, .. } = effect else {
            unreachable!("the exact remote Proposal projection is one FetchBody")
        };
        let source = BodyPipelineReplaySourceV1 {
            tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
            origin: BodyPipelineOriginV1::Proposal(proposal.clone()),
        };
        canonical_replay_authority(
            replay_context(proposal.round),
            LifecycleReplaySourceV1::BodyPipeline(source.clone()),
            LifecycleStageKind::FetchBody,
            ReplayPayloadBindingV1::None,
        )?;
        let evidence = Self {
            authenticated,
            ingress,
            source,
            fetch_pending: Arc::new(pending),
        };
        evidence.exactly_matches_fetch(effect).then_some(evidence)
    }

    /// Recheck the exact signed Proposal, ingress carrier, Fetch, and causal root.
    pub(in crate::sumeragi) fn exactly_matches_fetch(&self, effect: &AdapterEffect) -> bool {
        exact_remote_proposal_fetch(&self.authenticated, &self.ingress, effect).is_some_and(
            |proposal| {
                self.source
                    == (BodyPipelineReplaySourceV1 {
                        tag: match effect {
                            AdapterEffect::FetchBody { tag, .. } => ReplayEventTagV1::new(
                                tag.height(),
                                tag.view(),
                                tag.generation().get(),
                            ),
                            _ => return false,
                        },
                        origin: BodyPipelineOriginV1::Proposal(proposal.clone()),
                    })
                    && self.fetch_pending.exactly_binds_adapter_effect(effect)
                    && canonical_replay_authority(
                        replay_context(proposal.round),
                        LifecycleReplaySourceV1::BodyPipeline(self.source.clone()),
                        LifecycleStageKind::FetchBody,
                        ReplayPayloadBindingV1::None,
                    )
                    .is_some()
            },
        )
    }

    /// Compare the wrapper with the exact pending binding retained by its owner.
    pub(in crate::sumeragi) fn exactly_matches_fetch_pending(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.exactly_matches_fetch(effect) && self.fetch_pending.as_ref() == pending
    }

    /// Return whether a retry carries the same opaque Proposal and runtime binding.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        candidate: &Self,
        effect: &AdapterEffect,
    ) -> bool {
        self.exactly_matches_fetch(effect)
            && candidate.exactly_matches_fetch(effect)
            && self
                .authenticated
                .same_wire_envelope(&candidate.authenticated)
            && self.ingress == candidate.ingress
            && self.source == candidate.source
            && self.fetch_pending == candidate.fetch_pending
    }

    /// Preflight the only accepted Fetch-to-Store causal projection.
    pub(in crate::sumeragi) fn exactly_projects_store(
        &self,
        store_effect: &AdapterEffect,
        store_pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        let Some(fetch_effect) = remote_proposal_fetch_effect(&self.source) else {
            return false;
        };
        self.exactly_matches_fetch(&fetch_effect)
            && self
                .fetch_pending
                .project_proposal_fetch_store_successor(&fetch_effect, store_effect)
                .as_ref()
                == Some(store_pending)
    }

    /// Consume the exact Fetch origin into its one Store successor.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn project_exact_store(
        self,
        store_effect: &AdapterEffect,
        store_pending: &PendingRuntimeEffectBinding,
    ) -> Result<RemoteProposalStoreReplayEvidenceV1, Self> {
        if !self.exactly_projects_store(store_effect, store_pending) {
            return Err(self);
        }
        let fetch_effect = remote_proposal_fetch_effect(&self.source)
            .expect("an exact remote Proposal source has one Fetch effect");
        let projected = self
            .fetch_pending
            .project_proposal_fetch_store_successor(&fetch_effect, store_effect)
            .expect("an exact remote Proposal Fetch has one Store successor");
        debug_assert_eq!(&projected, store_pending);
        Ok(RemoteProposalStoreReplayEvidenceV1 {
            authenticated: self.authenticated,
            ingress: self.ingress,
            source: self.source,
            store_pending: Arc::new(projected),
        })
    }
}

impl RemoteProposalStoreReplayEvidenceV1 {
    /// Match the exact Store effect and inherited Proposal causal root.
    pub(in crate::sumeragi) fn exactly_matches_store(&self, effect: &AdapterEffect) -> bool {
        remote_proposal_store_matches(&self.authenticated, &self.ingress, &self.source, effect)
            && self.store_pending.exactly_binds_adapter_effect(effect)
    }

    /// Compare the Store wrapper with the exact pending binding retained by its owner.
    pub(in crate::sumeragi) fn exactly_matches_store_pending(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.exactly_matches_store(effect) && self.store_pending.as_ref() == pending
    }

    /// Join only the exact durable BodyFrame produced by this Store.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_durable_body(
        self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> Result<RemoteProposalStoredReplayEvidenceV1, Self> {
        if !self.exactly_matches_store(effect) {
            return Err(self);
        }
        let Some(family) = exact_remote_proposal_body_pipeline_family(
            &self.authenticated,
            &self.ingress,
            &self.source,
            receipt,
        ) else {
            return Err(self);
        };
        Ok(RemoteProposalStoredReplayEvidenceV1 {
            family,
            store_pending: self.store_pending,
        })
    }
}

impl RemoteProposalStoredReplayEvidenceV1 {
    /// Recheck the canonical Proposal+BodyFrame family at Store.
    pub(in crate::sumeragi) fn exactly_matches_store(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        self.store_pending.exactly_binds_adapter_effect(effect)
            && remote_proposal_body_stage_matches(
                &self.family,
                effect,
                receipt,
                LifecycleStageKind::StoreBody,
            )
    }

    /// Project an exact canonical Store candidate for focused transition tests.
    #[cfg(test)]
    pub(super) fn project_candidate_for_test(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_matches_store(effect, receipt) || self.store_pending.as_ref() != pending {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = replay_context(receipt.round());
        let payload = DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(active_context, receipt)
                .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?,
        );
        let payload_binding = ReplayPayloadBindingV1::from_payload(payload);
        if payload_binding != ReplayPayloadBindingV1::BodyFrame(self.family.body_frame) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let authority = canonical_replay_authority(
            active_context,
            LifecycleReplaySourceV1::BodyPipeline(self.family.source.clone()),
            LifecycleStageKind::StoreBody,
            payload_binding,
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?;
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            effect,
            pending,
        )?;
        candidate_from_authorized_projection(active_context, projected, payload, authority)
            .ok_or(AdapterEffectAdmissionError::InvalidCarrier)
    }

    /// Consume Store evidence through the exact Store-to-Validate pending lineage.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn project_exact_validate(
        self,
        store_effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> Result<RemoteProposalValidateReplayEvidenceV1, Self> {
        if !self.exactly_matches_store(store_effect, receipt)
            || !remote_proposal_body_stage_matches(
                &self.family,
                validate_effect,
                receipt,
                LifecycleStageKind::ValidateBody,
            )
            || self
                .store_pending
                .project_store_validate_successor(store_effect, validate_effect)
                .as_ref()
                != Some(validate_pending)
        {
            return Err(self);
        }
        let projected = self
            .store_pending
            .project_store_validate_successor(store_effect, validate_effect)
            .expect("an exact remote Proposal Store has one Validate successor");
        debug_assert_eq!(&projected, validate_pending);
        Ok(RemoteProposalValidateReplayEvidenceV1 {
            family: self.family,
            validate_pending: Arc::new(projected),
        })
    }
}

impl RemoteProposalValidateReplayEvidenceV1 {
    /// Recheck the canonical Proposal+BodyFrame family at Validate.
    pub(in crate::sumeragi) fn exactly_matches_validate(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        self.validate_pending.exactly_binds_adapter_effect(effect)
            && remote_proposal_body_stage_matches(
                &self.family,
                effect,
                receipt,
                LifecycleStageKind::ValidateBody,
            )
    }

    /// Compare the Validate wrapper with the exact pending binding retained by its owner.
    pub(in crate::sumeragi) fn exactly_matches_validate_pending(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.exactly_matches_validate(effect, receipt) && self.validate_pending.as_ref() == pending
    }
}

fn exact_remote_proposal_fetch<'a>(
    authenticated: &'a crate::sumeragi::v2::AuthenticatedConsensusMessage,
    ingress: &RuntimeIngressOwnershipEvidence,
    effect: &AdapterEffect,
) -> Option<&'a wire::Proposal> {
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = authenticated.payload() else {
        return None;
    };
    let AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: Some(manifest),
        certified_sources,
        certificate: None,
    } = effect
    else {
        return None;
    };
    (ingress.exactly_matches_authenticated(authenticated)
        && certified_sources.is_empty()
        && *round == proposal.round
        && *subject == proposal.subject
        && manifest == &proposal.manifest
        && tag.height() == round.height
        && tag.view() >= round.view)
        .then_some(proposal)
}

fn remote_proposal_fetch_effect(source: &BodyPipelineReplaySourceV1) -> Option<AdapterEffect> {
    let BodyPipelineOriginV1::Proposal(proposal) = &source.origin else {
        return None;
    };
    Some(AdapterEffect::FetchBody {
        tag: EventTag::new(
            source.tag.height,
            source.tag.view,
            crate::sumeragi::v2_core::Generation::new(source.tag.generation),
        ),
        round: proposal.round,
        subject: proposal.subject,
        manifest: Some(proposal.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    })
}

fn remote_proposal_store_matches(
    authenticated: &crate::sumeragi::v2::AuthenticatedConsensusMessage,
    ingress: &RuntimeIngressOwnershipEvidence,
    source: &BodyPipelineReplaySourceV1,
    effect: &AdapterEffect,
) -> bool {
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = authenticated.payload() else {
        return false;
    };
    let BodyPipelineOriginV1::Proposal(retained) = &source.origin else {
        return false;
    };
    let AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    } = effect
    else {
        return false;
    };
    ingress.exactly_matches_authenticated(authenticated)
        && retained == proposal
        && source.tag == ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get())
        && *round == proposal.round
        && *subject == proposal.subject
}

fn exact_remote_proposal_body_pipeline_family(
    authenticated: &crate::sumeragi::v2::AuthenticatedConsensusMessage,
    ingress: &RuntimeIngressOwnershipEvidence,
    source: &BodyPipelineReplaySourceV1,
    receipt: &DurableBodyReceipt,
) -> Option<RemoteProposalBodyPipelineReplayFamilyV1> {
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = authenticated.payload() else {
        return None;
    };
    let BodyPipelineOriginV1::Proposal(retained) = &source.origin else {
        return None;
    };
    if !ingress.exactly_matches_authenticated(authenticated)
        || retained != proposal
        || receipt.context_id() != proposal.round.context_id
        || receipt.round() != proposal.round
        || receipt.subject() != proposal.subject
        || receipt.manifest_hash() != HashOf::new(&proposal.manifest)
    {
        return None;
    }
    let frame = durable_body_frame_reference(replay_context(proposal.round), receipt)?;
    let ReplayPayloadBindingV1::BodyFrame(body_frame) =
        ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame))
    else {
        unreachable!("a durable body frame projects one body-frame binding")
    };
    let family = RemoteProposalBodyPipelineReplayFamilyV1 {
        authenticated: authenticated.clone(),
        ingress: ingress.clone(),
        source: source.clone(),
        body_frame,
    };
    (family.is_exact_for_stage(LifecycleStageKind::StoreBody)
        && family.is_exact_for_stage(LifecycleStageKind::ValidateBody))
    .then_some(family)
}

impl RemoteProposalBodyPipelineReplayFamilyV1 {
    fn is_exact_for_stage(&self, stage: LifecycleStageKind) -> bool {
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = self.authenticated.payload()
        else {
            return false;
        };
        matches!(
            stage,
            LifecycleStageKind::StoreBody | LifecycleStageKind::ValidateBody
        ) && self
            .ingress
            .exactly_matches_authenticated(&self.authenticated)
            && matches!(&self.source.origin, BodyPipelineOriginV1::Proposal(retained) if retained == proposal)
            && canonical_replay_authority(
                replay_context(proposal.round),
                LifecycleReplaySourceV1::BodyPipeline(self.source.clone()),
                stage,
                ReplayPayloadBindingV1::BodyFrame(self.body_frame),
            )
            .is_some()
    }
}

fn remote_proposal_body_stage_matches(
    family: &RemoteProposalBodyPipelineReplayFamilyV1,
    effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
    stage: LifecycleStageKind,
) -> bool {
    let exact_effect = match (stage, effect) {
        (
            LifecycleStageKind::StoreBody,
            AdapterEffect::StoreBody {
                tag,
                round,
                subject,
            },
        )
        | (
            LifecycleStageKind::ValidateBody,
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            },
        ) => {
            let BodyPipelineOriginV1::Proposal(proposal) = &family.source.origin else {
                return false;
            };
            family.source.tag
                == ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get())
                && *round == proposal.round
                && *subject == proposal.subject
        }
        _ => false,
    };
    exact_effect
        && exact_remote_proposal_body_pipeline_family(
            &family.authenticated,
            &family.ingress,
            &family.source,
            receipt,
        )
        .is_some_and(|expected| {
            expected.source == family.source
                && expected.body_frame == family.body_frame
                && expected.ingress == family.ingress
                && expected
                    .authenticated
                    .same_wire_envelope(&family.authenticated)
                && family.is_exact_for_stage(stage)
        })
}

/// Move-only pre-intent replay seal for one exact local `AssembleBody -> StoreBody` owner.
///
/// The private runtime mint permit prevents a cloned scheduling sidecar from
/// manufacturing this authority. The seal remains non-decodable and cannot be
/// completed from caller-supplied source or pending parts.
#[derive(Debug)]
#[must_use = "a local body replay seal must remain attached to its exact Store work"]
pub(in crate::sumeragi) struct LocalBodyPreIntentReplaySealV1 {
    source: BodyPipelineReplaySourceV1,
    store_pending: PendingRuntimeEffectBinding,
}

/// Canonical inert Validate evidence inherited only from the exact local Store owner.
#[derive(Debug)]
#[must_use = "local Validate replay evidence must remain attached through completion"]
pub(in crate::sumeragi) struct LocalValidateReplayEvidenceV1 {
    family: LocalBodyPipelineReplayFamilyV1,
    validate_pending: PendingRuntimeEffectBinding,
}

/// Inert replay evidence retained beside one exact queued `LocalProposalReady` owner.
///
/// This value is deliberately not part of the cloneable runtime command. Its
/// fixed equality oracle binds that command's durable and validated receipts
/// to the same causal root which owned local Store and Validate.
#[derive(Debug)]
#[must_use = "local proposal replay evidence must remain beside its exact runtime handoff"]
pub(in crate::sumeragi) struct LocalProposalReadyReplayEvidenceV1 {
    family: LocalBodyPipelineReplayFamilyV1,
    validate_pending: PendingRuntimeEffectBinding,
    validated_receipt: ValidatedBodyReceipt,
    command_identity: LocalProposalReadyCommandIdentity,
}

/// Inert composite joining one local body origin to its exact `ProposalIntent`.
///
/// The runtime effect and its pending binding stay private and non-decodable.
/// This value is the only post-command form of the local body capability, so
/// dispatching Sign work cannot discard the companion Store/Validate origin.
#[derive(Debug)]
#[must_use = "local ProposalIntent replay evidence must remain attached until atomic admission"]
pub(in crate::sumeragi) struct LocalProposalIntentReplayEvidenceV1 {
    ready: LocalProposalReadyReplayEvidenceV1,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
}

#[derive(Debug, PartialEq, Eq)]
struct LocalBodyPipelineReplayFamilyV1 {
    source: BodyPipelineReplaySourceV1,
    body_frame: BodyFrameBindingV1,
}

impl LocalBodyPreIntentReplaySealV1 {
    #[cfg(test)]
    fn for_test(
        effect: &AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        manifest: &wire::PayloadManifest,
    ) -> Option<Self> {
        let AdapterEffect::StoreBody { tag, .. } = effect else {
            return None;
        };
        let seal = Self {
            source: BodyPipelineReplaySourceV1 {
                tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
                origin: BodyPipelineOriginV1::LocalBody(manifest.clone()),
            },
            store_pending: pending,
        };
        seal.exactly_matches_store(effect, manifest).then_some(seal)
    }

    /// Consume the runtime's one-shot mint permit for the exact local Store owner.
    pub(in crate::sumeragi) fn from_exact_assemble_body(
        _permit: LocalBodyReplayMintPermit,
        effect: &AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        manifest: &wire::PayloadManifest,
    ) -> Option<Self> {
        let AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } = effect
        else {
            return None;
        };
        if *round != manifest.round
            || *subject != manifest.subject
            || tag.height() != round.height
            || tag.view() < round.view
            || !pending.exactly_binds_adapter_effect(effect)
        {
            return None;
        }
        let seal = Self {
            source: BodyPipelineReplaySourceV1 {
                tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
                origin: BodyPipelineOriginV1::LocalBody(manifest.clone()),
            },
            store_pending: pending,
        };
        seal.exactly_matches_store(effect, manifest).then_some(seal)
    }

    /// Recheck the exact Store effect, manifest, and non-decodable causal owner.
    pub(in crate::sumeragi) fn exactly_matches_store(
        &self,
        effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
    ) -> bool {
        local_body_source_matches(
            &self.source,
            effect,
            manifest,
            LifecycleStageKind::StoreBody,
        ) && self.store_pending.exactly_binds_adapter_effect(effect)
    }

    /// Preflight the exact Store-to-Validate lineage before either worker is retired.
    pub(in crate::sumeragi) fn exactly_projects_validate(
        &self,
        store_effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        let Some(family) = exact_local_body_pipeline_family(&self.source, receipt) else {
            return false;
        };
        self.exactly_matches_store(store_effect, manifest)
            && family.is_exact_for_stage(LifecycleStageKind::StoreBody)
            && local_body_stage_matches(
                &family,
                validate_effect,
                receipt,
                LifecycleStageKind::ValidateBody,
            )
            && self
                .store_pending
                .project_store_validate_successor(store_effect, validate_effect)
                .as_ref()
                == Some(validate_pending)
    }

    /// Atomically join durability and project the exact Validate successor.
    ///
    /// This is the production Store-completion cut. A failed preflight returns
    /// the original pre-intent seal, so no worker error can strand a partially
    /// advanced replay capability.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_and_project_validate(
        self,
        store_effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> Result<LocalValidateReplayEvidenceV1, Self> {
        if !self.exactly_projects_validate(
            store_effect,
            manifest,
            receipt,
            validate_effect,
            validate_pending,
        ) {
            return Err(self);
        }
        let family = exact_local_body_pipeline_family(&self.source, receipt)
            .expect("an exact local durability projection has one canonical family");
        let projected = self
            .store_pending
            .project_store_validate_successor(store_effect, validate_effect)
            .expect("an exact local Store owner has one Validate successor");
        debug_assert_eq!(&projected, validate_pending);
        Ok(LocalValidateReplayEvidenceV1 {
            family,
            validate_pending: projected,
        })
    }
}

impl LocalValidateReplayEvidenceV1 {
    /// Compare this canonical family with its exact inherited Validate owner.
    pub(in crate::sumeragi) fn exactly_matches_validate(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        self.validate_pending.exactly_binds_adapter_effect(effect)
            && local_body_stage_matches(
                &self.family,
                effect,
                receipt,
                LifecycleStageKind::ValidateBody,
            )
    }

    /// Compare one installed Validate task without exposing its pending owner.
    pub(in crate::sumeragi) fn exactly_matches_validate_task(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        self.exactly_matches_validate(effect, receipt)
            && ownership.pending_adapter_effect_binding(effect).as_ref()
                == Some(&self.validate_pending)
    }

    /// Consume successful validation into an exact local-proposal handoff.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn complete_local_proposal(
        self,
        effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
        validated_receipt: ValidatedBodyReceipt,
        command_identity: LocalProposalReadyCommandIdentity,
    ) -> Result<LocalProposalReadyReplayEvidenceV1, Self> {
        let AdapterEffect::ValidateBody { tag, .. } = effect else {
            return Err(self);
        };
        if validated_receipt.durable().manifest_hash() != HashOf::new(manifest)
            || !self.exactly_matches_validate(effect, validated_receipt.durable())
            || !local_body_family_manifest_matches(&self.family, manifest)
            || !command_identity.exactly_matches_handoff(
                *tag,
                manifest,
                validated_receipt.durable(),
                &validated_receipt,
                &self.validate_pending,
            )
        {
            return Err(self);
        }
        Ok(LocalProposalReadyReplayEvidenceV1 {
            family: self.family,
            validate_pending: self.validate_pending,
            validated_receipt,
            command_identity,
        })
    }
}

impl LocalProposalReadyReplayEvidenceV1 {
    /// Match an idempotent local-build retry against the retained command.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        tag: crate::sumeragi::v2_core::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> bool {
        self.command_identity == command_identity
            && local_body_family_manifest_matches(&self.family, manifest)
            && self.command_identity.exactly_matches_handoff(
                tag,
                manifest,
                self.validated_receipt.durable(),
                &self.validated_receipt,
                &self.validate_pending,
            )
    }

    /// Match terminal cleanup of the exact queued local-proposal command.
    pub(in crate::sumeragi) fn exactly_matches_retirement(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        tag: crate::sumeragi::v2_core::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        let BodyPipelineOriginV1::LocalBody(manifest) = &self.family.source.origin else {
            return false;
        };
        manifest.round == round
            && manifest.subject == subject
            && self.exactly_matches_retry(command_identity, tag, manifest)
    }

    /// Match the complete queued handoff without exposing its source or receipt parts.
    pub(in crate::sumeragi) fn exactly_matches_handoff(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        manifest: &wire::PayloadManifest,
        durable_receipt: &DurableBodyReceipt,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> bool {
        self.command_identity == command_identity
            && &self.validate_pending == validate_pending
            && self
                .validate_pending
                .exactly_binds_adapter_effect(validate_effect)
            && &self.validated_receipt == validated_receipt
            && validated_receipt.durable() == durable_receipt
            && local_body_family_manifest_matches(&self.family, manifest)
            && local_body_stage_matches(
                &self.family,
                validate_effect,
                durable_receipt,
                LifecycleStageKind::ValidateBody,
            )
    }

    /// Identify this command's exact unsigned ProposalIntent before owner comparison.
    pub(in crate::sumeragi) fn exactly_matches_proposal_intent_effect(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        effect: &AdapterEffect,
    ) -> bool {
        let BodyPipelineOriginV1::LocalBody(manifest) = &self.family.source.origin else {
            return false;
        };
        let AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } = effect
        else {
            return false;
        };
        self.exactly_matches_retry(command_identity, *tag, manifest)
            && proposal.signature.is_empty()
            && proposal.round == manifest.round
            && proposal.subject == manifest.subject
            && proposal.manifest == *manifest
    }

    /// Match only the exact ProposalIntent successor of this retained command.
    pub(in crate::sumeragi) fn exactly_matches_proposal_intent(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        let BodyPipelineOriginV1::LocalBody(manifest) = &self.family.source.origin else {
            return false;
        };
        self.command_identity == command_identity
            && self.command_identity.exactly_matches_proposal_intent(
                &self.validate_pending,
                manifest,
                effect,
                ownership,
            )
            && self
                .family
                .is_exact_for_stage(LifecycleStageKind::ValidateBody)
    }

    /// Consume the exact queued command into one inseparable ProposalIntent composite.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_proposal_intent(
        self,
        command_identity: LocalProposalReadyCommandIdentity,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<LocalProposalIntentReplayEvidenceV1, Self> {
        if !self.exactly_matches_proposal_intent(command_identity, effect, ownership) {
            return Err(self);
        }
        let Some(pending) = ownership.pending_adapter_effect_binding(effect) else {
            return Err(self);
        };
        Ok(LocalProposalIntentReplayEvidenceV1 {
            ready: self,
            effect: effect.clone(),
            pending,
        })
    }
}

impl LocalProposalIntentReplayEvidenceV1 {
    /// Match an idempotent local-build retry after ProposalIntent was emitted.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        tag: crate::sumeragi::v2_core::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> bool {
        self.ready
            .exactly_matches_retry(command_identity, tag, manifest)
            && self.pending.exactly_binds_adapter_effect(&self.effect)
    }

    /// Match terminal cleanup of the exact local ProposalIntent composite.
    pub(in crate::sumeragi) fn exactly_matches_retirement(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        tag: crate::sumeragi::v2_core::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        self.ready
            .exactly_matches_retirement(command_identity, tag, round, subject)
            && self.pending.exactly_binds_adapter_effect(&self.effect)
    }

    /// Recheck the complete ProposalIntent and causal owner without exposing parts.
    pub(in crate::sumeragi) fn exactly_matches_proposal_intent(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        self.ready
            .exactly_matches_proposal_intent(command_identity, effect, ownership)
            && &self.effect == effect
            && ownership.pending_adapter_effect_binding(effect).as_ref() == Some(&self.pending)
    }

    /// Identify a duplicate or foreign-owner emission of this exact ProposalIntent.
    pub(in crate::sumeragi) fn exactly_matches_proposal_intent_effect(
        &self,
        command_identity: LocalProposalReadyCommandIdentity,
        effect: &AdapterEffect,
    ) -> bool {
        &self.effect == effect
            && self
                .ready
                .exactly_matches_proposal_intent_effect(command_identity, effect)
    }
}

fn exact_local_body_pipeline_family(
    source: &BodyPipelineReplaySourceV1,
    receipt: &DurableBodyReceipt,
) -> Option<LocalBodyPipelineReplayFamilyV1> {
    let BodyPipelineOriginV1::LocalBody(manifest) = &source.origin else {
        return None;
    };
    if receipt.round() != manifest.round
        || receipt.subject() != manifest.subject
        || receipt.context_id() != manifest.round.context_id
        || receipt.manifest_hash() != HashOf::new(manifest)
    {
        return None;
    }
    let context = replay_context(manifest.round);
    let frame = durable_body_frame_reference(context, receipt)?;
    let ReplayPayloadBindingV1::BodyFrame(body_frame) =
        ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame))
    else {
        unreachable!("a durable body frame projects one body-frame binding")
    };
    let family = LocalBodyPipelineReplayFamilyV1 {
        source: source.clone(),
        body_frame,
    };
    (family.is_exact_for_stage(LifecycleStageKind::StoreBody)
        && family.is_exact_for_stage(LifecycleStageKind::ValidateBody))
    .then_some(family)
}

impl LocalBodyPipelineReplayFamilyV1 {
    fn is_exact_for_stage(&self, stage: LifecycleStageKind) -> bool {
        let BodyPipelineOriginV1::LocalBody(manifest) = &self.source.origin else {
            return false;
        };
        if !matches!(
            stage,
            LifecycleStageKind::StoreBody | LifecycleStageKind::ValidateBody
        ) {
            return false;
        }
        canonical_replay_authority(
            replay_context(manifest.round),
            LifecycleReplaySourceV1::BodyPipeline(self.source.clone()),
            stage,
            ReplayPayloadBindingV1::BodyFrame(self.body_frame),
        )
        .is_some()
    }
}

fn local_body_family_manifest_matches(
    family: &LocalBodyPipelineReplayFamilyV1,
    manifest: &wire::PayloadManifest,
) -> bool {
    matches!(
        &family.source.origin,
        BodyPipelineOriginV1::LocalBody(retained) if retained == manifest
    )
}

fn local_body_source_matches(
    source: &BodyPipelineReplaySourceV1,
    effect: &AdapterEffect,
    manifest: &wire::PayloadManifest,
    stage: LifecycleStageKind,
) -> bool {
    let (tag, round, subject) = match (stage, effect) {
        (
            LifecycleStageKind::StoreBody,
            AdapterEffect::StoreBody {
                tag,
                round,
                subject,
            },
        )
        | (
            LifecycleStageKind::ValidateBody,
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            },
        ) => (tag, round, subject),
        _ => return false,
    };
    source.tag == ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get())
        && matches!(
            &source.origin,
            BodyPipelineOriginV1::LocalBody(retained) if retained == manifest
        )
        && *round == manifest.round
        && *subject == manifest.subject
}

fn local_body_stage_matches(
    family: &LocalBodyPipelineReplayFamilyV1,
    effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
    stage: LifecycleStageKind,
) -> bool {
    let BodyPipelineOriginV1::LocalBody(manifest) = &family.source.origin else {
        return false;
    };
    local_body_source_matches(&family.source, effect, manifest, stage)
        && exact_local_body_pipeline_family(&family.source, receipt)
            .is_some_and(|expected| expected == *family && family.is_exact_for_stage(stage))
}

/// Selector-authenticated origin awaiting one exact durable body-frame binding.
///
/// This move-only value is not codec data. Its sole production mint accepts
/// the sealed selector authority and its exact pending Fetch effect, so a
/// caller cannot assemble it from a certificate, manifest, or response parts.
#[derive(Debug)]
#[must_use = "an authenticated certified Fetch origin still requires its durable body receipt"]
pub(super) struct AuthenticatedCertifiedFetchReplayOriginV1 {
    coordinates: CertifiedBodyPipelineCoordinatesV1,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
}

/// Canonical inert replay evidence for the certified Fetch stage.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "certified Fetch replay evidence must remain attached to its closed carrier"]
pub(super) struct CertifiedFetchReplayEvidenceV1 {
    family: CertifiedBodyPipelineReplayFamilyV1,
}

/// Opaque restart-stable projection of one body-fsynced Certified Fetch.
///
/// The canonical digest binds the complete Fetch effect identity, its causal
/// key, and the canonical frame-bound replay envelope. Transport occurrence
/// hashes and fair-ingress ordinals never enter this value.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "a durable Certified Fetch projection must remain with its exact completion"]
pub(super) struct DurableCertifiedFetchReplayProjectionV1 {
    payload: DurablePayloadReference,
    authority: LifecycleReplayAuthorityV1,
    causal_key: Hash,
    effect_identity: Hash,
    completion_digest: LifecycleDigest,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
}

/// Opaque result of the consuming LedgerV1/body-store Certified-Fetch join.
#[must_use = "recovered durable Fetch authority must enter coordinator and registry recovery"]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct AuthenticatedRecoveredDurableCertifiedFetchV1
{
    completion: CertifiedFetchCompletion,
    candidate: CandidateAdmission,
}

/// Aggregate opaque recovery cut for every live BodyFrame-backed Fetch row.
///
/// No row, candidate, effect, pending binding, or registry material can be
/// extracted independently. Startup must eventually consume the whole cut at
/// one coordinator-open/registry-install boundary.
#[must_use = "the complete durable Fetch census must be consumed atomically"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct AuthenticatedRecoveredDurableCertifiedFetchCensusV1
{
    ledger_frame_identity: LifecycleDigest,
    entries: Vec<AuthenticatedRecoveredDurableCertifiedFetchV1>,
}

/// One consuming startup phase for the complete recovered Ready-Fetch census.
///
/// Candidate projection is moved into the logical recovery cut exactly once.
/// The remaining closed completions can then enter only an initially empty
/// concrete registry. Neither side has a row or parts accessor.
#[must_use = "prepared durable Fetch startup authority must be installed atomically"]
pub(super) struct PreparedDurableCertifiedFetchStartupV1 {
    ledger_frame_identity: LifecycleDigest,
    entries: Vec<PreparedDurableCertifiedFetchStartupEntryV1>,
}

struct PreparedDurableCertifiedFetchStartupEntryV1 {
    candidate: Option<CandidateAdmission>,
    completion: CertifiedFetchCompletion,
}

impl AuthenticatedRecoveredDurableCertifiedFetchV1 {
    fn is_exact(&self) -> bool {
        self.completion.matches_recovered_candidate(&self.candidate)
    }
}

impl AuthenticatedRecoveredDurableCertifiedFetchCensusV1 {
    fn from_exact_ledger_census(
        _permit: DurableCertifiedFetchLedgerCensusPermit,
        entries: Vec<AuthenticatedRecoveredDurableCertifiedFetchV1>,
    ) -> Option<Self> {
        let mut addresses = std::collections::BTreeSet::new();
        let mut owners = std::collections::BTreeSet::new();
        let mut digests = std::collections::BTreeSet::new();
        let mut body_frames = std::collections::BTreeSet::new();
        for entry in &entries {
            let DurablePayloadReference::BodyFrame(body_frame) = entry.candidate.payload else {
                return None;
            };
            if !entry.is_exact()
                || !addresses.insert(entry.completion.address())
                || !owners.insert(entry.completion.owner())
                || !entry
                    .completion
                    .ready_digest()
                    .is_some_and(|digest| digests.insert(digest))
                || !body_frames.insert(body_frame)
            {
                return None;
            }
        }
        Some(Self {
            ledger_frame_identity: _permit.into_frame_identity(),
            entries,
        })
    }

    fn is_exact(&self) -> bool {
        self.entries
            .iter()
            .all(AuthenticatedRecoveredDurableCertifiedFetchV1::is_exact)
    }

    /// Compare against the exact opened frame and its complete live-Fetch count.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exactly_matches_opened_ledger(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
        live_body_fetch_count: usize,
    ) -> bool {
        self.ledger_frame_identity == ledger.frame_identity()
            && self.entries.len() == live_body_fetch_count
    }

    /// Consume the authenticated census into its single startup phase.
    pub(super) fn into_startup(
        self,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> Option<PreparedDurableCertifiedFetchStartupV1> {
        let live_body_fetch_count = ledger
            .records()
            .iter()
            .filter(|record| {
                record.work_class() == Some(LifecycleWorkClass::Fetch)
                    && record.terminal() == Some(None)
                    && matches!(
                        record.durable_payload(),
                        Some(DurablePayloadReference::BodyFrame(_))
                    )
            })
            .count();
        if !self.exactly_matches_opened_ledger(ledger, live_body_fetch_count) || !self.is_exact() {
            return None;
        }
        Some(PreparedDurableCertifiedFetchStartupV1 {
            ledger_frame_identity: self.ledger_frame_identity,
            entries: self
                .entries
                .into_iter()
                .map(|entry| PreparedDurableCertifiedFetchStartupEntryV1 {
                    candidate: Some(entry.candidate),
                    completion: entry.completion,
                })
                .collect(),
        })
    }

    #[cfg(test)]
    pub(super) fn corrupt_first_completion_for_test(&mut self) {
        if let Some(entry) = self.entries.first_mut() {
            entry.completion.corrupt_for_startup_test();
        }
    }
}

impl PreparedDurableCertifiedFetchStartupV1 {
    /// Verify the complete phase against one still-empty concrete registry.
    pub(super) fn preflights_empty_registry(
        &self,
        registry: &ConcreteLifecycleWorkRegistry,
    ) -> bool {
        let mut addresses = std::collections::BTreeSet::new();
        let mut digests = std::collections::BTreeSet::new();
        registry.is_empty()
            && self.entries.iter().all(|entry| {
                entry.candidate.as_ref().is_some_and(|candidate| {
                    entry.completion.matches_recovered_candidate(candidate)
                        && addresses.insert(entry.completion.address())
                        && entry
                            .completion
                            .ready_digest()
                            .is_some_and(|digest| digests.insert(digest))
                })
            })
    }

    /// Move every exact logical candidate into one recovery map.
    ///
    /// Complete validation precedes mutation, and a second call is rejected.
    pub(super) fn splice_candidates(
        &mut self,
        ledger: &super::ledger::LifecycleLedgerV1,
        candidates: &mut std::collections::BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        if self.ledger_frame_identity != ledger.frame_identity()
            || self.entries.iter().any(|entry| {
                entry.candidate.as_ref().is_none_or(|candidate| {
                    !entry.completion.matches_recovered_candidate(candidate)
                        || candidates.contains_key(&candidate.key)
                })
            })
        {
            return false;
        }
        for entry in &mut self.entries {
            let candidate = entry
                .candidate
                .take()
                .expect("whole-census preflight retained every Fetch candidate");
            assert!(candidates.insert(candidate.key, candidate).is_none());
        }
        true
    }

    /// Install all retained concrete completions into one empty registry.
    ///
    /// The complete no-collision and carrier-integrity preflight precedes the
    /// first insertion. The mutation tail is therefore assertion-only.
    pub(super) fn install_into_empty_registry(
        self,
        registry: &mut ConcreteLifecycleWorkRegistry,
    ) -> Result<(), Self> {
        let mut addresses = std::collections::BTreeSet::new();
        if !registry.is_empty()
            || self.entries.iter().any(|entry| {
                entry.candidate.is_some()
                    || !addresses.insert(entry.completion.address())
                    || entry.completion.ready_digest().is_none()
                    || !entry.completion.validates(
                        entry
                            .completion
                            .ready_digest()
                            .expect("guard retained a Ready digest"),
                    )
            })
        {
            return Err(self);
        }
        for entry in self.entries {
            assert!(
                registry
                    .install_recovered_durable_fetch(entry.completion)
                    .is_ok(),
                "preflighted empty-registry Fetch installation is infallible"
            );
        }
        Ok(())
    }

    /// Install the final-frame Fetch census beside one exact recovered-WAL
    /// authority carrier.
    ///
    /// Logical candidates must already have been spliced into the same
    /// authenticated recovery cut. The complete collision and owner preflight
    /// precedes mutation, so the insertion tail is infallible and cannot leave
    /// a partial recovered registry.
    pub(super) fn install_alongside_recovered_wal_authority(
        self,
        registry: &mut ConcreteLifecycleWorkRegistry,
    ) -> Result<(), Self> {
        let completions = self
            .entries
            .iter()
            .map(|entry| &entry.completion)
            .collect::<Vec<_>>();
        if self.entries.iter().any(|entry| entry.candidate.is_some())
            || !registry.preflights_recovered_fetches_alongside_wal_authority(&completions)
        {
            return Err(self);
        }
        for entry in self.entries {
            assert!(
                registry
                    .install_recovered_durable_fetch(entry.completion)
                    .is_ok(),
                "preflighted recovered Sign-plus-Fetch installation is infallible"
            );
        }
        Ok(())
    }
}

/// Seal the complete opened-ledger Fetch census without releasing row parts.
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn seal_recovered_durable_certified_fetch_census(
    permit: DurableCertifiedFetchLedgerCensusPermit,
    entries: Vec<AuthenticatedRecoveredDurableCertifiedFetchV1>,
) -> Option<AuthenticatedRecoveredDurableCertifiedFetchCensusV1> {
    let census = AuthenticatedRecoveredDurableCertifiedFetchCensusV1::from_exact_ledger_census(
        permit, entries,
    )?;
    census.is_exact().then_some(census)
}

/// One-shot proof that a pending Fetch binding is reconstructed only while an
/// exact frame-bound Certified replay family is still sealed.
pub(in crate::sumeragi) struct DurableCertifiedFetchPendingMintPermit {
    _linearity: DurableCertifiedFetchPendingMintLinearity,
}

struct DurableCertifiedFetchPendingMintLinearity;

impl Drop for DurableCertifiedFetchPendingMintLinearity {
    fn drop(&mut self) {}
}

impl DurableCertifiedFetchPendingMintPermit {
    fn new() -> Self {
        Self {
            _linearity: DurableCertifiedFetchPendingMintLinearity,
        }
    }
}

/// Canonical inert replay evidence projected for the certified Store stage.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "certified Store replay evidence must remain attached to its closed carrier"]
pub(super) struct CertifiedStoreReplayEvidenceV1 {
    family: CertifiedBodyPipelineReplayFamilyV1,
}

/// Canonical inert replay evidence projected for the certified Validate stage.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "certified Validate replay evidence must remain attached to its closed carrier"]
pub(in crate::sumeragi) struct CertifiedValidateReplayEvidenceV1 {
    family: CertifiedBodyPipelineReplayFamilyV1,
    validate_pending: DirectSignedPendingBindingV1,
}

/// Closed replay origin retained by an exact durable Validate carrier.
///
/// Both variants are authenticated before this enum is constructed. The enum
/// has no fallback and exposes neither source family nor manifest parts.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Clone, Debug)]
#[must_use = "durable Validate replay evidence must remain attached to its exact carrier"]
pub(in crate::sumeragi) enum DurableValidateReplayEvidenceV1 {
    /// Body fetched under the exact retained certificate.
    Certified(CertifiedValidateReplayEvidenceV1),
    /// Ordinary body fetched from one exact signed remote Proposal.
    RemoteProposal(RemoteProposalValidateReplayEvidenceV1),
}

/// Canonical non-decodable replay evidence for one invalid certified body report.
///
/// The body origin and complete canonical V1 envelope remain private. The
/// runtime-only pending fingerprint binds the exact adapter-proved report to
/// the causal root of the rejected Validate owner.
#[derive(Debug)]
#[must_use = "invalid-body replay evidence must remain attached to its report work"]
pub(in crate::sumeragi) struct InvalidBodyReportReplayEvidenceV1 {
    authority: LifecycleReplayAuthorityV1,
    validate_origin: DurableValidateReplayEvidenceV1,
    report_pending: DirectSignedPendingBindingV1,
}

/// One inert replay family shared by a Certified-Serve request and its
/// atomically reserved ProducerTurn.
///
/// The family is runtime-only authority. Its canonical storage source remains
/// private and cannot be decoded or reconstructed from source parts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CertifiedServeStorageReplayFamilyV1 {
    source: CertifiedServeStorageSourceV1,
}

/// Opaque replay evidence for one exact post-fsync Certified-Serve record.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Certified-Serve replay evidence must remain with its reserved producer turn"]
struct CertifiedServeReplayEvidenceV1 {
    family: Arc<CertifiedServeStorageReplayFamilyV1>,
    payload: ReplayPayloadBindingV1,
}

/// Opaque replay evidence for the dormant ProducerTurn reserved beside one
/// exact Certified-Serve request.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "ProducerTurn replay evidence must remain with its Certified-Serve origin"]
struct CertifiedServeProducerTurnReplayEvidenceV1 {
    family: Arc<CertifiedServeStorageReplayFamilyV1>,
}

/// Closed pair preserving one common post-fsync storage origin across the
/// Certified-Serve record and its reserved ProducerTurn.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "the Certified-Serve replay pair has not entered durable admission"]
pub(super) struct CertifiedServeReplayEvidencePairV1 {
    serve: CertifiedServeReplayEvidenceV1,
    producer: CertifiedServeProducerTurnReplayEvidenceV1,
}

/// One move-only terminal replay-family replacement for an adjacent
/// Certified-Serve/ProducerTurn pair.
///
/// Production construction is closed over either a post-fsync terminal
/// receipt or an authenticated payload-store recovery record. In particular,
/// no caller can inject the terminal payload-frame hash as raw bytes.
#[derive(Debug)]
#[must_use = "the terminal Certified-Serve replay pair must be installed atomically"]
pub(super) struct CertifiedServeTerminalReplayAuthorityPairV1 {
    terminal_payload: DurablePayloadReference,
    terminal_outcome: TerminalOutcome,
    serve: LifecycleReplayAuthorityV1,
    producer: LifecycleReplayAuthorityV1,
}

impl CertifiedServeTerminalReplayAuthorityPairV1 {
    /// Return the terminal tombstone bound by this sealed pair.
    pub(super) const fn terminal_outcome(&self) -> TerminalOutcome {
        self.terminal_outcome
    }

    /// Clone this still-sealed terminal family into one whole concrete-carrier
    /// proof. No serve/producer authority or raw frame hash leaves the replay
    /// module; the registry may install the returned pair only through its
    /// typed terminal transition.
    pub(super) fn terminal_carrier_replay_evidence(
        &self,
    ) -> Option<CertifiedServeReplayEvidencePairV1> {
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &self.serve.source else {
            return None;
        };
        let family = Arc::new(CertifiedServeStorageReplayFamilyV1 {
            source: source.clone(),
        });
        let evidence = CertifiedServeReplayEvidencePairV1 {
            serve: CertifiedServeReplayEvidenceV1 {
                family: Arc::clone(&family),
                payload: ReplayPayloadBindingV1::from_payload(self.terminal_payload),
            },
            producer: CertifiedServeProducerTurnReplayEvidenceV1 { family },
        };
        (evidence.shares_exact_storage_origin()
            && evidence.serve.exactly_matches_authority(&self.serve)
            && evidence.producer.exactly_matches_authority(&self.producer))
        .then_some(evidence)
    }

    /// Rebind one live Pending pair from an exact post-fsync completion
    /// receipt.
    pub(super) fn from_completed_receipt(
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
        receipt: DurableCertifiedServeCompletedReceipt,
    ) -> Option<Self> {
        let DurablePayloadReference::CertifiedServePending {
            request,
            certificate,
        } = serve_metadata.payload
        else {
            return None;
        };
        if request != digest_from_bytes(receipt.id().request_hash().as_ref())
            || certificate != digest_from_bytes(receipt.certificate_hash().as_ref())
        {
            return None;
        }
        let response = digest_from_bytes(receipt.response_hash().as_ref());
        Self::from_terminal_frame(
            active_context,
            serve_record,
            serve_metadata,
            producer_record,
            producer_metadata,
            DurablePayloadReference::CertifiedServeCompleted {
                request,
                certificate,
                response,
            },
            TerminalOutcome::Completed(Some(response)),
            receipt.payload_hash(),
        )
    }

    /// Rebind one live Pending pair from an exact post-fsync negative receipt.
    pub(super) fn from_negative_receipt(
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
        receipt: DurableCertifiedServeNegativeReceipt,
    ) -> Option<Self> {
        let DurablePayloadReference::CertifiedServePending {
            request,
            certificate,
        } = serve_metadata.payload
        else {
            return None;
        };
        if request != digest_from_bytes(receipt.id().request_hash().as_ref())
            || certificate != digest_from_bytes(receipt.certificate_hash().as_ref())
        {
            return None;
        }
        let outcome = match receipt.outcome() {
            CertifiedServePayloadNegativeOutcome::Cancelled => {
                DurableServeNegativeOutcome::Cancelled
            }
            CertifiedServePayloadNegativeOutcome::Rejected(code) => {
                DurableServeNegativeOutcome::Rejected(code)
            }
            CertifiedServePayloadNegativeOutcome::Failed(code) => {
                DurableServeNegativeOutcome::Failed(code)
            }
        };
        Self::from_terminal_frame(
            active_context,
            serve_record,
            serve_metadata,
            producer_record,
            producer_metadata,
            DurablePayloadReference::CertifiedServeNegative {
                request,
                certificate,
                outcome,
            },
            outcome.terminal(),
            receipt.payload_hash(),
        )
    }

    /// Seal the terminal replay pair recovered from one authenticated payload
    /// frame and its independently reconstructed admission candidate.
    pub(super) fn from_authenticated_recovery(
        active_context: LifecycleContext,
        recovered: &AuthenticatedRecoveredCertifiedServePayload,
        candidate: &CandidateAdmission,
        terminal_payload: DurablePayloadReference,
        terminal_outcome: TerminalOutcome,
    ) -> Option<Self> {
        if !recovered.exactly_matches_persisted_payload()
            || !terminal_payload
                .matches_terminal(LifecycleWorkClass::CertifiedServe, Some(terminal_outcome))
            || candidate.work_class != LifecycleWorkClass::CertifiedServe
            || !candidate.payload.same_admission_material(terminal_payload)
        {
            return None;
        }
        let recovered_payload = recovered_certified_serve_payload(recovered)?;
        if recovered_payload != ReplayPayloadBindingV1::from_payload(terminal_payload) {
            return None;
        }
        let recovered_family = exact_certified_serve_storage_replay_family(
            active_context,
            recovered.request(),
            recovered.payload_hash(),
            recovered.local_retainer(),
        )?;
        let recovered_source =
            LifecycleReplaySourceV1::CertifiedServeStorage(recovered_family.source.clone());
        if candidate.replay_authority.source != recovered_source {
            return None;
        }
        let producer = candidate.producer_turn.as_ref()?;
        let serve = LifecycleReplayAuthorityV1 {
            format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
            payload: recovered_payload,
            source: recovered_source,
        };
        let sealed = Self {
            terminal_payload,
            terminal_outcome,
            serve,
            producer: producer.replay_authority.clone(),
        };
        sealed
            .pending_candidate_matches_terminal_family(active_context, candidate)
            .then_some(sealed)
    }

    #[allow(clippy::too_many_arguments)]
    fn from_terminal_frame(
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
        terminal_payload: DurablePayloadReference,
        terminal_outcome: TerminalOutcome,
        terminal_frame_hash: Hash,
    ) -> Option<Self> {
        if !terminal_payload
            .matches_terminal(LifecycleWorkClass::CertifiedServe, Some(terminal_outcome))
            || !serve_metadata
                .payload
                .same_admission_material(terminal_payload)
        {
            return None;
        }
        let LifecycleReplaySourceV1::CertifiedServeStorage(mut source) =
            serve_metadata.replay_authority.source.clone()
        else {
            return None;
        };
        source.payload_hash = *terminal_frame_hash.as_ref();
        let terminal_source = LifecycleReplaySourceV1::CertifiedServeStorage(source);
        let sealed = Self {
            terminal_payload,
            terminal_outcome,
            serve: LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::from_payload(terminal_payload),
                source: terminal_source.clone(),
            },
            producer: LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::None,
                source: terminal_source,
            },
        };
        sealed
            .exactly_advances_pending_records(
                active_context,
                serve_record,
                serve_metadata,
                producer_record,
                producer_metadata,
            )
            .then_some(sealed)
    }

    /// Construct a synthetic terminal-frame transition for pure reducer tests.
    /// Production paths have no raw-hash constructor and must use a durable
    /// receipt or authenticated recovery record.
    #[cfg(test)]
    pub(super) fn from_test_terminal_outcome(
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
        terminal_outcome: TerminalOutcome,
    ) -> Option<Self> {
        let terminal_payload = serve_metadata.payload.terminalized(terminal_outcome)?;
        let mut preimage = Vec::with_capacity(Hash::LENGTH + 2);
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) =
            &serve_metadata.replay_authority.source
        else {
            return None;
        };
        preimage.extend_from_slice(&source.payload_hash);
        preimage.push(0xFF);
        preimage.push(match terminal_outcome {
            TerminalOutcome::Advanced => 0,
            TerminalOutcome::Completed(_) => 1,
            TerminalOutcome::Cancelled => 2,
            TerminalOutcome::Rejected(_) => 3,
            TerminalOutcome::Failed(_) => 4,
        });
        Self::from_terminal_frame(
            active_context,
            serve_record,
            serve_metadata,
            producer_record,
            producer_metadata,
            terminal_payload,
            terminal_outcome,
            Hash::new(preimage),
        )
    }

    /// Prove the sole permitted payload-store-ahead transition: one exact
    /// Pending ledger pair advances to this authenticated terminal frame while
    /// request, certificate, local retainer, keys, and stages remain fixed.
    pub(super) fn exactly_advances_pending_records(
        &self,
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
    ) -> bool {
        self.exactly_advances_pending_coordinates(
            active_context,
            serve_record.key,
            serve_record.owner,
            serve_record.ordinal,
            serve_record.stage,
            serve_metadata.reconstruction_source,
            serve_metadata.payload,
            &serve_metadata.replay_authority,
            producer_record.key,
            producer_record.owner,
            producer_record.ordinal,
            producer_record.stage,
            producer_metadata.reconstruction_source,
            producer_metadata.payload,
            &producer_metadata.replay_authority,
        )
    }

    /// Match a Pending logical pair after both authorities have already been
    /// rebound to this exact terminal frame family but before the terminal
    /// payload/tombstone is installed.
    pub(super) fn exactly_matches_rebound_records(
        &self,
        active_context: LifecycleContext,
        serve_record: &LifecycleRecord,
        serve_metadata: &DurableRecordMetadata,
        producer_record: &LifecycleRecord,
        producer_metadata: &DurableRecordMetadata,
    ) -> bool {
        let DurablePayloadReference::CertifiedServePending { .. } = serve_metadata.payload else {
            return false;
        };
        serve_record.ordinal.checked_add(1) == Some(producer_record.ordinal)
            && serve_record.work_class == LifecycleWorkClass::CertifiedServe
            && serve_record.stage.kind() == LifecycleStageKind::CertifiedServe
            && producer_record.work_class == LifecycleWorkClass::ProducerTurn
            && producer_record.stage.kind() == LifecycleStageKind::ProducerTurn
            && serve_record.owner == producer_record.owner
            && serve_and_producer_keys_match(serve_record.key, producer_record.key)
            && serve_metadata.reconstruction_source == producer_metadata.reconstruction_source
            && serve_metadata.reconstruction_source == serve_record.owner.causal_root().digest()
            && producer_metadata.payload == DurablePayloadReference::None
            && self
                .terminal_payload
                .same_admission_material(serve_metadata.payload)
            && serve_metadata.replay_authority == self.serve
            && producer_metadata.replay_authority == self.producer
            && self.serve.structurally_matches_record(
                active_context,
                serve_record.key,
                LifecycleWorkClass::CertifiedServe,
                serve_record.stage,
                self.terminal_payload,
            )
            && self.producer.structurally_matches_record(
                active_context,
                producer_record.key,
                LifecycleWorkClass::ProducerTurn,
                producer_record.stage,
                DurablePayloadReference::None,
            )
            && self.serve.same_persisted_family(&self.producer)
    }

    /// Apply the transition-only oracle to decoded ledger coordinates without
    /// exposing either encoded authority outside the lifecycle subsystem.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn exactly_advances_pending_coordinates(
        &self,
        active_context: LifecycleContext,
        serve_key: LifecycleKey,
        serve_owner: OwnerId,
        serve_ordinal: u128,
        serve_stage: LifecycleStage,
        serve_reconstruction_source: LifecycleDigest,
        serve_payload: DurablePayloadReference,
        serve_authority: &LifecycleReplayAuthorityV1,
        producer_key: LifecycleKey,
        producer_owner: OwnerId,
        producer_ordinal: u128,
        producer_stage: LifecycleStage,
        producer_reconstruction_source: LifecycleDigest,
        producer_payload: DurablePayloadReference,
        producer_authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        let DurablePayloadReference::CertifiedServePending { .. } = serve_payload else {
            return false;
        };
        if serve_ordinal.checked_add(1) != Some(producer_ordinal)
            || serve_stage.kind() != LifecycleStageKind::CertifiedServe
            || producer_stage.kind() != LifecycleStageKind::ProducerTurn
            || serve_owner != producer_owner
            || !serve_and_producer_keys_match(serve_key, producer_key)
            || producer_payload != DurablePayloadReference::None
            || serve_reconstruction_source != producer_reconstruction_source
            || serve_reconstruction_source != serve_owner.causal_root().digest()
            || !serve_authority.same_persisted_family(producer_authority)
            || !serve_authority.structurally_matches_record(
                active_context,
                serve_key,
                LifecycleWorkClass::CertifiedServe,
                serve_stage,
                serve_payload,
            )
            || !producer_authority.structurally_matches_record(
                active_context,
                producer_key,
                LifecycleWorkClass::ProducerTurn,
                producer_stage,
                DurablePayloadReference::None,
            )
            || !self.terminal_payload.same_admission_material(serve_payload)
            || !self.serve.structurally_matches_record(
                active_context,
                serve_key,
                LifecycleWorkClass::CertifiedServe,
                serve_stage,
                self.terminal_payload,
            )
            || !self.producer.structurally_matches_record(
                active_context,
                producer_key,
                LifecycleWorkClass::ProducerTurn,
                producer_stage,
                DurablePayloadReference::None,
            )
            || !self.serve.same_persisted_family(&self.producer)
        {
            return false;
        }
        certified_serve_sources_share_origin_except_frame(
            &serve_authority.source,
            &self.serve.source,
        )
    }

    /// Match the exact terminal candidate reconstructed from the same
    /// authenticated recovery frame.
    pub(super) fn exactly_matches_recovered_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        let Some(producer) = candidate.producer_turn.as_ref() else {
            return false;
        };
        candidate.work_class == LifecycleWorkClass::CertifiedServe
            && candidate.payload == self.terminal_payload
            && candidate.replay_authority_is_exact(active_context)
            && candidate.replay_authority == self.serve
            && producer.replay_authority == self.producer
            && self.serve.same_persisted_family(&self.producer)
    }

    /// Replace the Pending projection fields with the exact terminal payload
    /// and authority derived from the same authenticated recovery frame.
    pub(super) fn bind_recovered_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &mut CandidateAdmission,
    ) -> bool {
        if !self.pending_candidate_matches_terminal_family(active_context, candidate) {
            return false;
        }
        candidate.payload = self.terminal_payload;
        candidate.replay_authority = self.serve.clone();
        self.exactly_matches_recovered_candidate(active_context, candidate)
    }

    fn pending_candidate_matches_terminal_family(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        let Some(producer) = candidate.producer_turn.as_ref() else {
            return false;
        };
        matches!(
            candidate.payload,
            DurablePayloadReference::CertifiedServePending { .. }
        ) && candidate.work_class == LifecycleWorkClass::CertifiedServe
            && candidate
                .payload
                .same_admission_material(self.terminal_payload)
            && candidate.replay_authority_is_exact(active_context)
            && candidate
                .replay_authority
                .same_persisted_family(&self.serve)
            && producer.replay_authority == self.producer
            && self.serve.same_persisted_family(&self.producer)
    }

    /// Consume the authenticated frame transition into the exact terminal
    /// payload, outcome, and separately encoded adjacent authorities.
    pub(super) fn consume_terminal_rebind(
        self,
    ) -> (
        DurablePayloadReference,
        TerminalOutcome,
        LifecycleReplayAuthorityV1,
        LifecycleReplayAuthorityV1,
    ) {
        (
            self.terminal_payload,
            self.terminal_outcome,
            self.serve,
            self.producer,
        )
    }
}

fn certified_serve_sources_share_origin_except_frame(
    pending: &LifecycleReplaySourceV1,
    terminal: &LifecycleReplaySourceV1,
) -> bool {
    let (
        LifecycleReplaySourceV1::CertifiedServeStorage(pending),
        LifecycleReplaySourceV1::CertifiedServeStorage(terminal),
    ) = (pending, terminal)
    else {
        return false;
    };
    pending.request == terminal.request && pending.local_retainer == terminal.local_retainer
}

// The pair is consumed only by the fixed adjacent CandidateAdmission factory;
// its decoded ledger descendants remain inert and cannot reconstruct this pair.

impl CertifiedServeReplayEvidencePairV1 {
    /// Seal one exact freshly persisted Pending request and its ProducerTurn.
    pub(super) fn from_post_fsync_pending(
        active_context: LifecycleContext,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        receipt: DurableCertifiedServeAdmissionReceipt,
    ) -> Option<Self> {
        if !receipt.exactly_matches_pending(authenticated)
            || receipt.id().request_hash() != authenticated.request_hash()
            || receipt.certificate_hash() != HashOf::new(&authenticated.request().certificate)
        {
            return None;
        }
        let payload = certified_serve_pending_payload(authenticated);
        let family = exact_certified_serve_storage_replay_family(
            active_context,
            authenticated,
            receipt.payload_hash(),
            receipt.local_retainer(),
        )?;
        let evidence = Self {
            serve: CertifiedServeReplayEvidenceV1 {
                family: Arc::clone(&family),
                payload,
            },
            producer: CertifiedServeProducerTurnReplayEvidenceV1 { family },
        };
        evidence
            .exactly_matches_post_fsync_pending(active_context, authenticated, receipt)
            .then_some(evidence)
    }

    /// Reconstruct the same closed pair only from a fully authenticated
    /// payload-store recovery record.
    pub(super) fn from_authenticated_recovery(
        active_context: LifecycleContext,
        recovered: &AuthenticatedRecoveredCertifiedServePayload,
    ) -> Option<Self> {
        if !recovered.exactly_matches_persisted_payload() {
            return None;
        }
        let payload = recovered_certified_serve_payload(recovered)?;
        let family = exact_certified_serve_storage_replay_family(
            active_context,
            recovered.request(),
            recovered.payload_hash(),
            recovered.local_retainer(),
        )?;
        let evidence = Self {
            serve: CertifiedServeReplayEvidenceV1 {
                family: Arc::clone(&family),
                payload,
            },
            producer: CertifiedServeProducerTurnReplayEvidenceV1 { family },
        };
        evidence
            .exactly_matches_recovered(active_context, recovered)
            .then_some(evidence)
    }

    /// Project one exact shared storage family into the adjacent durable
    /// admission pair without consuming the runtime-only family. Semantic keys,
    /// stages, payloads, authorities, slots, and physical digests are all
    /// derived here. The same pair can then move, whole, into the two concrete
    /// registry carriers.
    pub(super) fn admission_candidate(
        &self,
        active_context: LifecycleContext,
    ) -> Option<CandidateAdmission> {
        let storage_payload = self.serve.payload.durable_payload()?;
        let storage_payload_hash = Hash::prehashed(self.serve.family.source.payload_hash);
        let serve_stage = LifecycleStage::new(
            LifecycleStageKind::CertifiedServe,
            PredecessorScope::ReadyOrdinalPrefix,
        );
        let producer_stage = LifecycleStage::new(
            LifecycleStageKind::ProducerTurn,
            PredecessorScope::ProducerHandoffBarrier,
        );
        let serve_shape = self
            .serve
            .family
            .source
            .project(active_context, serve_stage.kind(), &self.serve.payload)
            .ok()?;
        let producer_shape = self
            .producer
            .family
            .source
            .project(
                active_context,
                producer_stage.kind(),
                &ReplayPayloadBindingV1::None,
            )
            .ok()?;
        if !self.exactly_matches_serve_record(
            active_context,
            serve_shape.key,
            serve_stage,
            storage_payload,
            storage_payload_hash,
        ) || !self.exactly_matches_producer_record(
            active_context,
            producer_shape.key,
            producer_stage,
            DurablePayloadReference::None,
            storage_payload_hash,
        ) {
            return None;
        }
        let source =
            LifecycleReplaySourceV1::CertifiedServeStorage(self.serve.family.source.clone());
        let request = digest_from_bytes(HashOf::new(&self.serve.family.source.request).as_ref());
        let certificate =
            digest_from_bytes(HashOf::new(&self.serve.family.source.request.certificate).as_ref());
        let serve_payload = DurablePayloadReference::certified_serve_pending(request, certificate);
        let serve_authority = LifecycleReplayAuthorityV1::decode_canonical(
            &LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::from_payload(serve_payload),
                source: source.clone(),
            }
            .encode(),
        )
        .ok()?;
        let producer_authority = LifecycleReplayAuthorityV1::decode_canonical(
            &LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::None,
                source,
            }
            .encode(),
        )
        .ok()?;
        let reconstruction_source = request;
        let serve_slot =
            PhysicalSlotId::for_capacity(LifecycleWorkClass::CertifiedServe.capacity_class(), 0);
        let producer_slot =
            PhysicalSlotId::for_capacity(LifecycleWorkClass::ProducerTurn.capacity_class(), 0);
        Some(CandidateAdmission::new(
            serve_shape.key,
            CausalRoot::new(reconstruction_source),
            LifecycleWorkClass::CertifiedServe,
            serve_stage,
            InitialLifecycleState::Ready,
            reconstruction_source,
            serve_payload,
            serve_authority,
            PhysicalGeometry::new(
                [PhysicalSlot::new(
                    serve_slot,
                    digest_from_hash(&storage_payload_hash),
                )],
                [serve_slot],
            ),
            Some(ProducerTurnAdmission::new(
                producer_shape.key,
                producer_stage,
                reconstruction_source,
                producer_authority,
                PhysicalGeometry::new(
                    [PhysicalSlot::new(
                        producer_slot,
                        self.producer_physical_digest(),
                    )],
                    [producer_slot],
                ),
            )),
        ))
    }

    /// Compare the retained Serve evidence with one exact logical record and
    /// its independently retained payload-store frame hash.
    pub(super) fn exactly_matches_serve_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        storage_payload_hash: Hash,
    ) -> bool {
        self.shares_exact_storage_origin()
            && self.serve.exactly_matches_record(
                active_context,
                key,
                stage,
                payload,
                storage_payload_hash,
            )
    }

    /// Match one exact terminal Serve row without inventing an executable
    /// physical carrier. Steady terminal Ledger rows reopen with empty geometry,
    /// while payload-store-ahead reconciliation may retain the former Pending
    /// geometry in its reconciled tombstone. Neither shape is executable: the
    /// retained storage family derives its own frame hash and must still match
    /// the complete logical/durable authority.
    pub(super) fn exactly_matches_terminal_serve_record(
        &self,
        active_context: LifecycleContext,
        record: &LifecycleRecord,
        metadata: &DurableRecordMetadata,
    ) -> bool {
        let super::LifecycleState::Terminal(outcome) = record.state else {
            return false;
        };
        let storage_payload_hash = Hash::prehashed(self.serve.family.source.payload_hash);
        record.work_class == LifecycleWorkClass::CertifiedServe
            && record.owner.causal_root().digest() == metadata.reconstruction_source
            && metadata
                .payload
                .matches_terminal(LifecycleWorkClass::CertifiedServe, Some(outcome))
            && self.exactly_matches_serve_record(
                active_context,
                record.key,
                record.stage,
                metadata.payload,
                storage_payload_hash,
            )
            && self
                .serve
                .exactly_matches_authority(&metadata.replay_authority)
    }

    /// Compare the retained ProducerTurn evidence with one exact dormant
    /// logical record while retaining the same payload-store origin.
    pub(super) fn exactly_matches_producer_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        storage_payload_hash: Hash,
    ) -> bool {
        self.shares_exact_storage_origin()
            && self.producer.exactly_matches_record(
                active_context,
                key,
                stage,
                payload,
                storage_payload_hash,
            )
    }

    fn shares_exact_storage_origin(&self) -> bool {
        Arc::ptr_eq(&self.serve.family, &self.producer.family)
    }

    /// Match the exact one-slot Serve carrier without exposing the payload-store
    /// frame hash retained by this family.
    pub(super) fn exactly_matches_serve_carrier(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        physical_digest: LifecycleDigest,
        replay_authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        let storage_payload_hash = Hash::prehashed(self.serve.family.source.payload_hash);
        physical_digest == digest_from_hash(&storage_payload_hash)
            && self.exactly_matches_serve_record(
                active_context,
                key,
                stage,
                payload,
                storage_payload_hash,
            )
            && self.serve.exactly_matches_authority(replay_authority)
    }

    /// Match the exact one-slot ProducerTurn carrier while retaining the same
    /// opaque payload-store family as its Serve origin.
    pub(super) fn exactly_matches_producer_carrier(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        physical_digest: LifecycleDigest,
        replay_authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        let storage_payload_hash = Hash::prehashed(self.serve.family.source.payload_hash);
        physical_digest == self.producer_physical_digest()
            && self.exactly_matches_producer_record(
                active_context,
                key,
                stage,
                payload,
                storage_payload_hash,
            )
            && self.producer.exactly_matches_authority(replay_authority)
    }

    fn producer_physical_digest(&self) -> LifecycleDigest {
        certified_serve_producer_physical_digest(&self.serve.family.source)
    }

    fn exactly_matches_post_fsync_pending(
        &self,
        active_context: LifecycleContext,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        receipt: DurableCertifiedServeAdmissionReceipt,
    ) -> bool {
        self.shares_exact_storage_origin()
            && receipt.exactly_matches_pending(authenticated)
            && self.serve.payload == certified_serve_pending_payload(authenticated)
            && self
                .serve
                .family
                .source
                .project(
                    active_context,
                    LifecycleStageKind::CertifiedServe,
                    &self.serve.payload,
                )
                .is_ok()
            && exact_certified_serve_storage_replay_family(
                active_context,
                authenticated,
                receipt.payload_hash(),
                receipt.local_retainer(),
            )
            .is_some_and(|expected| {
                expected.as_ref() == self.serve.family.as_ref()
                    && receipt.id().request_hash() == authenticated.request_hash()
                    && receipt.certificate_hash()
                        == HashOf::new(&authenticated.request().certificate)
            })
    }

    fn exactly_matches_recovered(
        &self,
        active_context: LifecycleContext,
        recovered: &AuthenticatedRecoveredCertifiedServePayload,
    ) -> bool {
        self.shares_exact_storage_origin()
            && recovered.exactly_matches_persisted_payload()
            && recovered_certified_serve_payload(recovered).as_ref() == Some(&self.serve.payload)
            && self
                .serve
                .family
                .source
                .project(
                    active_context,
                    LifecycleStageKind::CertifiedServe,
                    &self.serve.payload,
                )
                .is_ok()
            && exact_certified_serve_storage_replay_family(
                active_context,
                recovered.request(),
                recovered.payload_hash(),
                recovered.local_retainer(),
            )
            .is_some_and(|expected| {
                expected.as_ref() == self.serve.family.as_ref()
                    && recovered.id().request_hash() == recovered.request().request_hash()
                    && recovered.certificate_hash()
                        == HashOf::new(&recovered.request().request().certificate)
            })
    }
}

impl CertifiedServeReplayEvidenceV1 {
    fn exactly_matches_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        storage_payload_hash: Hash,
    ) -> bool {
        self.family.source.payload_hash == *storage_payload_hash.as_ref()
            && self.exactly_matches_authority(&LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::from_payload(payload),
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            })
            && LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: self.payload.clone(),
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            }
            .validate_record(
                active_context,
                key,
                LifecycleWorkClass::CertifiedServe,
                stage,
                payload,
            )
            .is_ok()
    }

    fn exactly_matches_authority(&self, authority: &LifecycleReplayAuthorityV1) -> bool {
        authority
            == &LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: self.payload.clone(),
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            }
    }
}

impl CertifiedServeProducerTurnReplayEvidenceV1 {
    fn exactly_matches_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
        storage_payload_hash: Hash,
    ) -> bool {
        self.family.source.payload_hash == *storage_payload_hash.as_ref()
            && self.exactly_matches_authority(&LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::from_payload(payload),
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            })
            && LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::None,
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            }
            .validate_record(
                active_context,
                key,
                LifecycleWorkClass::ProducerTurn,
                stage,
                payload,
            )
            .is_ok()
    }

    fn exactly_matches_authority(&self, authority: &LifecycleReplayAuthorityV1) -> bool {
        authority
            == &LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: ReplayPayloadBindingV1::None,
                source: LifecycleReplaySourceV1::CertifiedServeStorage(self.family.source.clone()),
            }
    }
}

fn certified_serve_producer_physical_digest(
    source: &CertifiedServeStorageSourceV1,
) -> LifecycleDigest {
    let request_hash = HashOf::new(&source.request);
    let mut projection =
        Vec::with_capacity(PRODUCER_TURN_PHYSICAL_DOMAIN.len() + size_of::<u64>() + Hash::LENGTH);
    projection.extend_from_slice(PRODUCER_TURN_PHYSICAL_DOMAIN);
    append_field(&mut projection, request_hash.as_ref());
    digest_from_hash(&Hash::new(projection))
}

fn exact_certified_serve_storage_replay_family(
    active_context: LifecycleContext,
    authenticated: &AuthenticatedCertifiedBodyRequest,
    storage_payload_hash: Hash,
    local_retainer: wire::ValidatorIndex,
) -> Option<Arc<CertifiedServeStorageReplayFamilyV1>> {
    let request = authenticated.request();
    let local_retainer_index = usize::try_from(local_retainer).ok()?;
    if authenticated.request_hash() != HashOf::new(request)
        || local_retainer_index >= wire::MAX_VALIDATORS_PER_HEIGHT
        || request
            .certificate
            .signers
            .binary_search(&local_retainer)
            .is_err()
    {
        return None;
    }
    let source = CertifiedServeStorageSourceV1 {
        request: request.clone(),
        payload_hash: *storage_payload_hash.as_ref(),
        local_retainer,
    };
    let serve = source
        .project(
            active_context,
            LifecycleStageKind::CertifiedServe,
            &certified_serve_pending_payload(authenticated),
        )
        .ok()?;
    let producer = source
        .project(
            active_context,
            LifecycleStageKind::ProducerTurn,
            &ReplayPayloadBindingV1::None,
        )
        .ok()?;
    if super::schema::producer_turn_key_for_serve(serve.key) != Some(producer.key)
        || serve.work_class != LifecycleWorkClass::CertifiedServe
        || producer.work_class != LifecycleWorkClass::ProducerTurn
    {
        return None;
    }
    Some(Arc::new(CertifiedServeStorageReplayFamilyV1 { source }))
}

fn certified_serve_pending_payload(
    authenticated: &AuthenticatedCertifiedBodyRequest,
) -> ReplayPayloadBindingV1 {
    ReplayPayloadBindingV1::CertifiedServePending {
        request: *authenticated.request_hash().as_ref(),
        certificate: *HashOf::new(&authenticated.request().certificate).as_ref(),
    }
}

fn recovered_certified_serve_payload(
    recovered: &AuthenticatedRecoveredCertifiedServePayload,
) -> Option<ReplayPayloadBindingV1> {
    let request = recovered.request();
    let request_hash = request.request_hash();
    let certificate_hash = recovered.certificate_hash();
    if recovered.id().request_hash() != request_hash
        || request_hash != HashOf::new(request.request())
        || certificate_hash != HashOf::new(&request.request().certificate)
    {
        return None;
    }
    Some(match recovered.state() {
        AuthenticatedRecoveredCertifiedServePayloadState::Pending => {
            ReplayPayloadBindingV1::CertifiedServePending {
                request: *request_hash.as_ref(),
                certificate: *certificate_hash.as_ref(),
            }
        }
        AuthenticatedRecoveredCertifiedServePayloadState::Completed(completed) => {
            ReplayPayloadBindingV1::CertifiedServeCompleted {
                request: *request_hash.as_ref(),
                certificate: *certificate_hash.as_ref(),
                response: *completed.response_hash().as_ref(),
            }
        }
        AuthenticatedRecoveredCertifiedServePayloadState::Negative(outcome) => {
            let outcome = match outcome {
                CertifiedServePayloadNegativeOutcome::Cancelled => {
                    DurableServeNegativeOutcome::Cancelled
                }
                CertifiedServePayloadNegativeOutcome::Rejected(code) => {
                    DurableServeNegativeOutcome::Rejected(*code)
                }
                CertifiedServePayloadNegativeOutcome::Failed(code) => {
                    DurableServeNegativeOutcome::Failed(*code)
                }
            };
            ReplayPayloadBindingV1::from_payload(DurablePayloadReference::CertifiedServeNegative {
                request: LifecycleDigest::new(*request_hash.as_ref()),
                certificate: LifecycleDigest::new(*certificate_hash.as_ref()),
                outcome,
            })
        }
    })
}

include!("v2_lifecycle_replay_authority_certified_body.rs");

include!("v2_lifecycle_replay_authority_payload_projection.rs");
#[cfg(test)]
mod tests {
    include!("tests/v2_lifecycle_replay_authority_fixtures.rs");
    include!("tests/v2_lifecycle_replay_authority_cases.rs");
}

#[cfg(test)]
pub(super) use tests::{
    durable_certified_fetch_projection_fixture, exact_body_record_fixture,
    exact_durable_certified_fetch_record_fixture, exact_local_body_record_fixture,
    exact_record_fixture, exact_recovered_decision_terminal_family_fixture,
    exact_replay_authority_for_payload_fixture, foreign_certified_serve_family_authority_fixture,
};
