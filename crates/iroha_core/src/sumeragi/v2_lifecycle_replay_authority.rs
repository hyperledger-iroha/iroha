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
        CandidateAdmission, CausalRoot, DurableBodyFrameReference,
        DurablePayloadReference, DurableRecordMetadata, DurableServeNegativeOutcome,
        InitialLifecycleState, LifecycleContext, LifecycleDigest, LifecycleKey, LifecyclePhase,
        LifecycleRecord, LifecycleRound, LifecycleStage, LifecycleStageKind, LifecycleWorkClass,
        OwnerId, PhysicalGeometry, PhysicalSlot, PhysicalSlotId, PredecessorScope,
        ProducerTurnAdmission, TerminalOutcome, serve_and_producer_keys_match,
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
pub(super) struct LifecycleReplayAuthorityV1 {
    format_version: u16,
    payload: ReplayPayloadBindingV1,
    source: LifecycleReplaySourceV1,
}

impl LifecycleReplayAuthorityV1 {
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
        ) = (&self.source, self.payload)
        else {
            return None;
        };
        let family = CertifiedBodyPipelineReplayFamilyV1 {
            source: source.clone(),
            body_frame,
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
            self.apply,
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

    /// Insert the final Apply candidate only when its exact live row owns it.
    pub(super) fn splice_apply_candidate(
        &self,
        record: &LifecycleLedgerRecordV1,
        candidates: &mut std::collections::BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        Self::candidate_matches_record(
            &self.apply,
            record,
            record.owner(),
            None,
            super::schema::DurableContinuation::None,
        ) && candidates
            .insert(self.apply.key, self.apply.clone())
            .is_none()
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
    pub(super) fn bind_exact_validate_sign_pending(
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
    pub(super) fn exactly_binds_validate_sign_pending(&self) -> bool {
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
            let response = completed.response();
            if response.request_hash != request_hash
                || response.manifest.round != request.request().round
                || response.manifest.subject != request.request().subject
            {
                return None;
            }
            ReplayPayloadBindingV1::CertifiedServeCompleted {
                request: *request_hash.as_ref(),
                certificate: *certificate_hash.as_ref(),
                response: *HashOf::new(response).as_ref(),
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
            ReplayPayloadBindingV1::CertifiedServeNegative {
                request: *request_hash.as_ref(),
                certificate: *certificate_hash.as_ref(),
                outcome,
            }
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CertifiedBodyPipelineReplayFamilyV1 {
    source: BodyPipelineReplaySourceV1,
    body_frame: BodyFrameBindingV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CertifiedBodyPipelineCoordinatesV1 {
    tag: ReplayEventTagV1,
    certificate: wire::QuorumCertificate,
    manifest: wire::PayloadManifest,
    fetch_manifest_present: bool,
    certified_sources: Vec<PeerId>,
}

impl AuthenticatedCertifiedFetchReplayOriginV1 {
    /// Bind the exact selector-authenticated response to its pending Fetch.
    pub(super) fn from_completion_authority(
        authority: &CertifiedFetchCompletionAuthority<'_>,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        let response = authority.authenticated_response();
        if authority.request_hash() != response.request_hash
            || authority.response_hash() != HashOf::new(response)
            || !authority
                .candidate_pending()
                .exactly_binds_adapter_effect(effect)
        {
            return None;
        }
        Some(Self {
            coordinates: exact_certified_fetch_coordinates(effect, response)?,
            request_hash: authority.request_hash(),
            response_hash: authority.response_hash(),
        })
    }

    /// Consume the authenticated origin into one frame-bound canonical family.
    pub(super) fn bind_durable_body(
        self,
        receipt: &DurableCertifiedFetchBodyReceipt,
    ) -> Option<CertifiedFetchReplayEvidenceV1> {
        if receipt.request_hash() != self.request_hash
            || receipt.response_hash() != self.response_hash
        {
            return None;
        }
        Some(CertifiedFetchReplayEvidenceV1 {
            family: exact_certified_body_pipeline_family(
                &self.coordinates,
                receipt.durable_body(),
            )?,
        })
    }
}

impl CertifiedFetchReplayEvidenceV1 {
    /// Reauthenticate the persisted certificate and exact archive-source order
    /// against the immutable height context before restart authority is minted.
    fn authenticated_by_verified_height(&self, verified: &VerifiedHeightContext) -> bool {
        let BodyPipelineOriginV1::Certified {
            certificate,
            certified_sources,
            ..
        } = &self.family.source.origin
        else {
            return false;
        };
        let expected_sources = verified
            .context()
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        replay_context(certificate.round)
            == super::projection::lifecycle_context(verified.context())
            && certified_sources == &expected_sources
            && verified.verify_quorum_certificate(certificate).is_ok()
    }

    /// Compare this complete canonical family with the exact installed Fetch.
    pub(super) fn exactly_matches_fetch(
        &self,
        effect: &AdapterEffect,
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableCertifiedFetchBodyReceipt,
    ) -> bool {
        if receipt.request_hash() != response.request_hash
            || receipt.response_hash() != HashOf::new(response)
        {
            return false;
        }
        self.exactly_matches_fetch_body(effect, response, receipt.durable_body())
    }

    fn exactly_matches_fetch_body(
        &self,
        effect: &AdapterEffect,
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        exact_certified_fetch_coordinates(effect, response)
            .and_then(|coordinates| certified_body_pipeline_family(&coordinates, receipt))
            .is_some_and(|expected| {
                expected == self.family
                    && self
                        .family
                        .is_exact_for_stage(LifecycleStageKind::FetchBody)
            })
    }

    /// Close this family over the exact incumbent runtime binding and durable frame.
    pub(super) fn project_durable_ready_fetch(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Option<DurableCertifiedFetchReplayProjectionV1> {
        if exact_certified_fetch_effect(&self.family).as_ref() != Some(effect)
            || !pending.exactly_binds_adapter_effect(effect)
            || !certified_body_pipeline_family(&exact_family_coordinates(&self.family)?, receipt)
                .is_some_and(|expected| expected == self.family)
        {
            return None;
        }
        durable_certified_fetch_projection(&self.family, effect, pending, receipt)
    }

    /// Reconstruct the exact Fetch effect and pending binding from a durable owner.
    ///
    /// Decoded replay data alone cannot invoke the pending constructor: the
    /// one-shot permit is minted only while this frame-bound evidence remains
    /// intact.
    fn reconstruct_exact_fetch(
        &self,
        causal_root: CausalRoot,
    ) -> Option<(AdapterEffect, PendingRuntimeEffectBinding)> {
        let effect = exact_certified_fetch_effect(&self.family)?;
        let pending = PendingRuntimeEffectBinding::from_durable_certified_fetch(
            DurableCertifiedFetchPendingMintPermit::new(),
            Hash::prehashed(*causal_root.digest().as_bytes()),
            &effect,
        )?;
        (digest_from_hash(pending.causal_lifecycle_key()) == causal_root.digest()
            && self
                .family
                .is_exact_for_stage(LifecycleStageKind::FetchBody))
        .then_some((effect, pending))
    }

    /// Authenticate one opened body-store seal against this exact replay family.
    pub(super) fn exactly_matches_recovered_body_frame(
        &self,
        reference: &DurableBodyFrameReference,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        let Some(coordinates) = exact_family_coordinates(&self.family) else {
            return false;
        };
        coordinates.manifest == *manifest
            && self.family.body_frame.durable_reference() == *reference
            && durable_body_frame_reference(replay_context(receipt.round()), receipt)
                == Some(*reference)
            && certified_body_pipeline_family(&coordinates, receipt)
                .is_some_and(|expected| expected == self.family)
    }

    /// Derive the direct adapter preview inputs from the sealed durable family.
    pub(super) fn adapter_preview_inputs<'a>(
        &'a self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Option<(EventTag, &'a wire::PayloadManifest)> {
        self.project_durable_ready_fetch(effect, pending, receipt)?;
        let BodyPipelineOriginV1::Certified { manifest, .. } = &self.family.source.origin else {
            return None;
        };
        Some((
            EventTag::new(
                self.family.source.tag.height,
                self.family.source.tag.view,
                crate::sumeragi::v2_core::Generation::new(self.family.source.tag.generation),
            ),
            manifest,
        ))
    }

    #[cfg(test)]
    fn exactly_matches_signed_response_for_test(
        &self,
        effect: &AdapterEffect,
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        signature_present(&response.signature)
            && self.exactly_matches_fetch_body(effect, response, receipt)
    }

    /// Project the fixed Store-stage evidence without exposing source parts.
    pub(super) fn project_store(
        &self,
        fetch_effect: &AdapterEffect,
        fetch_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        store_effect: &AdapterEffect,
    ) -> Option<CertifiedStoreReplayEvidenceV1> {
        (self
            .project_durable_ready_fetch(fetch_effect, fetch_pending, receipt)
            .is_some()
            && certified_body_stage_matches(
                &self.family,
                store_effect,
                receipt,
                LifecycleStageKind::StoreBody,
            ))
        .then(|| CertifiedStoreReplayEvidenceV1 {
            family: self.family.clone(),
        })
    }

    #[cfg(test)]
    pub(super) fn from_signed_response_for_test(
        fetch_effect: &AdapterEffect,
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableBodyReceipt,
    ) -> Option<Self> {
        if !signature_present(&response.signature) {
            return None;
        }
        Some(Self {
            family: exact_certified_body_pipeline_family(
                &exact_certified_fetch_coordinates(fetch_effect, response)?,
                receipt,
            )?,
        })
    }

    #[cfg(test)]
    pub(super) fn project_store_for_test(
        &self,
        store_effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> Option<CertifiedStoreReplayEvidenceV1> {
        certified_body_stage_matches(
            &self.family,
            store_effect,
            receipt,
            LifecycleStageKind::StoreBody,
        )
        .then(|| CertifiedStoreReplayEvidenceV1 {
            family: self.family.clone(),
        })
    }
}

impl DurableCertifiedFetchReplayProjectionV1 {
    /// Compare the complete frame-bound projection with one logical restart row.
    pub(super) fn exactly_matches_recovered_candidate(
        &self,
        candidate: &CandidateAdmission,
        owner: OwnerId,
    ) -> bool {
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Fetch.capacity_class(), 0);
        candidate.key.phase() == LifecyclePhase::Fetch
            && candidate.causal_root == owner.causal_root()
            && candidate.work_class == LifecycleWorkClass::Fetch
            && candidate.stage
                == LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent)
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.reconstruction_source == owner.causal_root().digest()
            && candidate.payload == self.payload
            && candidate.replay_authority == self.authority
            && candidate.producer_turn.is_none()
            && self.causal_key == Hash::prehashed(*owner.causal_root().digest().as_bytes())
            && self.authority.structurally_matches_record(
                LifecycleContext::new(candidate.key.context(), candidate.key.round().height()),
                candidate.key,
                candidate.work_class,
                candidate.stage,
                candidate.payload,
            )
            && candidate.physical_geometry.normalized().is_ok_and(
                |(physical, universe, consumed)| {
                    physical.len() == 1
                        && physical.get(&slot) == Some(&self.completion_digest)
                        && universe == std::collections::BTreeSet::from([slot])
                        && consumed == universe
                },
            )
    }

    /// Canonical physical identity of the body-fsynced completion.
    pub(super) const fn completion_digest(&self) -> LifecycleDigest {
        self.completion_digest
    }

    /// Exact manifest hash retained independently by the body-store receipt.
    pub(super) const fn expected_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.expected_manifest_hash
    }

    /// Recheck the exact runtime binding and durable body without exposing fields.
    pub(super) fn exactly_matches_runtime(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        pending.exactly_binds_adapter_effect(effect)
            && pending.causal_lifecycle_key() == &self.causal_key
            && pending.exact_effect_identity() == &self.effect_identity
            && receipt.manifest_hash() == self.expected_manifest_hash
            && durable_body_frame_reference(replay_context(receipt.round()), receipt)
                .map(DurablePayloadReference::BodyFrame)
                == Some(self.payload)
            && canonical_replay_authority(
                replay_context(receipt.round()),
                self.authority.source.clone(),
                LifecycleStageKind::FetchBody,
                ReplayPayloadBindingV1::from_payload(self.payload),
            ) == Some(self.authority.clone())
    }

    /// Project the exact Ready recovery candidate named by one durable row.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn project_recovered_candidate(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        owner: OwnerId,
        stage: LifecycleStage,
        reconstruction_source: LifecycleDigest,
        payload: DurablePayloadReference,
        persisted_authority: &LifecycleReplayAuthorityV1,
    ) -> Option<CandidateAdmission> {
        if stage
            != LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent)
            || !self.exactly_matches_durable_record(
                active_context,
                key,
                owner.causal_root(),
                payload,
                reconstruction_source,
                persisted_authority,
            )
        {
            return None;
        }
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Fetch.capacity_class(), 0);
        let candidate = CandidateAdmission::new(
            key,
            owner.causal_root(),
            LifecycleWorkClass::Fetch,
            stage,
            InitialLifecycleState::Ready,
            reconstruction_source,
            self.payload,
            self.authority.clone(),
            PhysicalGeometry::new([PhysicalSlot::new(slot, self.completion_digest)], [slot]),
            None,
        );
        candidate
            .replay_authority_is_exact(active_context)
            .then_some(candidate)
    }

    /// Rebind only the durable fields of an exact Waiting Fetch row.
    pub(super) fn rebind_waiting_fetch_metadata(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        metadata: &mut DurableRecordMetadata,
    ) -> bool {
        if key.phase() != LifecyclePhase::Fetch
            || metadata.payload != DurablePayloadReference::None
            || metadata.reconstruction_source != digest_from_hash(&self.causal_key)
            || metadata.continuation != super::schema::DurableContinuation::None
            || !metadata
                .replay_authority
                .same_persisted_family(&self.authority)
            || !self.authority.structurally_matches_record(
                active_context,
                key,
                LifecycleWorkClass::Fetch,
                LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
                self.payload,
            )
        {
            return false;
        }
        metadata.payload = self.payload;
        metadata.replay_authority = self.authority.clone();
        true
    }

    /// Compare a recovered ledger row without exposing authority parts.
    pub(super) fn exactly_matches_durable_record(
        &self,
        active_context: LifecycleContext,
        key: LifecycleKey,
        causal_root: CausalRoot,
        metadata_payload: DurablePayloadReference,
        reconstruction_source: LifecycleDigest,
        authority: &LifecycleReplayAuthorityV1,
    ) -> bool {
        self.payload == metadata_payload
            && self.authority == *authority
            && reconstruction_source == causal_root.digest()
            && self.causal_key == Hash::prehashed(*causal_root.digest().as_bytes())
            && self.authority.structurally_matches_record(
                active_context,
                key,
                LifecycleWorkClass::Fetch,
                LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
                self.payload,
            )
    }
}

/// Consume the sole opened-ledger/body-store join into restart authority.
#[allow(clippy::too_many_arguments)]
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn authenticate_recovered_durable_certified_fetch<
    F,
>(
    _permit: DurableCertifiedFetchLedgerJoinPermit,
    verified: &VerifiedHeightContext,
    key: LifecycleKey,
    owner: OwnerId,
    ordinal: u128,
    stage: LifecycleStage,
    reconstruction_source: LifecycleDigest,
    payload: DurablePayloadReference,
    authority: &LifecycleReplayAuthorityV1,
    authenticate_body: F,
) -> Result<Option<AuthenticatedRecoveredDurableCertifiedFetchV1>, DurableBodyFrameRecoveryError>
where
    F: FnOnce() -> Result<AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError>,
{
    let active_context = super::projection::lifecycle_context(verified.context());
    if ordinal == 0
        || owner.first_admission_ordinal() == 0
        || owner.first_admission_ordinal() > ordinal
        || reconstruction_source != owner.causal_root().digest()
    {
        return Ok(None);
    }
    let Some(evidence) =
        authority.recover_durable_certified_fetch(active_context, key, stage, payload)
    else {
        return Ok(None);
    };
    if !evidence.authenticated_by_verified_height(verified) {
        return Ok(None);
    }
    // The body-store seal is minted only after the retained source list and QC
    // have been authenticated by the immutable verified height context.
    let body = authenticate_body()?;
    let Some(durable_receipt) = body.into_certified_fetch_body(&evidence) else {
        return Ok(None);
    };
    let Some((effect, pending)) = evidence.reconstruct_exact_fetch(owner.causal_root()) else {
        return Ok(None);
    };
    let Some(ready_projection) =
        evidence.project_durable_ready_fetch(&effect, &pending, &durable_receipt)
    else {
        return Ok(None);
    };
    let Some(candidate) = ready_projection.project_recovered_candidate(
        active_context,
        key,
        owner,
        stage,
        reconstruction_source,
        payload,
        authority,
    ) else {
        return Ok(None);
    };
    let Ok(completion) = CertifiedFetchCompletion::from_recovered_durable_fetch(
        owner,
        ordinal,
        effect,
        pending,
        durable_receipt,
        evidence,
        &ready_projection,
    ) else {
        return Ok(None);
    };
    let recovered = AuthenticatedRecoveredDurableCertifiedFetchV1 {
        completion,
        candidate,
    };
    Ok(recovered.is_exact().then_some(recovered))
}

impl CertifiedStoreReplayEvidenceV1 {
    /// Compare this canonical family with one exact durable Store carrier.
    pub(super) fn exactly_matches_store(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        certified_body_stage_matches(&self.family, effect, receipt, LifecycleStageKind::StoreBody)
    }

    /// Project one installed Store carrier without exposing its replay family.
    ///
    /// The registry-only one-shot permit proves the evidence, durable frame,
    /// concrete effect, and pending binding still reside in one closed carrier.
    pub(in crate::sumeragi) fn project_installed_store_candidate(
        &self,
        _permit: InstalledBodyCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_store_candidate(verified, effect, receipt, pending)
    }

    /// Project one Store successor still sealed under its exact Fetch parent.
    pub(in crate::sumeragi) fn project_sealed_store_successor_candidate(
        &self,
        _permit: SealedBodySuccessorProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_store_candidate(verified, effect, receipt, pending)
    }

    fn project_exact_store_candidate(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_matches_store(effect, receipt) {
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

    /// Project the canonical Store candidate for focused transition tests.
    #[cfg(test)]
    pub(super) fn project_candidate_for_test(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_store_candidate(verified, effect, receipt, pending)
    }

    /// Replace only the retained event origin in a negative test fixture.
    #[cfg(test)]
    pub(super) fn replace_with_foreign_origin_for_test(&mut self) -> bool {
        let previous = self.family.source.tag;
        self.family.source.tag.generation = previous.generation.wrapping_add(1);
        self.family.source.tag != previous
    }

    /// Project the fixed Validate-stage evidence without exposing source parts.
    pub(super) fn project_validate(
        &self,
        store_effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> Option<CertifiedValidateReplayEvidenceV1> {
        if !self.exactly_matches_store(store_effect, receipt)
            || !certified_body_stage_matches(
                &self.family,
                validate_effect,
                receipt,
                LifecycleStageKind::ValidateBody,
            )
        {
            return None;
        }
        Some(CertifiedValidateReplayEvidenceV1 {
            family: self.family.clone(),
            validate_pending: DirectSignedPendingBindingV1::from_exact_effect(
                validate_effect,
                validate_pending,
            )?,
        })
    }
}

impl CertifiedValidateReplayEvidenceV1 {
    /// Compare this canonical family and causal root with one exact Validate carrier.
    fn exactly_matches_validate_pending(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        certified_body_stage_matches(
            &self.family,
            effect,
            receipt,
            LifecycleStageKind::ValidateBody,
        ) && self.validate_pending.exactly_matches(effect, pending)
    }

    /// Revalidate the canonical family against its retained durable frame.
    pub(super) fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        exact_family_coordinates(&self.family)
            .and_then(|coordinates| certified_body_pipeline_family(&coordinates, receipt))
            .is_some_and(|expected| {
                expected == self.family
                    && self
                        .family
                        .is_exact_for_stage(LifecycleStageKind::ValidateBody)
            })
    }
}

impl DurableValidateReplayEvidenceV1 {
    /// Wrap one exact certified Validate family without exposing its source.
    pub(super) const fn certified(evidence: CertifiedValidateReplayEvidenceV1) -> Self {
        Self::Certified(evidence)
    }

    /// Wrap one exact ordinary remote-Proposal Validate family.
    pub(super) const fn remote_proposal(evidence: RemoteProposalValidateReplayEvidenceV1) -> Self {
        Self::RemoteProposal(evidence)
    }

    /// Compare the closed family with one exact Validate effect, body frame,
    /// and causal pending binding.
    pub(in crate::sumeragi) fn exactly_matches_validate_pending(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        match self {
            Self::Certified(evidence) => {
                evidence.exactly_matches_validate_pending(effect, receipt, pending)
            }
            Self::RemoteProposal(evidence) => {
                evidence.exactly_matches_validate_pending(effect, receipt, pending)
            }
        }
    }

    /// Revalidate the closed family against its retained durable body frame.
    pub(super) fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        match self {
            Self::Certified(evidence) => evidence.exactly_matches_durable_body(receipt),
            Self::RemoteProposal(evidence) => {
                remote_proposal_validate_matches_durable_body(evidence, receipt)
            }
        }
    }

    /// Project one installed Validate carrier without exposing its replay family.
    ///
    /// The registry-only one-shot permit proves the evidence, durable frame,
    /// concrete effect, and pending binding still reside in one closed carrier.
    pub(in crate::sumeragi) fn project_installed_validate_candidate(
        &self,
        _permit: InstalledBodyCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_validate_candidate(verified, effect, receipt, pending)
    }

    /// Project one Validate successor still sealed under its exact Store parent.
    pub(in crate::sumeragi) fn project_sealed_validate_successor_candidate(
        &self,
        _permit: SealedBodySuccessorProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_validate_candidate(verified, effect, receipt, pending)
    }

    /// Replace only the retained event origin in a negative test fixture.
    #[cfg(test)]
    pub(super) fn replace_with_foreign_origin_for_test(&mut self) -> bool {
        let source = match self {
            Self::Certified(evidence) => &mut evidence.family.source,
            Self::RemoteProposal(evidence) => &mut evidence.family.source,
        };
        let previous = source.tag;
        source.tag.generation = previous.generation.wrapping_add(1);
        source.tag != previous
    }

    /// Join this retained Validate origin to the exact body frame and runtime owner.
    ///
    /// The canonical body-pipeline authority remains private and is attached
    /// only after the runtime projection, durable receipt, and retained
    /// pending fingerprint all agree exactly.
    pub(in crate::sumeragi) fn project_recovered_validate_candidate(
        &self,
        _permit: RecoveredWalCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<CandidateAdmission> {
        self.project_exact_validate_candidate(verified, effect, receipt, pending)
            .ok()
    }

    fn project_exact_validate_candidate(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_matches_validate_pending(effect, receipt, pending) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = replay_context(receipt.round());
        let payload = DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(active_context, receipt)
                .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?,
        );
        let payload_binding = ReplayPayloadBindingV1::from_payload(payload);
        let source = match self {
            Self::Certified(evidence) => {
                if payload_binding != ReplayPayloadBindingV1::BodyFrame(evidence.family.body_frame)
                {
                    return Err(AdapterEffectAdmissionError::InvalidCarrier);
                }
                evidence.family.source.clone()
            }
            Self::RemoteProposal(evidence) => {
                if payload_binding != ReplayPayloadBindingV1::BodyFrame(evidence.family.body_frame)
                {
                    return Err(AdapterEffectAdmissionError::InvalidCarrier);
                }
                evidence.family.source.clone()
            }
        };
        let authority = canonical_replay_authority(
            active_context,
            LifecycleReplaySourceV1::BodyPipeline(source),
            LifecycleStageKind::ValidateBody,
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

    /// Project the canonical Validate candidate for focused transition tests.
    #[cfg(test)]
    pub(super) fn project_candidate_for_test(
        &self,
        verified: &VerifiedHeightContext,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.project_exact_validate_candidate(verified, effect, receipt, pending)
    }

    /// Consume the adapter's move-only registered-Prepare proof into the exact
    /// canonical invalid-body report evidence.
    ///
    /// The capability is minted only by the fixed Ready/rejected adapter
    /// preview. Callers cannot substitute the report certificate or child
    /// pending binding, and decoded V1 data never reaches this constructor.
    pub(in crate::sumeragi) fn seal_invalid_body_report(
        capability: RegisteredPrepareInvalidBodyReportCapability,
        validate_origin: Self,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        report_effect: &AdapterEffect,
        report_pending: &PendingRuntimeEffectBinding,
    ) -> Option<InvalidBodyReportReplayEvidenceV1> {
        if !capability.exactly_matches_report(report_effect)
            || !validate_origin.exactly_matches_validate_pending(
                validate_effect,
                receipt,
                validate_pending,
            )
        {
            return None;
        }
        let projected_report_pending = validate_pending
            .project_validate_report_invalid_certified_body_successor(
                validate_effect,
                report_effect,
            )
            .or_else(|| {
                validate_pending
                    .project_validate_report_invalid_certified_body_with_registered_prepare(
                        validate_effect,
                        report_effect,
                        &capability,
                    )
            })?;
        if &projected_report_pending != report_pending {
            return None;
        }
        let authority = exact_invalid_body_report_authority(
            &validate_origin,
            validate_effect,
            receipt,
            report_effect,
        )?;
        let pending_fingerprint =
            DirectSignedPendingBindingV1::from_exact_effect(report_effect, report_pending)?;
        let evidence = InvalidBodyReportReplayEvidenceV1 {
            authority,
            validate_origin,
            report_pending: pending_fingerprint,
        };
        evidence
            .exactly_matches(
                validate_effect,
                validate_pending,
                receipt,
                report_effect,
                report_pending,
            )
            .then_some(evidence)
    }
}

impl InvalidBodyReportReplayEvidenceV1 {
    /// Compare the complete body origin, rejection envelope, report effect,
    /// and causal binding without exposing any retained part.
    pub(in crate::sumeragi) fn exactly_matches(
        &self,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        report_effect: &AdapterEffect,
        report_pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.validate_origin.exactly_matches_validate_pending(
            validate_effect,
            receipt,
            validate_pending,
        ) && self
            .report_pending
            .exactly_matches(report_effect, report_pending)
            && exact_invalid_body_report_authority(
                &self.validate_origin,
                validate_effect,
                receipt,
                report_effect,
            )
            .is_some_and(|expected| expected == self.authority)
    }

    /// Attach the retained invalid-body authority to its exact report shape.
    ///
    /// The private transition permit is borrowed across projection and remains
    /// owned by the registry join. No decoded or caller-supplied report can
    /// invoke this path.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn project_sealed_invalid_body_report_candidate(
        &self,
        _permit: &SealedInvalidBodyReportProjectionPermit,
        verified: &VerifiedHeightContext,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
        report_effect: &AdapterEffect,
        report_pending: &PendingRuntimeEffectBinding,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_matches(
            validate_effect,
            validate_pending,
            receipt,
            report_effect,
            report_pending,
        ) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = replay_context(receipt.round());
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            report_effect,
            report_pending,
        )?;
        candidate_from_authorized_projection(
            active_context,
            projected,
            DurablePayloadReference::None,
            self.authority.clone(),
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)
    }
}

fn remote_proposal_validate_matches_durable_body(
    evidence: &RemoteProposalValidateReplayEvidenceV1,
    receipt: &DurableBodyReceipt,
) -> bool {
    let BodyPipelineOriginV1::Proposal(proposal) = &evidence.family.source.origin else {
        return false;
    };
    let effect = AdapterEffect::ValidateBody {
        tag: EventTag::new(
            evidence.family.source.tag.height,
            evidence.family.source.tag.view,
            crate::sumeragi::v2_core::Generation::new(evidence.family.source.tag.generation),
        ),
        round: proposal.round,
        subject: proposal.subject,
    };
    evidence.exactly_matches_validate(&effect, receipt)
}

fn exact_invalid_body_report_authority(
    validate_origin: &DurableValidateReplayEvidenceV1,
    validate_effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
    report_effect: &AdapterEffect,
) -> Option<LifecycleReplayAuthorityV1> {
    const CANONICAL_REJECTION_CODE: u8 = 0;

    let AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    } = validate_effect
    else {
        return None;
    };
    let AdapterEffect::ReportInvalidCertifiedBody {
        subject: report_subject,
        certificate,
    } = report_effect
    else {
        return None;
    };
    if certificate.phase != wire::GlobalPhase::Prepare
        || certificate.round != *round
        || certificate.proposal_round != *round
        || certificate.subject != *subject
        || *report_subject != *subject
        || tag.height() != certificate.round.height
        || tag.view() != certificate.round.view
        || receipt.context_id() != round.context_id
        || receipt.round() != *round
        || receipt.subject() != *subject
    {
        return None;
    }

    let (validation_origin, manifest) = match validate_origin {
        DurableValidateReplayEvidenceV1::Certified(evidence) => {
            let coordinates = exact_family_coordinates(&evidence.family)?;
            if coordinates.certificate != *certificate {
                return None;
            }
            (evidence.family.source.clone(), coordinates.manifest)
        }
        DurableValidateReplayEvidenceV1::RemoteProposal(evidence) => {
            let BodyPipelineOriginV1::Proposal(proposal) = &evidence.family.source.origin else {
                return None;
            };
            if proposal.round != *round || proposal.subject != *subject {
                return None;
            }
            (evidence.family.source.clone(), proposal.manifest.clone())
        }
    };
    if receipt.manifest_hash() != HashOf::new(&manifest) {
        return None;
    }
    let context = replay_context(certificate.round);
    canonical_replay_authority(
        context,
        LifecycleReplaySourceV1::InvalidCertifiedBody(InvalidBodyReplaySourceV1 {
            validation_origin,
            certificate: certificate.clone(),
            outcome: RejectedBodyOutcomeBindingV1 {
                manifest,
                body_frame_hash: *receipt.frame_hash().as_ref(),
                rejection_code: CANONICAL_REJECTION_CODE,
            },
        }),
        LifecycleStageKind::ReportInvalidBody,
        ReplayPayloadBindingV1::None,
    )
}

fn exact_certified_fetch_coordinates(
    effect: &AdapterEffect,
    response: &wire::CertifiedBodyResponse,
) -> Option<CertifiedBodyPipelineCoordinatesV1> {
    let AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest,
        certificate,
        ..
    } = effect
    else {
        return None;
    };
    let certificate = certificate.as_ref()?;
    if response.manifest.round != *round
        || response.manifest.subject != *subject
        || manifest
            .as_ref()
            .is_some_and(|expected| expected != &response.manifest)
        || certificate.proposal_round != *round
        || certificate.subject != *subject
        || tag.height() != round.height
        || tag.view() < round.view
    {
        return None;
    }
    Some(CertifiedBodyPipelineCoordinatesV1 {
        tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
        certificate: certificate.clone(),
        manifest: response.manifest.clone(),
        fetch_manifest_present: manifest.is_some(),
        certified_sources: match effect {
            AdapterEffect::FetchBody {
                certified_sources, ..
            } => certified_sources.clone(),
            _ => unreachable!("Fetch shape was checked above"),
        },
    })
}

fn exact_certified_body_pipeline_family(
    coordinates: &CertifiedBodyPipelineCoordinatesV1,
    receipt: &DurableBodyReceipt,
) -> Option<CertifiedBodyPipelineReplayFamilyV1> {
    let family = certified_body_pipeline_family(coordinates, receipt)?;
    family.is_exact_all_stages().then_some(family)
}

fn certified_body_pipeline_family(
    coordinates: &CertifiedBodyPipelineCoordinatesV1,
    receipt: &DurableBodyReceipt,
) -> Option<CertifiedBodyPipelineReplayFamilyV1> {
    let certificate = &coordinates.certificate;
    let manifest = &coordinates.manifest;
    if receipt.context_id() != certificate.round.context_id
        || receipt.round() != manifest.round
        || receipt.subject() != manifest.subject
        || receipt.manifest_hash() != HashOf::new(manifest)
    {
        return None;
    }
    let context = LifecycleContext::new(
        digest_from_bytes(certificate.round.context_id.0.as_ref()),
        certificate.round.height,
    );
    let frame = durable_body_frame_reference(context, receipt)?;
    let source = BodyPipelineReplaySourceV1 {
        tag: coordinates.tag,
        origin: BodyPipelineOriginV1::Certified {
            certificate: certificate.clone(),
            manifest: manifest.clone(),
            fetch_manifest_present: coordinates.fetch_manifest_present,
            certified_sources: coordinates.certified_sources.clone(),
        },
    };
    let ReplayPayloadBindingV1::BodyFrame(body_frame) =
        ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame))
    else {
        unreachable!("a durable body frame projects one body-frame binding")
    };
    Some(CertifiedBodyPipelineReplayFamilyV1 { source, body_frame })
}

fn canonical_replay_authority(
    context: LifecycleContext,
    source: LifecycleReplaySourceV1,
    stage_kind: LifecycleStageKind,
    payload: ReplayPayloadBindingV1,
) -> Option<LifecycleReplayAuthorityV1> {
    let shape = source.project(context, stage_kind, &payload).ok()?;
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload,
        source,
    };
    authority
        .validate_record(
            context,
            shape.key,
            shape.work_class,
            LifecycleStage::new(stage_kind, PredecessorScope::Independent),
            match &authority.payload {
                ReplayPayloadBindingV1::None => DurablePayloadReference::None,
                ReplayPayloadBindingV1::BodyFrame(frame) => {
                    DurablePayloadReference::BodyFrame(frame.durable_reference())
                }
                ReplayPayloadBindingV1::CertifiedServePending { .. }
                | ReplayPayloadBindingV1::CertifiedServeCompleted { .. }
                | ReplayPayloadBindingV1::CertifiedServeNegative { .. } => return None,
            },
        )
        .ok()?;
    let canonical = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode()).ok()?;
    (canonical == authority).then_some(canonical)
}

/// Classify the private recovered-Decision body continuation family.
///
/// `None` means neither side belongs to this family and the ordinary body-edge
/// rules apply. `Some(false)` is a hard mismatch: once the payload-free
/// `FetchDecision` or a recovered-Decision body source appears, it cannot be
/// spliced to a generic body family or skip an intermediate stage.
pub(super) fn recovered_decision_body_continuation_is_exact(
    edge: super::schema::DurableContinuationEdge,
    parent: &LifecycleReplayAuthorityV1,
    parent_payload: DurablePayloadReference,
    child: &LifecycleReplayAuthorityV1,
    child_payload: DurablePayloadReference,
) -> Option<bool> {
    let fetch = recovered_decision_fetch_parts(parent);
    let parent_body = recovered_decision_body_parts(parent);
    let child_body = recovered_decision_body_parts(child);
    let family_present = fetch.is_some() || parent_body.is_some() || child_body.is_some();
    if !family_present {
        return None;
    }
    let canonical = |authority: &LifecycleReplayAuthorityV1, payload: DurablePayloadReference| {
        authority.payload.matches(payload)
            && LifecycleReplayAuthorityV1::decode_canonical(&authority.encode())
                .is_ok_and(|decoded| decoded == *authority)
    };
    if !canonical(parent, parent_payload) || !canonical(child, child_payload) {
        return Some(false);
    }
    Some(match edge {
        super::schema::DurableContinuationEdge::FetchToStore => {
            let (fetch_locator, fetch_tag, fetch_certificate) = match fetch {
                Some(parts) => parts,
                None => return Some(false),
            };
            let (body_source, body_frame) = match child_body {
                Some(parts) => parts,
                None => return Some(false),
            };
            parent_payload == DurablePayloadReference::None
                && child_payload
                    == DurablePayloadReference::BodyFrame(body_frame.durable_reference())
                && body_source.locator == fetch_locator
                && body_source.tag == fetch_tag
                && body_source.certificate == &fetch_certificate
        }
        super::schema::DurableContinuationEdge::StoreToValidate => {
            parent_body.is_some()
                && child_body.is_some()
                && parent == child
                && parent_payload == child_payload
                && matches!(parent_payload, DurablePayloadReference::BodyFrame(_))
        }
        super::schema::DurableContinuationEdge::ValidateToApply => {
            let (body_source, body_frame) = match parent_body {
                Some(parts) => parts,
                None => return Some(false),
            };
            let (apply_locator, apply_tag, apply_certificate, apply_frame) =
                match recovered_decision_apply_parts(child) {
                    Some(parts) => parts,
                    None => return Some(false),
                };
            parent_payload == DurablePayloadReference::BodyFrame(body_frame.durable_reference())
                && child_payload
                    == DurablePayloadReference::BodyFrame(apply_frame.durable_reference())
                && body_frame == apply_frame
                && body_source.locator == apply_locator
                && body_source.tag == apply_tag
                && body_source.certificate == apply_certificate
        }
        super::schema::DurableContinuationEdge::ValidateToInvalidBodyReport
        | super::schema::DurableContinuationEdge::ValidateToSignPrepare
        | super::schema::DurableContinuationEdge::ValidateToSignCommit => false,
    })
}

fn recovered_decision_fetch_parts(
    authority: &LifecycleReplayAuthorityV1,
) -> Option<(
    PersistedWalFrameLocatorV1,
    ReplayEventTagV1,
    wire::QuorumCertificate,
)> {
    let (
        ReplayPayloadBindingV1::None,
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role,
            tag,
            action:
                WalReplayActionV1::FetchDecision {
                    certificate,
                    certified_sources,
                },
        }),
    ) = (&authority.payload, &authority.source)
    else {
        return None;
    };
    (authority.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
        && locator.is_exact()
        && role.matches(ReplayWalRoleV1::DECISION)
        && certificate.phase == wire::GlobalPhase::Commit
        && certified_sources_are_bounded_unique(certified_sources)
        && !certified_sources.is_empty())
    .then_some((*locator, *tag, certificate.clone()))
}

struct RecoveredDecisionBodyReplayParts<'authority> {
    locator: PersistedWalFrameLocatorV1,
    tag: ReplayEventTagV1,
    certificate: &'authority wire::QuorumCertificate,
}

fn recovered_decision_body_parts(
    authority: &LifecycleReplayAuthorityV1,
) -> Option<(RecoveredDecisionBodyReplayParts<'_>, BodyFrameBindingV1)> {
    let (
        ReplayPayloadBindingV1::BodyFrame(body_frame),
        LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
            tag,
            origin:
                BodyPipelineOriginV1::RecoveredDecision {
                    locator,
                    certificate,
                    manifest,
                },
        }),
    ) = (&authority.payload, &authority.source)
    else {
        return None;
    };
    (authority.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
        && locator.is_exact()
        && certificate.phase == wire::GlobalPhase::Commit
        && body_frame.matches_origin(
            replay_context(certificate.round),
            certificate.proposal_round,
            certificate.subject,
        )
        && body_frame.manifest == *HashOf::new(manifest).as_ref())
    .then_some((
        RecoveredDecisionBodyReplayParts {
            locator: *locator,
            tag: *tag,
            certificate,
        },
        *body_frame,
    ))
}

fn recovered_decision_apply_parts(
    authority: &LifecycleReplayAuthorityV1,
) -> Option<(
    PersistedWalFrameLocatorV1,
    ReplayEventTagV1,
    &wire::QuorumCertificate,
    BodyFrameBindingV1,
)> {
    let (
        ReplayPayloadBindingV1::BodyFrame(body_frame),
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role,
            tag,
            action: WalReplayActionV1::ApplyDecision(certificate),
        }),
    ) = (&authority.payload, &authority.source)
    else {
        return None;
    };
    (authority.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
        && locator.is_exact()
        && role.matches(ReplayWalRoleV1::DECISION)
        && certificate.phase == wire::GlobalPhase::Commit
        && body_frame.matches_origin(
            replay_context(certificate.round),
            certificate.proposal_round,
            certificate.subject,
        ))
    .then_some((*locator, *tag, certificate, *body_frame))
}

impl CertifiedBodyPipelineReplayFamilyV1 {
    fn is_exact_all_stages(&self) -> bool {
        self.is_exact_for_stage(LifecycleStageKind::FetchBody)
            && self.is_exact_for_stage(LifecycleStageKind::StoreBody)
            && self.is_exact_for_stage(LifecycleStageKind::ValidateBody)
    }

    fn is_exact_for_stage(&self, stage: LifecycleStageKind) -> bool {
        let Some(coordinates) = exact_family_coordinates(self) else {
            return false;
        };
        let context = LifecycleContext::new(
            digest_from_bytes(coordinates.certificate.round.context_id.0.as_ref()),
            coordinates.certificate.round.height,
        );
        let source = LifecycleReplaySourceV1::BodyPipeline(self.source.clone());
        let payload = match stage {
            LifecycleStageKind::FetchBody
            | LifecycleStageKind::StoreBody
            | LifecycleStageKind::ValidateBody => {
                ReplayPayloadBindingV1::BodyFrame(self.body_frame)
            }
            _ => return false,
        };
        canonical_replay_authority(context, source, stage, payload).is_some()
    }
}

fn exact_family_coordinates(
    family: &CertifiedBodyPipelineReplayFamilyV1,
) -> Option<CertifiedBodyPipelineCoordinatesV1> {
    let BodyPipelineOriginV1::Certified {
        certificate,
        manifest,
        fetch_manifest_present,
        certified_sources,
    } = &family.source.origin
    else {
        return None;
    };
    Some(CertifiedBodyPipelineCoordinatesV1 {
        tag: family.source.tag,
        certificate: certificate.clone(),
        manifest: manifest.clone(),
        fetch_manifest_present: *fetch_manifest_present,
        certified_sources: certified_sources.clone(),
    })
}

fn exact_certified_fetch_effect(
    family: &CertifiedBodyPipelineReplayFamilyV1,
) -> Option<AdapterEffect> {
    let coordinates = exact_family_coordinates(family)?;
    Some(AdapterEffect::FetchBody {
        tag: EventTag::new(
            coordinates.tag.height,
            coordinates.tag.view,
            crate::sumeragi::v2_core::Generation::new(coordinates.tag.generation),
        ),
        round: coordinates.certificate.proposal_round,
        subject: coordinates.certificate.subject,
        manifest: coordinates
            .fetch_manifest_present
            .then_some(coordinates.manifest),
        certified_sources: coordinates.certified_sources,
        certificate: Some(coordinates.certificate),
    })
}

fn durable_certified_fetch_projection(
    family: &CertifiedBodyPipelineReplayFamilyV1,
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
    receipt: &DurableBodyReceipt,
) -> Option<DurableCertifiedFetchReplayProjectionV1> {
    if exact_certified_fetch_effect(family).as_ref() != Some(effect)
        || !pending.exactly_binds_adapter_effect(effect)
        || certified_body_pipeline_family(&exact_family_coordinates(family)?, receipt).as_ref()
            != Some(family)
    {
        return None;
    }
    let context = replay_context(receipt.round());
    let payload =
        DurablePayloadReference::BodyFrame(durable_body_frame_reference(context, receipt)?);
    let authority = canonical_replay_authority(
        context,
        LifecycleReplaySourceV1::BodyPipeline(family.source.clone()),
        LifecycleStageKind::FetchBody,
        ReplayPayloadBindingV1::from_payload(payload),
    )?;
    let causal_key = *pending.causal_lifecycle_key();
    let effect_identity = *pending.exact_effect_identity();
    let completion_digest = canonical_durable_certified_fetch_completion_digest(
        causal_key,
        effect_identity,
        &authority,
    );
    Some(DurableCertifiedFetchReplayProjectionV1 {
        payload,
        authority,
        causal_key,
        effect_identity,
        completion_digest,
        expected_manifest_hash: receipt.manifest_hash(),
    })
}

fn canonical_durable_certified_fetch_completion_digest(
    causal_key: Hash,
    effect_identity: Hash,
    authority: &LifecycleReplayAuthorityV1,
) -> LifecycleDigest {
    const DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:durable-certified-fetch:v1";
    let encoded_authority = authority.encode();
    let mut preimage =
        Vec::with_capacity(DOMAIN.len() + 1 + Hash::LENGTH * 2 + 8 + encoded_authority.len());
    preimage.extend_from_slice(DOMAIN);
    preimage.push(0);
    preimage.extend_from_slice(causal_key.as_ref());
    preimage.extend_from_slice(effect_identity.as_ref());
    preimage.extend_from_slice(
        &u64::try_from(encoded_authority.len())
            .expect("bounded replay authority encoding fits u64")
            .to_le_bytes(),
    );
    preimage.extend_from_slice(&encoded_authority);
    digest_from_hash(&Hash::new(preimage))
}

fn certified_body_stage_matches(
    family: &CertifiedBodyPipelineReplayFamilyV1,
    effect: &AdapterEffect,
    receipt: &DurableBodyReceipt,
    stage: LifecycleStageKind,
) -> bool {
    let Some(coordinates) = exact_family_coordinates(family) else {
        return false;
    };
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
            coordinates.tag
                == ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get())
                && *round == coordinates.certificate.round
                && *subject == coordinates.certificate.subject
        }
        _ => false,
    };
    exact_effect
        && certified_body_pipeline_family(&coordinates, receipt)
            .is_some_and(|expected| expected == *family && family.is_exact_for_stage(stage))
}

fn exact_live_wal_replay_projection(
    wal_identity: &LiveWalFrameIdentity,
    effect: &AdapterEffect,
) -> Option<LiveWalReplayProjectionV1> {
    if !wal_identity.is_exact() {
        return None;
    }
    let (tag, round, role, stage, action) = match effect {
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } => (
            *tag,
            proposal.round,
            ReplayWalRoleV1::PROPOSAL_INTENT,
            LifecycleStageKind::SignProposal,
            WalReplayActionV1::SignProposal(proposal.clone()),
        ),
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(vote),
        } => {
            let (role, stage) = match vote.phase {
                wire::GlobalPhase::Prepare => (
                    ReplayWalRoleV1::PREPARE_INTENT,
                    LifecycleStageKind::SignPrepareVote,
                ),
                wire::GlobalPhase::Commit => (
                    ReplayWalRoleV1::LOCK_AND_COMMIT,
                    LifecycleStageKind::SignCommitVote,
                ),
            };
            (
                *tag,
                vote.round,
                role,
                stage,
                WalReplayActionV1::SignVote(vote.clone()),
            )
        }
        AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(vote),
        } => (
            *tag,
            vote.round,
            ReplayWalRoleV1::TIMEOUT_INTENT,
            LifecycleStageKind::SignTimeoutVote,
            WalReplayActionV1::SignTimeoutVote(vote.clone()),
        ),
        AdapterEffect::Apply {
            tag, certificate, ..
        } => (
            *tag,
            certificate.round,
            ReplayWalRoleV1::DECISION,
            LifecycleStageKind::ApplyDecision,
            WalReplayActionV1::ApplyDecision(certificate.clone()),
        ),
        AdapterEffect::EnterView {
            tag,
            certificate,
            protected_lock,
        } => (
            *tag,
            certificate.round,
            ReplayWalRoleV1::INSTALL_TIMEOUT,
            LifecycleStageKind::EnterView,
            WalReplayActionV1::EnterView {
                certificate: certificate.clone(),
                protected_lock: protected_lock.clone(),
            },
        ),
        AdapterEffect::Broadcast(_)
        | AdapterEffect::FetchBody { .. }
        | AdapterEffect::StoreBody { .. }
        | AdapterEffect::ValidateBody { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => return None,
    };
    let context = replay_context(round);
    let replay_tag = ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get());
    let source = WalReplaySourceV1 {
        locator: wal_identity.persisted_locator(),
        role,
        tag: replay_tag,
        action,
    };
    if stage == LifecycleStageKind::ApplyDecision {
        let WalReplayActionV1::ApplyDecision(certificate) = &source.action else {
            unreachable!("Apply stage is constructed from one Decision action")
        };
        if !source.locator.is_exact()
            || !source.role.matches(ReplayWalRoleV1::DECISION)
            || !qc_shape(context, certificate)
            || certificate.phase != wire::GlobalPhase::Commit
            || !source.tag.matches_round(context, certificate.round)
        {
            return None;
        }
    }
    Some(LiveWalReplayProjectionV1 {
        context,
        stage,
        source,
    })
}

fn canonical_wal_source(source: &WalReplaySourceV1) -> bool {
    let encoded = source.encode();
    if encoded.is_empty() || encoded.len() > MAX_REPLAY_AUTHORITY_BYTES {
        return false;
    }
    let mut cursor = encoded.as_slice();
    WalReplaySourceV1::decode_all(&mut cursor).is_ok_and(|canonical| {
        cursor.is_empty() && canonical == *source && canonical.encode() == encoded
    })
}

fn exact_recovered_wal_control_authority(
    locator: RecoveredWalFrameIdentity,
    effect: &AdapterEffect,
) -> Option<LifecycleReplayAuthorityV1> {
    if !locator.is_exact() {
        return None;
    }
    let (
        tag,
        round,
        role,
        work_class,
        phase,
        stage_kind,
        action,
        proposal_round,
        subject,
        execution,
    ) = match effect {
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } => (
            *tag,
            proposal.round,
            ReplayWalRoleV1::PROPOSAL_INTENT,
            LifecycleWorkClass::SignProposal,
            LifecyclePhase::Proposal,
            LifecycleStageKind::SignProposal,
            WalReplayActionV1::SignProposal(proposal.clone()),
            Some(proposal.round),
            Some(block_subject(proposal.subject)),
            None,
        ),
        AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(vote),
        } => (
            *tag,
            vote.round,
            ReplayWalRoleV1::TIMEOUT_INTENT,
            LifecycleWorkClass::SignTimeout,
            LifecyclePhase::Timeout,
            LifecycleStageKind::SignTimeoutVote,
            WalReplayActionV1::SignTimeoutVote(vote.clone()),
            vote.highest_prepare_qc
                .as_ref()
                .map(|certificate| certificate.proposal_round),
            vote.highest_prepare_qc
                .as_ref()
                .map(|certificate| block_subject(certificate.subject)),
            vote.highest_prepare_qc
                .as_ref()
                .map(|certificate| execution_commitment(certificate.execution_commitment)),
        ),
        AdapterEffect::Sign {
            request: SignRequest::Vote(_),
            ..
        }
        | AdapterEffect::Broadcast(_)
        | AdapterEffect::FetchBody { .. }
        | AdapterEffect::StoreBody { .. }
        | AdapterEffect::ValidateBody { .. }
        | AdapterEffect::Apply { .. }
        | AdapterEffect::EnterView { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => return None,
    };
    if tag.height() != round.height || tag.view() != round.view {
        return None;
    }
    let context =
        LifecycleContext::new(digest_from_bytes(round.context_id.0.as_ref()), round.height);
    let source = LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
        locator: locator.persisted_locator(),
        role,
        tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
        action,
    });
    let payload = ReplayPayloadBindingV1::None;
    let shape = source.project(context, stage_kind, &payload).ok()?;
    if shape.work_class != work_class
        || shape.stage_kind != stage_kind
        || shape.key != lifecycle_key(context, round, proposal_round, subject, phase, execution)
    {
        return None;
    }
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload,
        source,
    };
    authority
        .validate_record(
            context,
            shape.key,
            work_class,
            LifecycleStage::new(stage_kind, PredecessorScope::Independent),
            DurablePayloadReference::None,
        )
        .ok()
        .map(|_| authority)
}

fn exact_recovered_wal_decision_fetch_authority(
    verified: &VerifiedHeightContext,
    locator: RecoveredWalFrameIdentity,
    effect: &AdapterEffect,
) -> Option<LifecycleReplayAuthorityV1> {
    if !locator.is_exact() {
        return None;
    }
    let AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: None,
        certified_sources,
        certificate: Some(certificate),
    } = effect
    else {
        return None;
    };
    let expected_sources = verified
        .context()
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    if certificate.phase != wire::GlobalPhase::Commit
        || certificate.proposal_round != *round
        || certificate.subject != *subject
        || certified_sources != &expected_sources
        || tag.height() != certificate.round.height
        || tag.view() < certificate.round.view
        || verified.verify_quorum_certificate(certificate).is_err()
    {
        return None;
    }
    let context = super::projection::lifecycle_context(verified.context());
    let source = LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
        locator: locator.persisted_locator(),
        role: ReplayWalRoleV1::DECISION,
        tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
        action: WalReplayActionV1::FetchDecision {
            certificate: certificate.clone(),
            certified_sources: certified_sources.clone(),
        },
    });
    let payload = ReplayPayloadBindingV1::None;
    let shape = source
        .project(context, LifecycleStageKind::FetchBody, &payload)
        .ok()?;
    if shape.work_class != LifecycleWorkClass::Fetch
        || shape.stage_kind != LifecycleStageKind::FetchBody
        || shape.key
            != lifecycle_key(
                context,
                certificate.round,
                Some(certificate.proposal_round),
                Some(block_subject(certificate.subject)),
                LifecyclePhase::Fetch,
                Some(execution_commitment(certificate.execution_commitment)),
            )
    {
        return None;
    }
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload,
        source,
    };
    authority
        .validate_record(
            context,
            shape.key,
            LifecycleWorkClass::Fetch,
            LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
            DurablePayloadReference::None,
        )
        .ok()?;
    Some(authority)
}

fn exact_recovered_wal_vote_authority(
    locator: RecoveredWalFrameIdentity,
    tag: EventTag,
    vote: &wire::Vote,
) -> Option<LifecycleReplayAuthorityV1> {
    let tag_matches_vote = tag.height() == vote.round.height
        && match vote.phase {
            wire::GlobalPhase::Prepare => tag.view() == vote.round.view,
            wire::GlobalPhase::Commit => tag.view() >= vote.round.view,
        };
    if !locator.is_exact() || !tag_matches_vote {
        return None;
    }
    let (role, phase, stage_kind) = match vote.phase {
        wire::GlobalPhase::Prepare => (
            ReplayWalRoleV1::PREPARE_INTENT,
            LifecyclePhase::Prepare,
            LifecycleStageKind::SignPrepareVote,
        ),
        wire::GlobalPhase::Commit => (
            ReplayWalRoleV1::LOCK_AND_COMMIT,
            LifecyclePhase::Commit,
            LifecycleStageKind::SignCommitVote,
        ),
    };
    let context = LifecycleContext::new(
        digest_from_bytes(vote.round.context_id.0.as_ref()),
        vote.round.height,
    );
    let payload = ReplayPayloadBindingV1::None;
    let source = LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
        locator: locator.persisted_locator(),
        role,
        tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
        action: WalReplayActionV1::SignVote(vote.clone()),
    });
    let shape = source.project(context, stage_kind, &payload).ok()?;
    if shape.work_class != LifecycleWorkClass::SignVote
        || shape.stage_kind != stage_kind
        || shape.key
            != lifecycle_key(
                context,
                vote.round,
                Some(vote.proposal_round),
                Some(block_subject(vote.subject)),
                phase,
                Some(execution_commitment(vote.execution_commitment)),
            )
    {
        return None;
    }
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload,
        source,
    };
    authority
        .validate_record(
            context,
            shape.key,
            LifecycleWorkClass::SignVote,
            LifecycleStage::new(stage_kind, PredecessorScope::Independent),
            DurablePayloadReference::None,
        )
        .ok()?;
    Some(authority)
}

impl WalReplaySourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        if !self.locator.is_exact() {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        let shape = match &self.action {
            WalReplayActionV1::SignProposal(proposal) => {
                if !self.role.matches(ReplayWalRoleV1::PROPOSAL_INTENT)
                    || !proposal_shape(context, proposal, false)
                    || !self.tag.matches_round(context, proposal.round)
                    || self.tag.view != proposal.round.view
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        proposal.round,
                        Some(proposal.round),
                        Some(block_subject(proposal.subject)),
                        LifecyclePhase::Proposal,
                        None,
                    ),
                    LifecycleWorkClass::SignProposal,
                    LifecycleStageKind::SignProposal,
                )
            }
            WalReplayActionV1::SignVote(vote) => {
                let (role, phase, stage_kind) = match vote.phase {
                    wire::GlobalPhase::Prepare => (
                        ReplayWalRoleV1::PREPARE_INTENT,
                        LifecyclePhase::Prepare,
                        LifecycleStageKind::SignPrepareVote,
                    ),
                    wire::GlobalPhase::Commit => (
                        ReplayWalRoleV1::LOCK_AND_COMMIT,
                        LifecyclePhase::Commit,
                        LifecycleStageKind::SignCommitVote,
                    ),
                };
                let tag_matches_vote = self.tag.matches_round(context, vote.round)
                    && match vote.phase {
                        wire::GlobalPhase::Prepare => self.tag.view == vote.round.view,
                        wire::GlobalPhase::Commit => true,
                    };
                if !self.role.matches(role)
                    || !vote_shape(context, vote, false)
                    || !tag_matches_vote
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        vote.round,
                        Some(vote.proposal_round),
                        Some(block_subject(vote.subject)),
                        phase,
                        Some(execution_commitment(vote.execution_commitment)),
                    ),
                    LifecycleWorkClass::SignVote,
                    stage_kind,
                )
            }
            WalReplayActionV1::SignTimeoutVote(vote) => {
                if !self.role.matches(ReplayWalRoleV1::TIMEOUT_INTENT)
                    || !timeout_vote_shape(context, vote, false)
                    || !self.tag.matches_round(context, vote.round)
                    || self.tag.view != vote.round.view
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                let highest = vote.highest_prepare_qc.as_ref();
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        vote.round,
                        highest.map(|qc| qc.proposal_round),
                        highest.map(|qc| block_subject(qc.subject)),
                        LifecyclePhase::Timeout,
                        highest.map(|qc| execution_commitment(qc.execution_commitment)),
                    ),
                    LifecycleWorkClass::SignTimeout,
                    LifecycleStageKind::SignTimeoutVote,
                )
            }
            WalReplayActionV1::ApplyDecision(certificate) => {
                if !self.role.matches(ReplayWalRoleV1::DECISION)
                    || !qc_shape(context, certificate)
                    || certificate.phase != wire::GlobalPhase::Commit
                    || !self.tag.matches_round(context, certificate.round)
                    || !payload.matches_body_origin(
                        context,
                        certificate.proposal_round,
                        certificate.subject,
                    )
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        certificate.round,
                        Some(certificate.proposal_round),
                        Some(block_subject(certificate.subject)),
                        LifecyclePhase::Apply,
                        Some(execution_commitment(certificate.execution_commitment)),
                    ),
                    LifecycleWorkClass::Apply,
                    LifecycleStageKind::ApplyDecision,
                )
            }
            WalReplayActionV1::FetchDecision {
                certificate,
                certified_sources,
            } => {
                if !self.role.matches(ReplayWalRoleV1::DECISION)
                    || !qc_shape(context, certificate)
                    || certificate.phase != wire::GlobalPhase::Commit
                    || !self.tag.matches_round(context, certificate.round)
                    || certified_sources.is_empty()
                    || certified_sources.len() > wire::MAX_VALIDATORS_PER_HEIGHT
                    || certified_sources
                        .iter()
                        .enumerate()
                        .any(|(index, source)| certified_sources[..index].contains(source))
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        certificate.round,
                        Some(certificate.proposal_round),
                        Some(block_subject(certificate.subject)),
                        LifecyclePhase::Fetch,
                        Some(execution_commitment(certificate.execution_commitment)),
                    ),
                    LifecycleWorkClass::Fetch,
                    LifecycleStageKind::FetchBody,
                )
            }
            WalReplayActionV1::EnterView {
                certificate,
                protected_lock,
            } => {
                if !self.role.matches(ReplayWalRoleV1::INSTALL_TIMEOUT)
                    || !timeout_certificate_shape(context, certificate)
                    || !enter_view_shape(context, self.tag, certificate, protected_lock.as_ref())
                    || !payload.is_none()
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                let execution_round = wire::ConsensusRound {
                    context_id: certificate.round.context_id,
                    height: certificate.round.height,
                    view: self.tag.view,
                };
                ReplayShape::new(
                    lifecycle_key(
                        context,
                        execution_round,
                        protected_lock.as_ref().map(|lock| lock.proposal_round),
                        protected_lock
                            .as_ref()
                            .map(|lock| block_subject(lock.subject)),
                        LifecyclePhase::EnterView,
                        protected_lock
                            .as_ref()
                            .map(|lock| execution_commitment(lock.execution_commitment)),
                    ),
                    LifecycleWorkClass::EnterView,
                    LifecycleStageKind::EnterView,
                )
            }
        };
        (shape.stage_kind == requested_stage)
            .then_some(shape)
            .ok_or(ReplayAuthorityValidationError::RecordMismatch)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct BodyPipelineReplaySourceV1 {
    tag: ReplayEventTagV1,
    origin: BodyPipelineOriginV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum BodyPipelineOriginV1 {
    #[codec(index = 0)]
    Proposal(wire::Proposal),
    #[codec(index = 1)]
    Certified {
        certificate: wire::QuorumCertificate,
        manifest: wire::PayloadManifest,
        fetch_manifest_present: bool,
        certified_sources: Vec<PeerId>,
    },
    #[codec(index = 2)]
    LocalBody(wire::PayloadManifest),
    #[codec(index = 3)]
    RecoveredDecision {
        locator: PersistedWalFrameLocatorV1,
        certificate: wire::QuorumCertificate,
        manifest: wire::PayloadManifest,
    },
}

impl BodyPipelineReplaySourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        let (round, proposal_round, subject, commitment, manifest, local_body, recovered_decision) =
            match &self.origin {
                BodyPipelineOriginV1::Proposal(proposal) => {
                    if !proposal_shape(context, proposal, true) {
                        return Err(ReplayAuthorityValidationError::InvalidSource);
                    }
                    (
                        proposal.round,
                        proposal.round,
                        proposal.subject,
                        None,
                        Some(&proposal.manifest),
                        false,
                        false,
                    )
                }
                BodyPipelineOriginV1::Certified {
                    certificate,
                    manifest,
                    fetch_manifest_present: _,
                    certified_sources,
                } => {
                    if !qc_shape(context, certificate)
                        || !manifest_matches_origin(
                            context,
                            manifest,
                            certificate.proposal_round,
                            certificate.subject,
                        )
                        || !certified_sources_are_bounded_unique(certified_sources)
                    {
                        return Err(ReplayAuthorityValidationError::InvalidSource);
                    }
                    (
                        certificate.round,
                        certificate.proposal_round,
                        certificate.subject,
                        Some(execution_commitment(certificate.execution_commitment)),
                        Some(manifest),
                        false,
                        false,
                    )
                }
                BodyPipelineOriginV1::LocalBody(manifest) => {
                    if !round_matches_context(context, manifest.round) {
                        return Err(ReplayAuthorityValidationError::InvalidSource);
                    }
                    (
                        manifest.round,
                        manifest.round,
                        manifest.subject,
                        None,
                        Some(manifest),
                        true,
                        false,
                    )
                }
                BodyPipelineOriginV1::RecoveredDecision {
                    locator,
                    certificate,
                    manifest,
                } => {
                    if !locator.is_exact()
                        || !qc_shape(context, certificate)
                        || certificate.phase != wire::GlobalPhase::Commit
                        || !manifest_matches_origin(
                            context,
                            manifest,
                            certificate.proposal_round,
                            certificate.subject,
                        )
                    {
                        return Err(ReplayAuthorityValidationError::InvalidSource);
                    }
                    (
                        certificate.round,
                        certificate.proposal_round,
                        certificate.subject,
                        Some(execution_commitment(certificate.execution_commitment)),
                        Some(manifest),
                        false,
                        true,
                    )
                }
            };
        if !self.tag.matches_round(context, round) {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        let (phase, work_class) = match requested_stage {
            LifecycleStageKind::FetchBody if !recovered_decision => {
                (LifecyclePhase::Fetch, LifecycleWorkClass::Fetch)
            }
            LifecycleStageKind::StoreBody => (LifecyclePhase::Store, LifecycleWorkClass::Store),
            LifecycleStageKind::ValidateBody => {
                (LifecyclePhase::Validate, LifecycleWorkClass::Validate)
            }
            _ => return Err(ReplayAuthorityValidationError::RecordMismatch),
        };
        if local_body && requested_stage == LifecycleStageKind::FetchBody {
            return Err(ReplayAuthorityValidationError::RecordMismatch);
        }
        let key = lifecycle_key(
            context,
            round,
            Some(proposal_round),
            Some(block_subject(subject)),
            phase,
            commitment,
        );
        match requested_stage {
            LifecycleStageKind::FetchBody
                if payload.is_none()
                    || (!local_body
                        && manifest.is_some_and(|manifest| {
                            payload.matches_exact_body(context, proposal_round, subject, manifest)
                        })) => {}
            LifecycleStageKind::StoreBody | LifecycleStageKind::ValidateBody
                if manifest.is_some_and(|manifest| {
                    payload.matches_exact_body(context, proposal_round, subject, manifest)
                }) => {}
            _ => return Err(ReplayAuthorityValidationError::PayloadMismatch),
        }
        Ok(ReplayShape::new(key, work_class, requested_stage))
    }
}

fn certified_sources_are_bounded_unique(certified_sources: &[PeerId]) -> bool {
    certified_sources.len() <= wire::MAX_VALIDATORS_PER_HEIGHT
        && certified_sources
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == certified_sources.len()
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct InvalidBodyReplaySourceV1 {
    validation_origin: BodyPipelineReplaySourceV1,
    certificate: wire::QuorumCertificate,
    outcome: RejectedBodyOutcomeBindingV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct RejectedBodyOutcomeBindingV1 {
    manifest: wire::PayloadManifest,
    body_frame_hash: [u8; 32],
    rejection_code: u8,
}

impl InvalidBodyReplaySourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        let origin_payload = ReplayPayloadBindingV1::BodyFrame(BodyFrameBindingV1 {
            context: *context.id().as_bytes(),
            round_height: self.outcome.manifest.round.height,
            round_view: self.outcome.manifest.round.view,
            subject: *block_subject(self.outcome.manifest.subject).as_bytes(),
            manifest: *HashOf::new(&self.outcome.manifest).as_ref(),
            frame: self.outcome.body_frame_hash,
        });
        let origin_shape = self.validation_origin.project(
            context,
            LifecycleStageKind::ValidateBody,
            &origin_payload,
        )?;
        if requested_stage != LifecycleStageKind::ReportInvalidBody
            || !payload.is_none()
            || !qc_shape(context, &self.certificate)
            || self.certificate.phase != wire::GlobalPhase::Prepare
            || self.certificate.round != self.certificate.proposal_round
            || self.outcome.rejection_code != 0
            || !manifest_matches_origin(
                context,
                &self.outcome.manifest,
                self.certificate.proposal_round,
                self.certificate.subject,
            )
        {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        match &self.validation_origin.origin {
            BodyPipelineOriginV1::Proposal(proposal)
                if proposal.round == self.certificate.proposal_round
                    && proposal.subject == self.certificate.subject
                    && proposal.manifest == self.outcome.manifest => {}
            BodyPipelineOriginV1::Certified {
                certificate,
                manifest,
                ..
            } if certificate == &self.certificate && manifest == &self.outcome.manifest => {}
            BodyPipelineOriginV1::Proposal(_)
            | BodyPipelineOriginV1::Certified { .. }
            | BodyPipelineOriginV1::LocalBody(_)
            | BodyPipelineOriginV1::RecoveredDecision { .. } => {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
        }
        if origin_shape.work_class != LifecycleWorkClass::Validate
            || origin_shape.stage_kind != LifecycleStageKind::ValidateBody
            || origin_shape.key.context() != context.id()
            || origin_shape.key.round()
                != LifecycleRound::new(
                    self.certificate.proposal_round.height,
                    self.certificate.proposal_round.view,
                )
            || origin_shape.key.proposal_round()
                != Some(LifecycleRound::new(
                    self.certificate.proposal_round.height,
                    self.certificate.proposal_round.view,
                ))
            || origin_shape.key.subject() != Some(block_subject(self.certificate.subject))
        {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        Ok(ReplayShape::new(
            lifecycle_key(
                context,
                self.certificate.round,
                Some(self.certificate.proposal_round),
                Some(block_subject(self.certificate.subject)),
                LifecyclePhase::DiagnosticInvalidBody,
                Some(execution_commitment(self.certificate.execution_commitment)),
            ),
            LifecycleWorkClass::InvalidBodyReport,
            LifecycleStageKind::ReportInvalidBody,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct CertifiedServeStorageSourceV1 {
    request: wire::CertifiedBodyRequest,
    payload_hash: [u8; 32],
    local_retainer: wire::ValidatorIndex,
}

impl CertifiedServeStorageSourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        let certificate = &self.request.certificate;
        let local_retainer = usize::try_from(self.local_retainer)
            .map_err(|_| ReplayAuthorityValidationError::InvalidSource)?;
        if local_retainer >= wire::MAX_VALIDATORS_PER_HEIGHT
            || !signature_present(&self.request.signature)
            || !round_matches_context(context, self.request.round)
            || !qc_shape(context, certificate)
            || certificate.proposal_round != self.request.round
            || certificate.subject != self.request.subject
            || certificate
                .signers
                .binary_search(&self.local_retainer)
                .is_err()
        {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        let request_hash = HashOf::new(&self.request);
        let request_digest = digest_from_bytes(request_hash.as_ref());
        let certificate_digest = digest_from_bytes(HashOf::new(certificate).as_ref());
        let phase = match requested_stage {
            LifecycleStageKind::CertifiedServe => LifecyclePhase::Serve,
            LifecycleStageKind::ProducerTurn => LifecyclePhase::ProducerTurn,
            _ => return Err(ReplayAuthorityValidationError::RecordMismatch),
        };
        let key = lifecycle_key(
            context,
            certificate.round,
            Some(self.request.round),
            Some(certified_serve_key_subject(
                self.request.subject,
                request_hash,
            )),
            phase,
            Some(execution_commitment(certificate.execution_commitment)),
        );
        let work_class = match requested_stage {
            LifecycleStageKind::CertifiedServe => {
                if !payload.matches_certified_serve(request_digest, certificate_digest) {
                    return Err(ReplayAuthorityValidationError::PayloadMismatch);
                }
                LifecycleWorkClass::CertifiedServe
            }
            LifecycleStageKind::ProducerTurn => {
                if !payload.is_none() {
                    return Err(ReplayAuthorityValidationError::PayloadMismatch);
                }
                LifecycleWorkClass::ProducerTurn
            }
            _ => unreachable!("stage was checked above"),
        };
        Ok(ReplayShape::new(key, work_class, requested_stage))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[allow(variant_size_differences)]
enum ReplayPayloadBindingV1 {
    #[codec(index = 0)]
    None,
    #[codec(index = 1)]
    BodyFrame(BodyFrameBindingV1),
    #[codec(index = 2)]
    CertifiedServePending {
        request: [u8; 32],
        certificate: [u8; 32],
    },
    #[codec(index = 3)]
    CertifiedServeCompleted {
        request: [u8; 32],
        certificate: [u8; 32],
        response: [u8; 32],
    },
    #[codec(index = 4)]
    CertifiedServeNegative {
        request: [u8; 32],
        certificate: [u8; 32],
        outcome_kind: u8,
        outcome_code: Option<u16>,
    },
}

impl ReplayPayloadBindingV1 {
    fn from_payload(payload: DurablePayloadReference) -> Self {
        match payload {
            DurablePayloadReference::None => Self::None,
            DurablePayloadReference::BodyFrame(frame) => Self::BodyFrame(BodyFrameBindingV1 {
                context: *frame.context.as_bytes(),
                round_height: frame.round.height(),
                round_view: frame.round.view(),
                subject: *frame.subject.as_bytes(),
                manifest: *frame.manifest.as_bytes(),
                frame: *frame.frame.as_bytes(),
            }),
            DurablePayloadReference::CertifiedServePending {
                request,
                certificate,
            } => Self::CertifiedServePending {
                request: *request.as_bytes(),
                certificate: *certificate.as_bytes(),
            },
            DurablePayloadReference::CertifiedServeCompleted {
                request,
                certificate,
                response,
            } => Self::CertifiedServeCompleted {
                request: *request.as_bytes(),
                certificate: *certificate.as_bytes(),
                response: *response.as_bytes(),
            },
            DurablePayloadReference::CertifiedServeNegative {
                request,
                certificate,
                outcome,
            } => {
                let (outcome_kind, outcome_code) = match outcome {
                    DurableServeNegativeOutcome::Cancelled => (0, None),
                    DurableServeNegativeOutcome::Rejected(code) => (1, Some(code)),
                    DurableServeNegativeOutcome::Failed(code) => (2, Some(code)),
                };
                Self::CertifiedServeNegative {
                    request: *request.as_bytes(),
                    certificate: *certificate.as_bytes(),
                    outcome_kind,
                    outcome_code,
                }
            }
        }
    }

    fn matches(&self, payload: DurablePayloadReference) -> bool {
        *self == Self::from_payload(payload)
    }

    fn durable_payload(&self) -> Option<DurablePayloadReference> {
        Some(match self {
            Self::None => DurablePayloadReference::None,
            Self::BodyFrame(frame) => DurablePayloadReference::BodyFrame(frame.durable_reference()),
            Self::CertifiedServePending {
                request,
                certificate,
            } => DurablePayloadReference::CertifiedServePending {
                request: LifecycleDigest::new(*request),
                certificate: LifecycleDigest::new(*certificate),
            },
            Self::CertifiedServeCompleted {
                request,
                certificate,
                response,
            } => DurablePayloadReference::CertifiedServeCompleted {
                request: LifecycleDigest::new(*request),
                certificate: LifecycleDigest::new(*certificate),
                response: LifecycleDigest::new(*response),
            },
            Self::CertifiedServeNegative {
                request,
                certificate,
                outcome_kind,
                outcome_code,
            } => DurablePayloadReference::CertifiedServeNegative {
                request: LifecycleDigest::new(*request),
                certificate: LifecycleDigest::new(*certificate),
                outcome: match (*outcome_kind, *outcome_code) {
                    (0, None) => DurableServeNegativeOutcome::Cancelled,
                    (1, Some(code)) => DurableServeNegativeOutcome::Rejected(code),
                    (2, Some(code)) => DurableServeNegativeOutcome::Failed(code),
                    _ => return None,
                },
            },
        })
    }

    const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    fn matches_exact_body(
        &self,
        context: LifecycleContext,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        manifest: &wire::PayloadManifest,
    ) -> bool {
        let Self::BodyFrame(frame) = self else {
            return false;
        };
        frame.matches_origin(context, proposal_round, subject)
            && frame.manifest == *digest_from_bytes(HashOf::new(manifest).as_ref()).as_bytes()
    }

    fn matches_body_origin(
        &self,
        context: LifecycleContext,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        match self {
            Self::BodyFrame(frame) => frame.matches_origin(context, proposal_round, subject),
            Self::None
            | Self::CertifiedServePending { .. }
            | Self::CertifiedServeCompleted { .. }
            | Self::CertifiedServeNegative { .. } => false,
        }
    }

    fn matches_certified_serve(
        &self,
        expected_request: LifecycleDigest,
        expected_certificate: LifecycleDigest,
    ) -> bool {
        let (request, certificate) = match self {
            Self::CertifiedServePending {
                request,
                certificate,
            }
            | Self::CertifiedServeCompleted {
                request,
                certificate,
                ..
            }
            | Self::CertifiedServeNegative {
                request,
                certificate,
                ..
            } => (request, certificate),
            Self::None | Self::BodyFrame(_) => return false,
        };
        request == expected_request.as_bytes() && certificate == expected_certificate.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct BodyFrameBindingV1 {
    context: [u8; 32],
    round_height: u64,
    round_view: u64,
    subject: [u8; 32],
    manifest: [u8; 32],
    frame: [u8; 32],
}

impl BodyFrameBindingV1 {
    const fn durable_reference(self) -> DurableBodyFrameReference {
        DurableBodyFrameReference::new(
            LifecycleDigest::new(self.context),
            LifecycleRound::new(self.round_height, self.round_view),
            LifecycleDigest::new(self.subject),
            LifecycleDigest::new(self.manifest),
            LifecycleDigest::new(self.frame),
        )
    }

    fn matches_origin(
        self,
        context: LifecycleContext,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        self.context == *context.id().as_bytes()
            && self.round_height == proposal_round.height
            && self.round_view == proposal_round.view
            && self.subject == *block_subject(subject).as_bytes()
    }
}

#[derive(Clone, Copy)]
struct ReplayShape {
    key: LifecycleKey,
    work_class: LifecycleWorkClass,
    stage_kind: LifecycleStageKind,
}

impl ReplayShape {
    const fn new(
        key: LifecycleKey,
        work_class: LifecycleWorkClass,
        stage_kind: LifecycleStageKind,
    ) -> Self {
        Self {
            key,
            work_class,
            stage_kind,
        }
    }
}

fn project_broadcast(
    context: LifecycleContext,
    message: &wire::ConsensusMessageV2,
    requested_stage: LifecycleStageKind,
    payload: &ReplayPayloadBindingV1,
) -> Result<ReplayShape, ReplayAuthorityValidationError> {
    if message.validate_version().is_err() || !payload.is_none() {
        return Err(ReplayAuthorityValidationError::InvalidSource);
    }
    let shape = match &message.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => {
            if !proposal_shape(context, proposal, true) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            ReplayShape::new(
                lifecycle_key(
                    context,
                    proposal.round,
                    Some(proposal.round),
                    Some(block_subject(proposal.subject)),
                    LifecyclePhase::BroadcastProposal,
                    None,
                ),
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastProposal,
            )
        }
        wire::ConsensusMessageV2Payload::Vote(vote) => {
            if !vote_shape(context, vote, true) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            let (phase, stage_kind) = match vote.phase {
                wire::GlobalPhase::Prepare => (
                    LifecyclePhase::BroadcastPrepareVote,
                    LifecycleStageKind::BroadcastPrepareVote,
                ),
                wire::GlobalPhase::Commit => (
                    LifecyclePhase::BroadcastCommitVote,
                    LifecycleStageKind::BroadcastCommitVote,
                ),
            };
            ReplayShape::new(
                lifecycle_key(
                    context,
                    vote.round,
                    Some(vote.proposal_round),
                    Some(block_subject(vote.subject)),
                    phase,
                    Some(execution_commitment(vote.execution_commitment)),
                ),
                LifecycleWorkClass::Broadcast,
                stage_kind,
            )
        }
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
            if !qc_shape(context, certificate) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            let (phase, stage_kind) = match certificate.phase {
                wire::GlobalPhase::Prepare => (
                    LifecyclePhase::BroadcastPrepareQc,
                    LifecycleStageKind::BroadcastPrepareQc,
                ),
                wire::GlobalPhase::Commit => (
                    LifecyclePhase::BroadcastCommitQc,
                    LifecycleStageKind::BroadcastCommitQc,
                ),
            };
            ReplayShape::new(
                lifecycle_key(
                    context,
                    certificate.round,
                    Some(certificate.proposal_round),
                    Some(block_subject(certificate.subject)),
                    phase,
                    Some(execution_commitment(certificate.execution_commitment)),
                ),
                LifecycleWorkClass::Broadcast,
                stage_kind,
            )
        }
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
            if !timeout_vote_shape(context, vote, true) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            let highest = vote.highest_prepare_qc.as_ref();
            ReplayShape::new(
                lifecycle_key(
                    context,
                    vote.round,
                    highest.map(|qc| qc.proposal_round),
                    highest.map(|qc| block_subject(qc.subject)),
                    LifecyclePhase::BroadcastTimeoutVote,
                    highest.map(|qc| execution_commitment(qc.execution_commitment)),
                ),
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastTimeoutVote,
            )
        }
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
            if !timeout_certificate_shape(context, certificate) {
                return Err(ReplayAuthorityValidationError::InvalidSource);
            }
            let highest = certificate.highest_prepare_qc();
            ReplayShape::new(
                lifecycle_key(
                    context,
                    certificate.round,
                    highest.map(|qc| qc.proposal_round),
                    highest.map(|qc| block_subject(qc.subject)),
                    LifecyclePhase::BroadcastTc,
                    highest.map(|qc| execution_commitment(qc.execution_commitment)),
                ),
                LifecycleWorkClass::Broadcast,
                LifecycleStageKind::BroadcastTc,
            )
        }
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
    };
    (shape.stage_kind == requested_stage)
        .then_some(shape)
        .ok_or(ReplayAuthorityValidationError::RecordMismatch)
}

fn project_equivocation(
    context: LifecycleContext,
    evidence: &wire::SumeragiV2Equivocation,
    requested_stage: LifecycleStageKind,
    payload: &ReplayPayloadBindingV1,
) -> Result<ReplayShape, ReplayAuthorityValidationError> {
    if !payload.is_none()
        || crate::sumeragi::evidence::canonicalize_v2_conflict(evidence) != *evidence
    {
        return Err(ReplayAuthorityValidationError::InvalidSource);
    }
    let (round, phase, stage_kind, valid) = match evidence {
        wire::SumeragiV2Equivocation::Proposal { first, second } => (
            first.round,
            LifecyclePhase::DiagnosticProposalEquivocation,
            LifecycleStageKind::ReportProposalEquivocation,
            proposal_shape(context, first, true)
                && proposal_shape(context, second, true)
                && first.round == second.round
                && first.proposer == second.proposer
                && first.signature_preimage() != second.signature_preimage(),
        ),
        wire::SumeragiV2Equivocation::PhaseVote { first, second } => (
            first.round,
            LifecyclePhase::DiagnosticVoteEquivocation,
            LifecycleStageKind::ReportVoteEquivocation,
            vote_shape(context, first, true)
                && vote_shape(context, second, true)
                && first.round == second.round
                && first.phase == second.phase
                && first.signer == second.signer
                && first.signature_preimage() != second.signature_preimage(),
        ),
        wire::SumeragiV2Equivocation::TimeoutVote { first, second } => (
            first.round,
            LifecyclePhase::DiagnosticTimeoutEquivocation,
            LifecycleStageKind::ReportTimeoutEquivocation,
            timeout_vote_shape(context, first, true)
                && timeout_vote_shape(context, second, true)
                && first.round == second.round
                && first.signer == second.signer
                && first.signature_preimage() != second.signature_preimage(),
        ),
    };
    if !valid || requested_stage != stage_kind {
        return Err(ReplayAuthorityValidationError::InvalidSource);
    }
    Ok(ReplayShape::new(
        lifecycle_key(
            context,
            round,
            None,
            Some(equivocation_subject(evidence)),
            phase,
            None,
        ),
        LifecycleWorkClass::EquivocationReport,
        stage_kind,
    ))
}

fn proposal_shape(context: LifecycleContext, proposal: &wire::Proposal, signed: bool) -> bool {
    round_matches_context(context, proposal.round)
        && proposal.manifest.round == proposal.round
        && proposal.manifest.subject == proposal.subject
        && signature_presence_matches(&proposal.signature, signed)
}

fn vote_shape(context: LifecycleContext, vote: &wire::Vote, signed: bool) -> bool {
    round_matches_context(context, vote.round)
        && round_matches_context(context, vote.proposal_round)
        && vote.proposal_round == vote.round
        && vote.execution_commitment.validate().is_ok()
        && signature_presence_matches(&vote.signature, signed)
}

fn qc_shape(context: LifecycleContext, certificate: &wire::QuorumCertificate) -> bool {
    round_matches_context(context, certificate.round)
        && round_matches_context(context, certificate.proposal_round)
        && certificate.proposal_round == certificate.round
        && certificate.execution_commitment.validate().is_ok()
        && !certificate.signers.is_empty()
        && certificate.signers.len() <= wire::MAX_VALIDATORS_PER_HEIGHT
        && certificate.signers.windows(2).all(|pair| pair[0] < pair[1])
        && signature_present(&certificate.aggregate_signature)
}

fn timeout_vote_shape(context: LifecycleContext, vote: &wire::TimeoutVote, signed: bool) -> bool {
    round_matches_context(context, vote.round)
        && signature_presence_matches(&vote.signature, signed)
        && vote.highest_prepare_qc.as_ref().is_none_or(|highest| {
            qc_shape(context, highest)
                && highest.phase == wire::GlobalPhase::Prepare
                && highest.round.view <= vote.round.view
        })
}

fn timeout_certificate_shape(
    context: LifecycleContext,
    certificate: &wire::TimeoutCertificate,
) -> bool {
    round_matches_context(context, certificate.round)
        && !certificate.groups.is_empty()
        && certificate.groups.iter().all(|group| {
            !group.signers.is_empty()
                && group.signers.windows(2).all(|pair| pair[0] < pair[1])
                && signature_present(&group.aggregate_signature)
                && group.highest_prepare_qc.as_ref().is_none_or(|highest| {
                    qc_shape(context, highest)
                        && highest.phase == wire::GlobalPhase::Prepare
                        && highest.round.view <= certificate.round.view
                })
        })
}

fn enter_view_shape(
    context: LifecycleContext,
    tag: ReplayEventTagV1,
    certificate: &wire::TimeoutCertificate,
    protected_lock: Option<&wire::QuorumCertificate>,
) -> bool {
    tag.height == context.height()
        && certificate.round.view.checked_add(1) == Some(tag.view)
        && protected_lock.is_none_or(|lock| {
            qc_shape(context, lock)
                && lock.phase == wire::GlobalPhase::Prepare
                && lock.proposal_round.view < tag.view
        })
        && match certificate.highest_prepare_qc() {
            None => true,
            Some(highest) => protected_lock.is_some_and(|protected| {
                protected.round.view > highest.round.view
                    || (protected.round.view == highest.round.view
                        && protected.round == highest.round
                        && protected.proposal_round == highest.proposal_round
                        && protected.phase == highest.phase
                        && protected.subject == highest.subject
                        && protected.execution_commitment == highest.execution_commitment)
            }),
        }
}

fn manifest_matches_origin(
    context: LifecycleContext,
    manifest: &wire::PayloadManifest,
    proposal_round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> bool {
    round_matches_context(context, manifest.round)
        && manifest.round == proposal_round
        && manifest.subject == subject
}

fn signature_presence_matches(signature: &[u8], signed: bool) -> bool {
    if signed {
        signature_present(signature)
    } else {
        signature.is_empty()
    }
}

fn signature_present(signature: &[u8]) -> bool {
    !signature.is_empty() && signature.len() <= wire::MAX_CONSENSUS_SIGNATURE_BYTES
}

fn round_matches_context(context: LifecycleContext, round: wire::ConsensusRound) -> bool {
    round.height == context.height()
        && digest_from_bytes(round.context_id.0.as_ref()) == context.id()
}

fn lifecycle_key(
    context: LifecycleContext,
    round: wire::ConsensusRound,
    proposal_round: Option<wire::ConsensusRound>,
    subject: Option<LifecycleDigest>,
    phase: LifecyclePhase,
    commitment: Option<LifecycleDigest>,
) -> LifecycleKey {
    LifecycleKey::new(
        context.id(),
        LifecycleRound::new(round.height, round.view),
        proposal_round.map(|round| LifecycleRound::new(round.height, round.view)),
        subject,
        phase,
        commitment,
    )
}

fn equivocation_subject(evidence: &wire::SumeragiV2Equivocation) -> LifecycleDigest {
    let (kind, offender, mut first, mut second) = match evidence {
        wire::SumeragiV2Equivocation::Proposal { first, second } => (
            1,
            first.proposer,
            first.signature_preimage(),
            second.signature_preimage(),
        ),
        wire::SumeragiV2Equivocation::PhaseVote { first, second } => (
            2,
            first.signer,
            first.signature_preimage(),
            second.signature_preimage(),
        ),
        wire::SumeragiV2Equivocation::TimeoutVote { first, second } => (
            3,
            first.signer,
            first.signature_preimage(),
            second.signature_preimage(),
        ),
    };
    if second < first {
        core::mem::swap(&mut first, &mut second);
    }
    let mut projection = Vec::new();
    projection.extend_from_slice(EQUIVOCATION_SUBJECT_DOMAIN);
    projection.push(kind);
    projection.extend_from_slice(&offender.to_le_bytes());
    append_field(&mut projection, &first);
    append_field(&mut projection, &second);
    digest_from_hash(&Hash::new(projection))
}

fn append_field(projection: &mut Vec<u8>, field: &[u8]) {
    projection.extend_from_slice(
        &u64::try_from(field.len())
            .expect("bounded replay-authority projection field fits u64")
            .to_le_bytes(),
    );
    projection.extend_from_slice(field);
}

fn digest_from_hash(hash: &Hash) -> LifecycleDigest {
    digest_from_bytes(hash.as_ref())
}

fn digest_from_bytes(bytes: &[u8]) -> LifecycleDigest {
    let mut digest = [0; 32];
    digest.copy_from_slice(bytes);
    LifecycleDigest::new(digest)
}

// Decoded envelopes remain inert persisted evidence. TODO: Reauthenticate each
// retained source against its owning durable store during startup before the
// registry reconstructs executable replay work.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    #[cfg(feature = "bls")]
    use std::num::NonZeroU64;

    #[cfg(feature = "bls")]
    use iroha_crypto::SignatureOf;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    #[cfg(feature = "bls")]
    use iroha_data_model::block::{BlockHeader, BlockSignature, SignedBlock};
    use iroha_data_model::peer::PeerId;
    use tempfile::TempDir;

    use super::super::schema::DurableContinuationEdge;
    use super::*;
    use crate::sumeragi::{
        v2::AdapterEquivocationEvidence,
        v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
        v2_core::Generation,
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
        v2_transport::authenticate_certified_body_request,
    };
    #[cfg(feature = "bls")]
    use crate::sumeragi::{
        v2::VerifiedHeightContext, v2_body_store::V2BodyStore,
        v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome,
    };

    pub(in crate::sumeragi::v2_lifecycle_coordinator) struct ReplayCase {
        pub(in crate::sumeragi::v2_lifecycle_coordinator) authority: LifecycleReplayAuthorityV1,
        pub(in crate::sumeragi::v2_lifecycle_coordinator) key: LifecycleKey,
        pub(in crate::sumeragi::v2_lifecycle_coordinator) work_class: LifecycleWorkClass,
        pub(in crate::sumeragi::v2_lifecycle_coordinator) stage: LifecycleStage,
        pub(in crate::sumeragi::v2_lifecycle_coordinator) payload: DurablePayloadReference,
    }

    struct Fixture {
        context: LifecycleContext,
        tag: ReplayEventTagV1,
        enter_tag: ReplayEventTagV1,
        proposal: wire::Proposal,
        conflicting_proposal: wire::Proposal,
        prepare_vote: wire::Vote,
        commit_vote: wire::Vote,
        conflicting_vote: wire::Vote,
        timeout_vote: wire::TimeoutVote,
        conflicting_timeout_vote: wire::TimeoutVote,
        prepare_qc: wire::QuorumCertificate,
        commit_qc: wire::QuorumCertificate,
        timeout_certificate: wire::TimeoutCertificate,
        serve_request: wire::CertifiedBodyRequest,
        body_receipt: DurableBodyReceipt,
        body_payload: DurablePayloadReference,
        serve_payload: DurablePayloadReference,
    }

    impl Fixture {
        fn new() -> Self {
            let context_hash = Hash::new(b"lifecycle replay authority context");
            Self::for_record(
                LifecycleContext::new(digest_from_bytes(context_hash.as_ref()), 7),
                0,
            )
        }

        fn for_record(context: LifecycleContext, seed: u8) -> Self {
            let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(
                Hash::prehashed(*context.id().as_bytes()),
            ));
            let round = wire::ConsensusRound {
                context_id,
                height: context.height(),
                view: u64::from(seed),
            };
            let subject_marker = seed.wrapping_add(0x31);
            let subject = self::subject(subject_marker);
            let conflicting_subject = self::subject(subject_marker.wrapping_add(1));
            let manifest = manifest(round, subject, subject_marker);
            let conflicting_manifest =
                manifest(round, conflicting_subject, subject_marker.wrapping_add(1));
            let proposal = wire::Proposal {
                round,
                proposer: 0,
                subject,
                manifest: manifest.clone(),
                justification: wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate: None },
                ),
                signature: vec![subject_marker],
            };
            let conflicting_proposal = wire::Proposal {
                subject: conflicting_subject,
                manifest: conflicting_manifest,
                signature: vec![subject_marker.wrapping_add(1)],
                ..proposal.clone()
            };
            let commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"replay parent state"),
                Hash::new(b"replay post state"),
                Hash::new(b"replay ordinary writes"),
                1,
                Hash::new(b"replay executed block"),
            );
            let prepare_vote = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment: commitment,
                signer: 0,
                signature: vec![0x41],
            };
            let commit_vote = wire::Vote {
                phase: wire::GlobalPhase::Commit,
                signature: vec![0x42],
                ..prepare_vote.clone()
            };
            let conflicting_vote = wire::Vote {
                subject: conflicting_subject,
                signature: vec![0x43],
                ..prepare_vote.clone()
            };
            let prepare_qc = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment: commitment,
                signers: vec![0],
                aggregate_signature: vec![0x51],
            };
            let commit_qc = wire::QuorumCertificate {
                phase: wire::GlobalPhase::Commit,
                aggregate_signature: vec![0x52],
                ..prepare_qc.clone()
            };
            let timeout_vote = wire::TimeoutVote {
                round,
                highest_prepare_qc: Some(prepare_qc.clone()),
                signer: 0,
                signature: vec![0x61],
            };
            let conflicting_timeout_vote = wire::TimeoutVote {
                highest_prepare_qc: None,
                signature: vec![0x62],
                ..timeout_vote.clone()
            };
            let timeout_certificate = wire::TimeoutCertificate {
                round,
                groups: vec![wire::TimeoutVoteGroup {
                    highest_prepare_qc: Some(prepare_qc.clone()),
                    signers: vec![0],
                    aggregate_signature: vec![0x63],
                }],
            };
            let requester_key =
                KeyPair::try_from_seed(vec![seed.wrapping_add(0x91); 32], Algorithm::Ed25519)
                    .expect("deterministic replay fixture requester key");
            let serve_request = wire::CertifiedBodyRequest {
                round,
                subject,
                certificate: prepare_qc.clone(),
                requester: PeerId::new(requester_key.public_key().clone()),
                signature: vec![0x71],
            };
            let body_receipt =
                DurableBodyReceipt::for_test(context_id, round, subject, HashOf::new(&manifest));
            let body_payload = DurablePayloadReference::BodyFrame(
                durable_body_frame_reference(context, &body_receipt)
                    .expect("canonical replay fixture body belongs to its context"),
            );
            let serve_payload = DurablePayloadReference::CertifiedServePending {
                request: digest_from_bytes(HashOf::new(&serve_request).as_ref()),
                certificate: digest_from_bytes(HashOf::new(&serve_request.certificate).as_ref()),
            };
            Self {
                context,
                tag: ReplayEventTagV1::new(round.height, round.view, 3),
                enter_tag: ReplayEventTagV1::new(round.height, round.view + 1, 0),
                proposal,
                conflicting_proposal,
                prepare_vote,
                commit_vote,
                conflicting_vote,
                timeout_vote,
                conflicting_timeout_vote,
                prepare_qc,
                commit_qc,
                timeout_certificate,
                serve_request,
                body_receipt,
                body_payload,
                serve_payload,
            }
        }

        fn cases(&self) -> Vec<ReplayCase> {
            let locator = RecoveredWalFrameIdentity::for_test(8, 9, [0x21; 32]).persisted_locator();
            let mut unsigned_proposal = self.proposal.clone();
            unsigned_proposal.signature.clear();
            let mut unsigned_prepare = self.prepare_vote.clone();
            unsigned_prepare.signature.clear();
            let mut unsigned_commit = self.commit_vote.clone();
            unsigned_commit.signature.clear();
            let mut unsigned_timeout = self.timeout_vote.clone();
            unsigned_timeout.signature.clear();
            let proposal_equivocation = crate::sumeragi::evidence::canonicalize_v2_conflict(
                &wire::SumeragiV2Equivocation::Proposal {
                    first: self.proposal.clone(),
                    second: self.conflicting_proposal.clone(),
                },
            );
            let vote_equivocation = crate::sumeragi::evidence::canonicalize_v2_conflict(
                &wire::SumeragiV2Equivocation::PhaseVote {
                    first: self.prepare_vote.clone(),
                    second: self.conflicting_vote.clone(),
                },
            );
            let timeout_equivocation = crate::sumeragi::evidence::canonicalize_v2_conflict(
                &wire::SumeragiV2Equivocation::TimeoutVote {
                    first: self.timeout_vote.clone(),
                    second: self.conflicting_timeout_vote.clone(),
                },
            );
            let sources = vec![
                (
                    LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                        locator,
                        role: ReplayWalRoleV1::PROPOSAL_INTENT,
                        tag: self.tag,
                        action: WalReplayActionV1::SignProposal(unsigned_proposal),
                    }),
                    LifecycleStageKind::SignProposal,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                        locator,
                        role: ReplayWalRoleV1::PREPARE_INTENT,
                        tag: self.tag,
                        action: WalReplayActionV1::SignVote(unsigned_prepare),
                    }),
                    LifecycleStageKind::SignPrepareVote,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                        locator,
                        role: ReplayWalRoleV1::LOCK_AND_COMMIT,
                        tag: self.tag,
                        action: WalReplayActionV1::SignVote(unsigned_commit),
                    }),
                    LifecycleStageKind::SignCommitVote,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                        locator,
                        role: ReplayWalRoleV1::TIMEOUT_INTENT,
                        tag: self.tag,
                        action: WalReplayActionV1::SignTimeoutVote(unsigned_timeout),
                    }),
                    LifecycleStageKind::SignTimeoutVote,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                        tag: self.tag,
                        origin: BodyPipelineOriginV1::Certified {
                            certificate: self.prepare_qc.clone(),
                            manifest: self.proposal.manifest.clone(),
                            fetch_manifest_present: true,
                            certified_sources: Vec::new(),
                        },
                    }),
                    LifecycleStageKind::FetchBody,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                        tag: self.tag,
                        origin: BodyPipelineOriginV1::Certified {
                            certificate: self.prepare_qc.clone(),
                            manifest: self.proposal.manifest.clone(),
                            fetch_manifest_present: true,
                            certified_sources: Vec::new(),
                        },
                    }),
                    LifecycleStageKind::StoreBody,
                    self.body_payload,
                ),
                (
                    LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                        tag: self.tag,
                        origin: BodyPipelineOriginV1::Certified {
                            certificate: self.prepare_qc.clone(),
                            manifest: self.proposal.manifest.clone(),
                            fetch_manifest_present: true,
                            certified_sources: Vec::new(),
                        },
                    }),
                    LifecycleStageKind::ValidateBody,
                    self.body_payload,
                ),
                (
                    LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                        locator,
                        role: ReplayWalRoleV1::DECISION,
                        tag: self.tag,
                        action: WalReplayActionV1::ApplyDecision(self.commit_qc.clone()),
                    }),
                    LifecycleStageKind::ApplyDecision,
                    self.body_payload,
                ),
                broadcast_case(
                    wire::ConsensusMessageV2Payload::Proposal(self.proposal.clone()),
                    LifecycleStageKind::BroadcastProposal,
                ),
                broadcast_case(
                    wire::ConsensusMessageV2Payload::Vote(self.prepare_vote.clone()),
                    LifecycleStageKind::BroadcastPrepareVote,
                ),
                broadcast_case(
                    wire::ConsensusMessageV2Payload::Vote(self.commit_vote.clone()),
                    LifecycleStageKind::BroadcastCommitVote,
                ),
                broadcast_case(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(self.prepare_qc.clone()),
                    LifecycleStageKind::BroadcastPrepareQc,
                ),
                broadcast_case(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(self.commit_qc.clone()),
                    LifecycleStageKind::BroadcastCommitQc,
                ),
                broadcast_case(
                    wire::ConsensusMessageV2Payload::TimeoutVote(self.timeout_vote.clone()),
                    LifecycleStageKind::BroadcastTimeoutVote,
                ),
                broadcast_case(
                    wire::ConsensusMessageV2Payload::TimeoutCertificate(
                        self.timeout_certificate.clone(),
                    ),
                    LifecycleStageKind::BroadcastTc,
                ),
                (
                    LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                        locator,
                        role: ReplayWalRoleV1::INSTALL_TIMEOUT,
                        tag: self.enter_tag,
                        action: WalReplayActionV1::EnterView {
                            certificate: self.timeout_certificate.clone(),
                            protected_lock: Some(self.prepare_qc.clone()),
                        },
                    }),
                    LifecycleStageKind::EnterView,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::Equivocation(proposal_equivocation),
                    LifecycleStageKind::ReportProposalEquivocation,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::Equivocation(vote_equivocation),
                    LifecycleStageKind::ReportVoteEquivocation,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::Equivocation(timeout_equivocation),
                    LifecycleStageKind::ReportTimeoutEquivocation,
                    DurablePayloadReference::None,
                ),
                (
                    LifecycleReplaySourceV1::InvalidCertifiedBody(InvalidBodyReplaySourceV1 {
                        validation_origin: BodyPipelineReplaySourceV1 {
                            tag: self.tag,
                            origin: BodyPipelineOriginV1::Proposal(self.proposal.clone()),
                        },
                        certificate: self.prepare_qc.clone(),
                        outcome: RejectedBodyOutcomeBindingV1 {
                            manifest: self.proposal.manifest.clone(),
                            body_frame_hash: [0x81; 32],
                            rejection_code: 0,
                        },
                    }),
                    LifecycleStageKind::ReportInvalidBody,
                    DurablePayloadReference::None,
                ),
                (
                    self.serve_storage_source(),
                    LifecycleStageKind::CertifiedServe,
                    self.serve_payload,
                ),
                (
                    self.serve_storage_source(),
                    LifecycleStageKind::ProducerTurn,
                    DurablePayloadReference::None,
                ),
            ];
            sources
                .into_iter()
                .map(|(source, stage_kind, payload)| {
                    replay_case(self.context, source, stage_kind, payload)
                })
                .collect()
        }

        fn recovered_tag(&self) -> EventTag {
            EventTag::new(
                self.tag.height,
                self.tag.view,
                Generation::new(self.tag.generation),
            )
        }

        fn serve_storage_source(&self) -> LifecycleReplaySourceV1 {
            LifecycleReplaySourceV1::CertifiedServeStorage(CertifiedServeStorageSourceV1 {
                request: self.serve_request.clone(),
                payload_hash: [0x91; 32],
                local_retainer: 0,
            })
        }
    }

    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_record_fixture(
        context: LifecycleContext,
        stage: LifecycleStageKind,
        seed: u8,
    ) -> ReplayCase {
        Fixture::for_record(context, seed)
            .cases()
            .into_iter()
            .find(|case| case.stage.kind() == stage)
            .expect("the canonical V1 fixture covers every lifecycle stage")
    }

    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_replay_authority_for_payload_fixture(
        context: LifecycleContext,
        stage: LifecycleStageKind,
        seed: u8,
        payload: DurablePayloadReference,
    ) -> LifecycleReplayAuthorityV1 {
        let case = exact_record_fixture(context, stage, seed);
        canonical_replay_authority(
            context,
            case.authority.source.clone(),
            stage,
            ReplayPayloadBindingV1::from_payload(payload),
        )
        .unwrap_or(case.authority)
    }

    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_body_record_fixture(
        context: LifecycleContext,
        stage: LifecycleStageKind,
        seed: u8,
    ) -> (ReplayCase, DurableBodyReceipt) {
        let fixture = Fixture::for_record(context, seed);
        let receipt = fixture.body_receipt.clone();
        let case = fixture
            .cases()
            .into_iter()
            .find(|case| case.stage.kind() == stage)
            .expect("the canonical V1 fixture covers every lifecycle stage");
        (case, receipt)
    }

    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn durable_certified_fetch_projection_fixture(
        context: LifecycleContext,
        causal_root: CausalRoot,
        seed: u8,
    ) -> DurableCertifiedFetchReplayProjectionV1 {
        let fixture = Fixture::for_record(context, seed);
        let coordinates = CertifiedBodyPipelineCoordinatesV1 {
            tag: fixture.tag,
            certificate: fixture.prepare_qc,
            manifest: fixture.proposal.manifest,
            fetch_manifest_present: true,
            certified_sources: Vec::new(),
        };
        let family = exact_certified_body_pipeline_family(&coordinates, &fixture.body_receipt)
            .expect("canonical fixture binds one body-fsynced Certified Fetch family");
        let effect = exact_certified_fetch_effect(&family)
            .expect("canonical fixture reconstructs one exact Fetch effect");
        let payload = DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(context, &fixture.body_receipt)
                .expect("canonical fixture body belongs to its lifecycle context"),
        );
        let authority = canonical_replay_authority(
            context,
            LifecycleReplaySourceV1::BodyPipeline(family.source),
            LifecycleStageKind::FetchBody,
            ReplayPayloadBindingV1::from_payload(payload),
        )
        .expect("canonical fixture projects one frame-bound Fetch authority");
        let causal_key = Hash::prehashed(*causal_root.digest().as_bytes());
        let effect_identity =
            crate::sumeragi::v2_runtime::adapter_effect_identity_for_test(&effect);
        let completion_digest = canonical_durable_certified_fetch_completion_digest(
            causal_key,
            effect_identity,
            &authority,
        );
        DurableCertifiedFetchReplayProjectionV1 {
            payload,
            authority,
            causal_key,
            effect_identity,
            completion_digest,
            expected_manifest_hash: fixture.body_receipt.manifest_hash(),
        }
    }

    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_durable_certified_fetch_record_fixture(
        context: LifecycleContext,
        tag: EventTag,
        certificate: wire::QuorumCertificate,
        manifest: wire::PayloadManifest,
        certified_sources: Vec<PeerId>,
        receipt: &DurableBodyReceipt,
    ) -> ReplayCase {
        let payload = DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(context, receipt)
                .expect("durable Certified Fetch fixture body belongs to its context"),
        );
        replay_case(
            context,
            LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
                origin: BodyPipelineOriginV1::Certified {
                    certificate,
                    manifest,
                    fetch_manifest_present: true,
                    certified_sources,
                },
            }),
            LifecycleStageKind::FetchBody,
            payload,
        )
    }

    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_local_body_record_fixture(
        context: LifecycleContext,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
        stage: LifecycleStageKind,
    ) -> Option<ReplayCase> {
        if receipt.context_id() != manifest.round.context_id
            || receipt.round() != manifest.round
            || receipt.subject() != manifest.subject
            || receipt.manifest_hash() != HashOf::new(&manifest)
        {
            return None;
        }
        let payload =
            DurablePayloadReference::BodyFrame(durable_body_frame_reference(context, receipt)?);
        Some(replay_case(
            context,
            LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
                origin: BodyPipelineOriginV1::LocalBody(manifest),
            }),
            stage,
            payload,
        ))
    }

    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn foreign_certified_serve_family_authority_fixture(
        context: LifecycleContext,
        stage: LifecycleStageKind,
        seed: u8,
    ) -> LifecycleReplayAuthorityV1 {
        let case = exact_record_fixture(context, stage, seed);
        let mut authority = case.authority;
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut authority.source else {
            panic!("Certified-Serve family fixture requires a Serve or ProducerTurn stage")
        };
        source.payload_hash[0] ^= 1;
        assert!(authority.structurally_matches_record(
            context,
            case.key,
            case.work_class,
            case.stage,
            case.payload,
        ));
        authority
    }

    struct CertifiedServeReplayFixture {
        context: wire::HeightContext,
        active_context: LifecycleContext,
        authenticated: AuthenticatedCertifiedBodyRequest,
    }

    impl CertifiedServeReplayFixture {
        fn new() -> Self {
            let mut keys = (0x81_u8..=0x84)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                        .expect("deterministic Certified-Serve replay key")
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
                network_id: crate::sumeragi::synthetic_network_id(
                    "certified-serve-replay-authority-test",
                ),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 2,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"Certified-Serve replay AMX context"),
                execution_policy_hash: Hash::new(b"Certified-Serve replay execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 8,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 16,
                    max_chunk_count: 4,
                },
                leader_seed: [0xA7; 32],
            };
            context
                .validate()
                .expect("valid Certified-Serve replay context");
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            };
            let request_subject = subject(0x91);
            let mut request = wire::CertifiedBodyRequest {
                round,
                subject: request_subject,
                certificate: wire::QuorumCertificate {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Prepare,
                    subject: request_subject,
                    execution_commitment:
                        wire::ExecutionCommitment::without_topups_or_merge_carrier(
                            Hash::new(b"Certified-Serve replay parent state"),
                            Hash::new(b"Certified-Serve replay post state"),
                            Hash::new(b"Certified-Serve replay ordinary writes"),
                            1,
                            Hash::new(b"Certified-Serve replay executed block"),
                        ),
                    signers: vec![0, 1, 2],
                    aggregate_signature: vec![0xA5; 48],
                },
                requester: PeerId::new(keys[3].public_key().clone()),
                signature: Vec::new(),
            };
            request.signature =
                Signature::new(keys[3].private_key(), &request.signature_preimage())
                    .payload()
                    .to_vec();
            let requester = request.requester.clone();
            let authenticated =
                authenticate_certified_body_request(&context, request, &requester, |_, _| {
                    Ok::<(), &'static str>(())
                })
                .expect("authenticate Certified-Serve replay request");
            let active_context =
                LifecycleContext::new(digest_from_bytes(context.id().0.as_ref()), context.height);
            Self {
                context,
                active_context,
                authenticated,
            }
        }

        fn pending_payload(&self) -> DurablePayloadReference {
            DurablePayloadReference::CertifiedServePending {
                request: digest_from_bytes(self.authenticated.request_hash().as_ref()),
                certificate: digest_from_bytes(
                    HashOf::new(&self.authenticated.request().certificate).as_ref(),
                ),
            }
        }
    }

    #[cfg(feature = "bls")]
    #[derive(Clone, Copy)]
    enum RecoveredServeState {
        Pending,
        Completed,
        Negative,
    }

    #[cfg(feature = "bls")]
    struct CertifiedServeRecoveredReplayFixture {
        verified: VerifiedHeightContext,
        keys: Vec<KeyPair>,
        body: Vec<u8>,
        manifest: wire::PayloadManifest,
        authenticated: AuthenticatedCertifiedBodyRequest,
        response: wire::CertifiedBodyResponse,
    }

    #[cfg(feature = "bls")]
    impl CertifiedServeRecoveredReplayFixture {
        #[allow(clippy::too_many_lines)]
        fn new() -> Self {
            let mut keys = (0x91_u8..=0x94)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic recovered Serve replay BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("recovered Serve replay proof of possession")
                })
                .collect::<Vec<_>>();
            let roster = keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let context = wire::HeightContext {
                network_id: crate::sumeragi::synthetic_network_id(
                    "recovered-certified-serve-replay-test",
                ),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"recovered Serve replay AMX context"),
                execution_policy_hash: Hash::new(b"recovered Serve replay execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1_048_576,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 1_048_576,
                    max_chunk_count: 2,
                },
                leader_seed: [0xB7; 32],
            };
            let verified = VerifiedHeightContext::genesis(context, proofs)
                .expect("verified recovered Serve replay context");
            let context = verified.context();
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            };
            let leader = context.leader(round.view);
            let leader_index = usize::try_from(leader).expect("fixture leader index fits usize");
            let header = BlockHeader::new(
                NonZeroU64::new(round.height).expect("non-zero fixture height"),
                None,
                None,
                None,
                1_000,
                round.view,
            );
            let block_signature =
                SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
                    .expect("sign recovered Serve replay block");
            let block = SignedBlock::presigned(
                BlockSignature::new(u64::from(leader), block_signature),
                header,
                Vec::new(),
            );
            let body = block.encode_wire().expect("canonical recovered Serve body");
            let request_subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: block.hash(),
                payload_hash: Hash::new(&body),
            };
            let chunks = wire::encode_payload_chunks(context.da_layout, &body)
                .expect("encode recovered Serve replay body");
            let manifest = wire::PayloadManifest::derive(
                context,
                round,
                request_subject,
                u64::try_from(body.len()).expect("fixture body length fits u64"),
                &chunks,
            )
            .expect("derive recovered Serve replay manifest");
            let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"recovered Serve replay parent state"),
                Hash::new(b"recovered Serve replay post state"),
                Hash::new(b"recovered Serve replay ordinary writes"),
                1,
                Hash::new(b"recovered Serve replay executed block"),
            );
            let signers = vec![0, 1, 2];
            let preimage = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: request_subject,
                execution_commitment,
                signer: 0,
                signature: Vec::new(),
            }
            .signature_preimage();
            let shares = signers
                .iter()
                .map(|signer| {
                    Signature::new(
                        keys[usize::try_from(*signer).expect("small fixture signer")].private_key(),
                        &preimage,
                    )
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate recovered Serve replay PrepareQC");
            let mut request = wire::CertifiedBodyRequest {
                round,
                subject: request_subject,
                certificate: wire::QuorumCertificate {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Prepare,
                    subject: request_subject,
                    execution_commitment,
                    signers,
                    aggregate_signature,
                },
                requester: PeerId::new(keys[3].public_key().clone()),
                signature: Vec::new(),
            };
            request.signature =
                Signature::new(keys[3].private_key(), &request.signature_preimage())
                    .payload()
                    .to_vec();
            let requester = request.requester.clone();
            let authenticated = authenticate_certified_body_request(
                context,
                request,
                &requester,
                |context, certificate| {
                    wire::finality::verify_quorum_certificate_with_validator_pops(
                        context,
                        certificate,
                        verified.proofs_of_possession(),
                    )
                    .map_err(|error| error.to_string())
                },
            )
            .expect("authenticate recovered Serve replay request");
            let mut response = wire::CertifiedBodyResponse {
                request_hash: authenticated.request_hash(),
                manifest: manifest.clone(),
                body: body.clone(),
                responder: 0,
                signature: Vec::new(),
            };
            response.signature =
                Signature::new(keys[0].private_key(), &response.signature_preimage())
                    .payload()
                    .to_vec();
            Self {
                verified,
                keys,
                body,
                manifest,
                authenticated,
                response,
            }
        }

        fn replay_pair(&self, state: RecoveredServeState) -> CertifiedServeReplayEvidencePairV1 {
            let temporary = TempDir::new().expect("temporary recovered Serve replay directory");
            let context = self.verified.context();
            let mut body_store = V2BodyStore::open(temporary.path(), context.clone())
                .expect("open recovered Serve body store");
            if matches!(state, RecoveredServeState::Completed) {
                body_store
                    .store(self.manifest.clone(), self.body.clone())
                    .expect("persist recovered Serve body");
            }
            let (mut payload_store, _) =
                CertifiedServePayloadStoreV1::open(temporary.path(), context)
                    .expect("open recovered Serve payload store");
            let pending = payload_store
                .persist_pending_with_verified_retention(
                    &self.verified,
                    &self.keys[0],
                    &self.authenticated,
                )
                .expect("persist verified recovered Serve request");
            match state {
                RecoveredServeState::Pending => {}
                RecoveredServeState::Completed => {
                    payload_store
                        .persist_completed(&self.authenticated, &self.response)
                        .expect("persist recovered Serve completion");
                }
                RecoveredServeState::Negative => {
                    payload_store
                        .persist_negative(
                            pending.id(),
                            CertifiedServePayloadNegativeOutcome::Rejected(17),
                        )
                        .expect("persist recovered Serve negative outcome");
                }
            }
            drop(payload_store);
            let (_, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), context)
                .expect("reopen recovered Serve payload store");
            let authenticated_recovery = recovery
                .authenticate(&self.verified, &self.keys[0], &body_store)
                .expect("authenticate recovered Serve payload state");
            let recovered = authenticated_recovery
                .get(pending.id())
                .expect("recover exact Serve request");
            assert_eq!(recovered.local_retainer(), 0);
            assert!(recovered.exactly_matches_persisted_payload());
            let active_context =
                LifecycleContext::new(digest_from_bytes(context.id().0.as_ref()), context.height);
            CertifiedServeReplayEvidencePairV1::from_authenticated_recovery(
                active_context,
                recovered,
            )
            .expect("reconstruct recovered Serve/Producer replay pair")
        }
    }

    fn broadcast_case(
        payload: wire::ConsensusMessageV2Payload,
        stage: LifecycleStageKind,
    ) -> (
        LifecycleReplaySourceV1,
        LifecycleStageKind,
        DurablePayloadReference,
    ) {
        (
            LifecycleReplaySourceV1::ConsensusBroadcast(wire::ConsensusMessageV2 {
                protocol_version: wire::PROTOCOL_VERSION,
                payload,
            }),
            stage,
            DurablePayloadReference::None,
        )
    }

    fn replay_case(
        context: LifecycleContext,
        source: LifecycleReplaySourceV1,
        stage_kind: LifecycleStageKind,
        payload: DurablePayloadReference,
    ) -> ReplayCase {
        let payload_binding = ReplayPayloadBindingV1::from_payload(payload);
        let shape = source
            .project(context, stage_kind, &payload_binding)
            .expect("fixture replay source projects");
        let predecessor = match shape.work_class {
            LifecycleWorkClass::CertifiedServe => PredecessorScope::ReadyOrdinalPrefix,
            LifecycleWorkClass::ProducerTurn => PredecessorScope::ProducerHandoffBarrier,
            _ => PredecessorScope::Independent,
        };
        ReplayCase {
            authority: LifecycleReplayAuthorityV1 {
                format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
                payload: payload_binding,
                source,
            },
            key: shape.key,
            work_class: shape.work_class,
            stage: LifecycleStage::new(stage_kind, predecessor),
            payload,
        }
    }

    fn pending_binding(
        effect: &AdapterEffect,
        tag: EventTag,
        ordinal: u128,
    ) -> PendingRuntimeEffectBinding {
        bind_adapter_effect_batch_ownership(
            core::slice::from_ref(effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .expect("bind exact direct signed replay fixture")
        .pop()
        .expect("one direct signed replay fixture owner")
        .pending_adapter_effect_binding(effect)
        .expect("mint exact direct signed replay pending binding")
    }

    fn signed_broadcast_effects(fixture: &Fixture) -> Vec<AdapterEffect> {
        [
            wire::ConsensusMessageV2Payload::Proposal(fixture.proposal.clone()),
            wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
            wire::ConsensusMessageV2Payload::Vote(fixture.commit_vote.clone()),
            wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.prepare_qc.clone()),
            wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.commit_qc.clone()),
            wire::ConsensusMessageV2Payload::TimeoutVote(fixture.timeout_vote.clone()),
            wire::ConsensusMessageV2Payload::TimeoutCertificate(
                fixture.timeout_certificate.clone(),
            ),
        ]
        .into_iter()
        .map(|payload| AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(payload)))
        .collect()
    }

    fn subject(marker: u8) -> wire::BlockSubject {
        wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 0xA1])),
            payload_hash: Hash::new([marker, 0xA2]),
        }
    }

    fn manifest(
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        marker: u8,
    ) -> wire::PayloadManifest {
        wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1024,
                max_chunk_count: 2,
            },
            chunk_hashes: vec![Hash::new([marker, 0xA3])],
            chunk_root: Hash::new([marker, 0xA4]),
        }
    }

    #[test]
    fn every_stage_has_one_canonical_round_trip_and_exact_record_mapping() {
        let fixture = Fixture::new();
        assert_eq!(fixture.tag.generation(), 3);
        let cases = fixture.cases();
        assert_eq!(cases.len(), LifecycleStageKind::ALL.len());
        let stages = cases
            .iter()
            .map(|case| case.stage.kind())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            stages,
            LifecycleStageKind::ALL.into_iter().collect::<BTreeSet<_>>()
        );

        for case in cases {
            let encoded = case.authority.encode();
            assert!(encoded.len() <= MAX_REPLAY_AUTHORITY_BYTES);
            let decoded = LifecycleReplayAuthorityV1::decode_canonical(&encoded)
                .expect("canonical replay authority decodes");
            assert_eq!(decoded, case.authority);
            decoded
                .validate_record(
                    fixture.context,
                    case.key,
                    case.work_class,
                    case.stage,
                    case.payload,
                )
                .expect("exact lifecycle row matches its replay envelope");
        }
    }

    #[test]
    fn canonical_decoder_enforces_version_size_and_complete_input() {
        let fixture = Fixture::new();
        let mut authority = fixture
            .cases()
            .into_iter()
            .next()
            .expect("fixture has cases")
            .authority;
        assert_eq!(
            LifecycleReplayAuthorityV1::decode_canonical(&[]),
            Err(ReplayAuthorityCodecError::FrameBounds)
        );
        assert_eq!(
            LifecycleReplayAuthorityV1::decode_canonical(&vec![0; MAX_REPLAY_AUTHORITY_BYTES + 1]),
            Err(ReplayAuthorityCodecError::FrameBounds)
        );

        authority.format_version = REPLAY_AUTHORITY_FORMAT_VERSION + 1;
        assert_eq!(
            LifecycleReplayAuthorityV1::decode_canonical(&authority.encode()),
            Err(ReplayAuthorityCodecError::UnsupportedVersion)
        );

        authority.format_version = REPLAY_AUTHORITY_FORMAT_VERSION;
        let mut trailing = authority.encode();
        trailing.push(0);
        assert!(matches!(
            LifecycleReplayAuthorityV1::decode_canonical(&trailing),
            Err(ReplayAuthorityCodecError::InvalidEncoding
                | ReplayAuthorityCodecError::NonCanonicalEncoding)
        ));
    }

    #[test]
    fn decoded_decision_fetch_rejects_duplicate_certified_sources() {
        let fixture = Fixture::new();
        let first_key = KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519)
            .expect("deterministic first Decision Fetch source");
        let second_key = KeyPair::try_from_seed(vec![0xD2; 32], Algorithm::Ed25519)
            .expect("deterministic second Decision Fetch source");
        let first = PeerId::new(first_key.public_key().clone());
        let second = PeerId::new(second_key.public_key().clone());
        let payload = ReplayPayloadBindingV1::None;
        let source = WalReplaySourceV1 {
            locator: RecoveredWalFrameIdentity::for_test(8, 9, [0xD3; 32]).persisted_locator(),
            role: ReplayWalRoleV1::DECISION,
            tag: fixture.tag,
            action: WalReplayActionV1::FetchDecision {
                certificate: fixture.commit_qc.clone(),
                certified_sources: vec![first.clone(), second],
            },
        };
        let shape = source
            .project(fixture.context, LifecycleStageKind::FetchBody, &payload)
            .expect("unique Decision Fetch source roster is structurally valid");
        let authority = LifecycleReplayAuthorityV1 {
            format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
            payload: payload.clone(),
            source: LifecycleReplaySourceV1::Wal(source),
        };
        let mut decoded = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode())
            .expect("unique Decision Fetch source decodes canonically");
        decoded
            .validate_record(
                fixture.context,
                shape.key,
                LifecycleWorkClass::Fetch,
                LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
                DurablePayloadReference::None,
            )
            .expect("unique Decision Fetch source projects exactly");

        let LifecycleReplaySourceV1::Wal(source) = &mut decoded.source else {
            unreachable!("Decision Fetch retains its WAL replay source")
        };
        let WalReplayActionV1::FetchDecision {
            certified_sources, ..
        } = &mut source.action
        else {
            unreachable!("Decision Fetch retains its exact replay action")
        };
        *certified_sources = vec![first.clone(), first];
        let duplicate = LifecycleReplayAuthorityV1::decode_canonical(&decoded.encode())
            .expect("duplicate source bytes remain canonically decodable");
        assert_eq!(
            duplicate.validate_record(
                fixture.context,
                shape.key,
                LifecycleWorkClass::Fetch,
                LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent,),
                DurablePayloadReference::None,
            ),
            Err(ReplayAuthorityValidationError::InvalidSource)
        );
    }

    #[test]
    fn recovered_decision_body_lineage_is_stage_closed_and_predecessor_bound() {
        let fixture = Fixture::new();
        let source_key = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::Ed25519)
            .expect("deterministic recovered Decision source");
        let locator = RecoveredWalFrameIdentity::for_test(21, 22, [0xD5; 32]).persisted_locator();
        let fetch = replay_case(
            fixture.context,
            LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                locator,
                role: ReplayWalRoleV1::DECISION,
                tag: fixture.tag,
                action: WalReplayActionV1::FetchDecision {
                    certificate: fixture.commit_qc.clone(),
                    certified_sources: vec![PeerId::new(source_key.public_key().clone())],
                },
            }),
            LifecycleStageKind::FetchBody,
            DurablePayloadReference::None,
        );
        let body_source = LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
            tag: fixture.tag,
            origin: BodyPipelineOriginV1::RecoveredDecision {
                locator,
                certificate: fixture.commit_qc.clone(),
                manifest: fixture.proposal.manifest.clone(),
            },
        });
        assert!(matches!(
            body_source.project(
                fixture.context,
                LifecycleStageKind::FetchBody,
                &ReplayPayloadBindingV1::from_payload(fixture.body_payload),
            ),
            Err(ReplayAuthorityValidationError::RecordMismatch)
        ));
        assert!(matches!(
            body_source.project(
                fixture.context,
                LifecycleStageKind::ApplyDecision,
                &ReplayPayloadBindingV1::from_payload(fixture.body_payload),
            ),
            Err(ReplayAuthorityValidationError::RecordMismatch)
        ));
        let store = replay_case(
            fixture.context,
            body_source.clone(),
            LifecycleStageKind::StoreBody,
            fixture.body_payload,
        );
        let validate = replay_case(
            fixture.context,
            body_source,
            LifecycleStageKind::ValidateBody,
            fixture.body_payload,
        );
        let apply = replay_case(
            fixture.context,
            LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                locator,
                role: ReplayWalRoleV1::DECISION,
                tag: fixture.tag,
                action: WalReplayActionV1::ApplyDecision(fixture.commit_qc.clone()),
            }),
            LifecycleStageKind::ApplyDecision,
            fixture.body_payload,
        );
        assert_eq!(
            recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::FetchToStore,
                &fetch.authority,
                fetch.payload,
                &store.authority,
                store.payload,
            ),
            Some(true)
        );
        assert_eq!(
            recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::StoreToValidate,
                &store.authority,
                store.payload,
                &validate.authority,
                validate.payload,
            ),
            Some(true)
        );
        assert_eq!(
            recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::ValidateToApply,
                &validate.authority,
                validate.payload,
                &apply.authority,
                apply.payload,
            ),
            Some(true)
        );
        assert_eq!(
            recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::FetchToStore,
                &fetch.authority,
                fetch.payload,
                &validate.authority,
                validate.payload,
            ),
            Some(false),
            "the recovered lineage cannot skip Store"
        );

        let mut foreign_store = store.authority.clone();
        let LifecycleReplaySourceV1::BodyPipeline(source) = &mut foreign_store.source else {
            unreachable!("recovered Store retains one body-pipeline source")
        };
        let BodyPipelineOriginV1::RecoveredDecision { locator, .. } = &mut source.origin else {
            unreachable!("recovered Store retains one Decision origin")
        };
        *locator = RecoveredWalFrameIdentity::for_test(22, 23, [0xD6; 32]).persisted_locator();
        assert_eq!(
            recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::FetchToStore,
                &fetch.authority,
                fetch.payload,
                &foreign_store,
                store.payload,
            ),
            Some(false),
            "a foreign exact locator cannot enter the body lineage"
        );
    }

    #[test]
    fn nested_record_validation_rejects_oversized_canonical_authority() {
        let fixture = Fixture::new();
        let case = fixture.cases().remove(8);
        assert_eq!(case.stage.kind(), LifecycleStageKind::BroadcastProposal);
        let mut authority = case.authority;
        let LifecycleReplaySourceV1::ConsensusBroadcast(message) = &mut authority.source else {
            panic!("BroadcastProposal fixture retains one consensus message")
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut message.payload else {
            panic!("BroadcastProposal fixture retains one proposal")
        };
        proposal.signature = vec![0xA5; MAX_REPLAY_AUTHORITY_BYTES + 1];
        assert!(authority.encode().len() > MAX_REPLAY_AUTHORITY_BYTES);
        assert_eq!(
            authority.validate_record(
                fixture.context,
                case.key,
                case.work_class,
                case.stage,
                case.payload,
            ),
            Err(ReplayAuthorityValidationError::InvalidEncoding)
        );
        assert!(!authority.structurally_matches_record(
            fixture.context,
            case.key,
            case.work_class,
            case.stage,
            case.payload,
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn certified_serve_pending_replay_pair_binds_exact_fsync_origin_and_records() {
        let temporary = TempDir::new().expect("temporary Certified-Serve replay directory");
        let fixture = CertifiedServeReplayFixture::new();
        let (mut store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open Certified-Serve replay payload store");
        assert!(recovery.is_empty());
        let receipt = store
            .persist_pending(&fixture.authenticated)
            .expect("persist exact Pending Certified-Serve request");
        let pair = CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
            fixture.active_context,
            &fixture.authenticated,
            receipt,
        )
        .expect("seal exact post-fsync Serve/Producer replay pair");
        assert!(pair.shares_exact_storage_origin());

        let serve_shape = pair
            .serve
            .family
            .source
            .project(
                fixture.active_context,
                LifecycleStageKind::CertifiedServe,
                &pair.serve.payload,
            )
            .expect("derive fixed Certified-Serve record");
        let producer_shape = pair
            .producer
            .family
            .source
            .project(
                fixture.active_context,
                LifecycleStageKind::ProducerTurn,
                &ReplayPayloadBindingV1::None,
            )
            .expect("derive fixed ProducerTurn record");
        let serve_stage = LifecycleStage::new(
            LifecycleStageKind::CertifiedServe,
            PredecessorScope::ReadyOrdinalPrefix,
        );
        let producer_stage = LifecycleStage::new(
            LifecycleStageKind::ProducerTurn,
            PredecessorScope::ProducerHandoffBarrier,
        );
        assert!(pair.exactly_matches_serve_record(
            fixture.active_context,
            serve_shape.key,
            serve_stage,
            fixture.pending_payload(),
            receipt.payload_hash(),
        ));
        assert!(pair.exactly_matches_producer_record(
            fixture.active_context,
            producer_shape.key,
            producer_stage,
            DurablePayloadReference::None,
            receipt.payload_hash(),
        ));

        let shared = Arc::new(pair);
        let adjacent = Arc::clone(&shared);
        assert!(Arc::ptr_eq(&shared, &adjacent));
        assert!(shared.shares_exact_storage_origin());
        assert!(shared.exactly_matches_serve_record(
            fixture.active_context,
            serve_shape.key,
            serve_stage,
            fixture.pending_payload(),
            receipt.payload_hash(),
        ));
        assert!(adjacent.exactly_matches_producer_record(
            fixture.active_context,
            producer_shape.key,
            producer_stage,
            DurablePayloadReference::None,
            receipt.payload_hash(),
        ));

        let foreign_request_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"foreign Certified-Serve replay request"));
        assert!(
            CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
                fixture.active_context,
                &fixture.authenticated,
                receipt.with_request_hash_for_test(foreign_request_hash),
            )
            .is_none()
        );
        let foreign_certificate_hash = HashOf::from_untyped_unchecked(Hash::new(
            b"foreign Certified-Serve replay certificate",
        ));
        assert!(
            CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
                fixture.active_context,
                &fixture.authenticated,
                receipt.with_certificate_hash_for_test(foreign_certificate_hash),
            )
            .is_none()
        );
        assert!(
            CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
                fixture.active_context,
                &fixture.authenticated,
                receipt.with_payload_hash_for_test(Hash::new(
                    b"foreign Certified-Serve replay payload",
                )),
            )
            .is_none()
        );
        let out_of_range = wire::ValidatorIndex::try_from(wire::MAX_VALIDATORS_PER_HEIGHT)
            .expect("validator hard bound fits its wire index");
        assert!(
            CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
                fixture.active_context,
                &fixture.authenticated,
                receipt.with_local_retainer_for_test(out_of_range),
            )
            .is_none()
        );
        assert!(
            CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
                fixture.active_context,
                &fixture.authenticated,
                receipt.with_local_retainer_for_test(1),
            )
            .is_none(),
            "a different QC signer cannot replace the receipt's exact local retainer"
        );
        assert!(
            CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
                fixture.active_context,
                &fixture.authenticated,
                receipt.with_local_retainer_for_test(3),
            )
            .is_none(),
            "a roster member absent from the QC signer set cannot retain replay authority"
        );

        let foreign_context = LifecycleContext::new(
            LifecycleDigest::new([0xD1; 32]),
            fixture.active_context.height(),
        );
        assert!(
            CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
                foreign_context,
                &fixture.authenticated,
                receipt,
            )
            .is_none()
        );
        assert!(!pair.exactly_matches_serve_record(
            fixture.active_context,
            producer_shape.key,
            serve_stage,
            fixture.pending_payload(),
            receipt.payload_hash(),
        ));
        assert!(!pair.exactly_matches_serve_record(
            fixture.active_context,
            serve_shape.key,
            producer_stage,
            fixture.pending_payload(),
            receipt.payload_hash(),
        ));
        assert!(!pair.exactly_matches_serve_record(
            fixture.active_context,
            serve_shape.key,
            serve_stage,
            DurablePayloadReference::None,
            receipt.payload_hash(),
        ));
        assert!(!pair.exactly_matches_serve_record(
            fixture.active_context,
            serve_shape.key,
            serve_stage,
            fixture.pending_payload(),
            Hash::new(b"wrong retained payload hash"),
        ));
        assert!(!pair.exactly_matches_producer_record(
            fixture.active_context,
            producer_shape.key,
            producer_stage,
            fixture.pending_payload(),
            receipt.payload_hash(),
        ));

        let authority = LifecycleReplayAuthorityV1 {
            format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
            payload: pair.serve.payload.clone(),
            source: LifecycleReplaySourceV1::CertifiedServeStorage(
                pair.serve.family.source.clone(),
            ),
        };
        let canonical = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode())
            .expect("exact Certified-Serve replay source canonical-roundtrips");
        assert!(pair.serve.exactly_matches_authority(&canonical));

        let mut wrong_payload_source = canonical.clone();
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) =
            &mut wrong_payload_source.source
        else {
            unreachable!("Serve replay authority retains its storage source")
        };
        source.payload_hash[0] ^= 1;
        assert!(!pair.serve.exactly_matches_authority(&wrong_payload_source));

        let mut wrong_qc_source = canonical.clone();
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut wrong_qc_source.source
        else {
            unreachable!("Serve replay authority retains its storage source")
        };
        source.request.certificate.aggregate_signature[0] ^= 1;
        let wrong_qc_source =
            LifecycleReplayAuthorityV1::decode_canonical(&wrong_qc_source.encode())
                .expect("mutated QC source remains canonical codec data");
        assert!(!pair.serve.exactly_matches_authority(&wrong_qc_source));

        let mut absent_retainer = canonical;
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut absent_retainer.source
        else {
            unreachable!("Serve replay authority retains its storage source")
        };
        source.local_retainer = 3;
        assert!(
            absent_retainer
                .validate_record(
                    fixture.active_context,
                    serve_shape.key,
                    LifecycleWorkClass::CertifiedServe,
                    serve_stage,
                    fixture.pending_payload(),
                )
                .is_err()
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_serve_states_reconstruct_one_common_source_per_replay_pair() {
        let fixture = CertifiedServeRecoveredReplayFixture::new();
        let pending = fixture.replay_pair(RecoveredServeState::Pending);
        let completed = fixture.replay_pair(RecoveredServeState::Completed);
        let negative = fixture.replay_pair(RecoveredServeState::Negative);

        for pair in [&pending, &completed, &negative] {
            assert!(pair.shares_exact_storage_origin());
            assert!(Arc::ptr_eq(&pair.serve.family, &pair.producer.family));
        }
        assert!(matches!(
            pending.serve.payload,
            ReplayPayloadBindingV1::CertifiedServePending { .. }
        ));
        assert!(matches!(
            completed.serve.payload,
            ReplayPayloadBindingV1::CertifiedServeCompleted { .. }
        ));
        assert!(matches!(
            negative.serve.payload,
            ReplayPayloadBindingV1::CertifiedServeNegative {
                outcome: DurableServeNegativeOutcome::Rejected(17),
                ..
            }
        ));
        assert_eq!(
            pending.serve.family.source.request,
            completed.serve.family.source.request
        );
        assert_eq!(
            pending.serve.family.source.request,
            negative.serve.family.source.request
        );
        assert_eq!(
            pending.serve.family.source.local_retainer,
            completed.serve.family.source.local_retainer
        );
        assert_eq!(
            pending.serve.family.source.local_retainer,
            negative.serve.family.source.local_retainer
        );
        assert_ne!(
            pending.serve.family.source.payload_hash, completed.serve.family.source.payload_hash,
            "the exact canonical frame hash binds its completed state"
        );
        assert_ne!(
            pending.serve.family.source.payload_hash, negative.serve.family.source.payload_hash,
            "the exact canonical frame hash binds its negative state"
        );
    }

    #[test]
    fn recovered_prepare_and_commit_votes_build_canonical_attached_evidence() {
        let fixture = Fixture::new();
        let locator = RecoveredWalFrameIdentity::for_test(8, 9, [0xB1; 32]);
        let tag = fixture.recovered_tag();
        for mut vote in [fixture.prepare_vote.clone(), fixture.commit_vote.clone()] {
            vote.signature.clear();
            let evidence =
                RecoveredWalVoteReplayEvidenceV1::from_sealed_recovered_vote(locator, tag, &vote)
                    .expect("production-shaped recovered vote builds canonical evidence");
            assert!(evidence.exactly_matches_recovered_vote(locator, tag, &vote));
            assert_eq!(evidence, evidence.clone());
            let encoded = evidence.authority.encode();
            assert_eq!(
                LifecycleReplayAuthorityV1::decode_canonical(&encoded)
                    .expect("attached evidence remains canonical"),
                evidence.authority
            );
            let LifecycleReplaySourceV1::Wal(source) = &evidence.authority.source else {
                panic!("recovered vote evidence is WAL-backed")
            };
            let expected_role = match vote.phase {
                wire::GlobalPhase::Prepare => ReplayWalRoleV1::PREPARE_INTENT,
                wire::GlobalPhase::Commit => ReplayWalRoleV1::LOCK_AND_COMMIT,
            };
            assert!(source.role.matches(expected_role));
            assert!(source.locator.exactly_matches_runtime(locator));
        }
    }

    #[test]
    fn recovered_vote_evidence_rejects_role_vote_and_frame_hash_substitution() {
        let fixture = Fixture::new();
        let locator = RecoveredWalFrameIdentity::for_test(8, 9, [0xB2; 32]);
        let tag = fixture.recovered_tag();
        let mut vote = fixture.prepare_vote.clone();
        vote.signature.clear();
        let evidence =
            RecoveredWalVoteReplayEvidenceV1::from_sealed_recovered_vote(locator, tag, &vote)
                .expect("Prepare replay evidence fixture");

        let mut wrong_role = evidence.clone();
        let LifecycleReplaySourceV1::Wal(source) = &mut wrong_role.authority.source else {
            panic!("recovered vote evidence is WAL-backed")
        };
        source.role = ReplayWalRoleV1::LOCK_AND_COMMIT;
        assert!(!wrong_role.exactly_matches_recovered_vote(locator, tag, &vote));

        let mut wrong_vote = vote.clone();
        wrong_vote.subject = fixture.conflicting_vote.subject;
        assert!(!evidence.exactly_matches_recovered_vote(locator, tag, &wrong_vote));

        let wrong_hash = RecoveredWalFrameIdentity::for_test(8, 9, [0xB3; 32]);
        assert!(!evidence.exactly_matches_recovered_vote(wrong_hash, tag, &vote));
    }

    #[test]
    fn certified_fetch_store_validate_evidence_retains_one_canonical_origin_and_frame() {
        let fixture = Fixture::new();
        let tag = fixture.recovered_tag();
        let certificate = fixture.prepare_qc.clone();
        let manifest = fixture.proposal.manifest.clone();
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(certificate),
        };
        let responder = KeyPair::random();
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(&fixture.serve_request),
            manifest: manifest.clone(),
            body: vec![0xA1, 0xA2],
            responder: 0,
            signature: Vec::new(),
        };
        response.signature =
            Signature::new(responder.private_key(), &response.signature_preimage())
                .payload()
                .to_vec();
        let receipt = DurableBodyReceipt::for_test(
            manifest.round.context_id,
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let fetch = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
            &fetch_effect,
            &response,
            &receipt,
        )
        .expect("signed certified response builds canonical Fetch evidence");
        assert!(fetch.family.is_exact_all_stages());
        assert!(
            fetch.exactly_matches_signed_response_for_test(&fetch_effect, &response, &receipt,)
        );
        let mut zero_frame = fetch.family.clone();
        zero_frame.body_frame.frame = [0; 32];
        assert!(
            zero_frame.is_exact_all_stages(),
            "body-frame digests have no reserved zero sentinel"
        );

        let store_effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let store = fetch
            .project_store_for_test(&store_effect, &receipt)
            .expect("Fetch evidence projects only its exact Store stage");
        assert!(store.exactly_matches_store(&store_effect, &receipt));

        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let store_pending = pending_binding(&store_effect, tag, 81);
        let validate_pending = store_pending
            .project_store_validate_successor(&store_effect, &validate_effect)
            .expect("Store pending projects one exact Validate root");
        let validate = store
            .project_validate(&store_effect, &receipt, &validate_effect, &validate_pending)
            .expect("Store evidence projects only its exact Validate stage");
        assert!(validate.exactly_matches_validate_pending(
            &validate_effect,
            &receipt,
            &validate_pending,
        ));
        let foreign_pending = pending_binding(&validate_effect, tag, 82);
        assert!(!validate.exactly_matches_validate_pending(
            &validate_effect,
            &receipt,
            &foreign_pending,
        ));
        assert!(validate.exactly_matches_durable_body(&receipt));
        assert_eq!(validate, validate.clone());
        assert_eq!(fetch.family, store.family);
        assert_eq!(store.family, validate.family);
    }

    #[test]
    fn durable_ready_fetch_digest_ignores_transport_retransmission_but_binds_replay_identity() {
        fn projection(
            effect: &AdapterEffect,
            response: &wire::CertifiedBodyResponse,
            receipt: &DurableBodyReceipt,
            causal_root: CausalRoot,
        ) -> DurableCertifiedFetchReplayProjectionV1 {
            let evidence = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
                effect, response, receipt,
            )
            .expect("structurally signed response projects one exact durable family");
            let pending = PendingRuntimeEffectBinding::from_durable_certified_fetch(
                DurableCertifiedFetchPendingMintPermit::new(),
                Hash::prehashed(*causal_root.digest().as_bytes()),
                effect,
            )
            .expect("exact certified Fetch effect mints one frame-bound pending binding");
            evidence
                .project_durable_ready_fetch(effect, &pending, receipt)
                .expect("exact family, pending binding, and receipt project Ready Fetch")
        }

        let fixture = Fixture::new();
        let tag = fixture.recovered_tag();
        let manifest = fixture.proposal.manifest.clone();
        let effect = AdapterEffect::FetchBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(fixture.prepare_qc.clone()),
        };
        let receipt = DurableBodyReceipt::for_test(
            manifest.round.context_id,
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let first_response = wire::CertifiedBodyResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(b"first request occurrence")),
            manifest: manifest.clone(),
            body: vec![0xD1, 0xD2],
            responder: 0,
            signature: vec![0xD3],
        };
        let retransmitted_response = wire::CertifiedBodyResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"different request occurrence",
            )),
            responder: 3,
            signature: vec![0xD4, 0xD5],
            ..first_response.clone()
        };
        assert_ne!(
            HashOf::new(&first_response),
            HashOf::new(&retransmitted_response)
        );
        let causal_root = CausalRoot::new(digest_from_hash(&Hash::new(b"ready Fetch causal root")));
        let first = projection(&effect, &first_response, &receipt, causal_root);
        let retransmitted = projection(&effect, &retransmitted_response, &receipt, causal_root);
        let first_queue_identity =
            super::super::ingress_position::PendingFairIngressIdentity::for_test(
                fixture.context,
                digest_from_hash(&Hash::new(b"first queue occurrence")),
                11,
            );
        let retransmitted_queue_identity =
            super::super::ingress_position::PendingFairIngressIdentity::for_test(
                fixture.context,
                digest_from_hash(&Hash::new(b"second queue occurrence")),
                12,
            );
        assert_ne!(first_queue_identity, retransmitted_queue_identity);
        assert_eq!(
            first.completion_digest(),
            retransmitted.completion_digest(),
            "request, response, responder, signature, and physical queue occurrence are not restart identity",
        );

        let foreign_causal = projection(
            &effect,
            &first_response,
            &receipt,
            CausalRoot::new(digest_from_hash(&Hash::new(b"foreign Fetch causal root"))),
        );
        assert_ne!(
            first.completion_digest(),
            foreign_causal.completion_digest()
        );

        let foreign_effect_identity = Hash::new(b"foreign exact Fetch effect identity");
        assert_ne!(
            first.completion_digest(),
            canonical_durable_certified_fetch_completion_digest(
                first.causal_key,
                foreign_effect_identity,
                &first.authority,
            )
        );

        let mut foreign_qc_authority = first.authority.clone();
        let LifecycleReplaySourceV1::BodyPipeline(source) = &mut foreign_qc_authority.source else {
            panic!("durable Ready Fetch authority is body-pipeline backed")
        };
        let BodyPipelineOriginV1::Certified { certificate, .. } = &mut source.origin else {
            panic!("durable Ready Fetch authority is certified")
        };
        certificate.aggregate_signature[0] ^= 1;
        assert_ne!(
            first.completion_digest(),
            canonical_durable_certified_fetch_completion_digest(
                first.causal_key,
                first.effect_identity,
                &foreign_qc_authority,
            )
        );

        let manifest_absent_effect = AdapterEffect::FetchBody {
            manifest: None,
            ..effect.clone()
        };
        let manifest_absent = projection(
            &manifest_absent_effect,
            &first_response,
            &receipt,
            causal_root,
        );
        assert_ne!(
            first.completion_digest(),
            manifest_absent.completion_digest()
        );

        let source_key = KeyPair::try_from_seed(vec![0xD6; 32], Algorithm::Ed25519)
            .expect("deterministic certified-source identity");
        let foreign_sources_effect = AdapterEffect::FetchBody {
            certified_sources: vec![PeerId::new(source_key.public_key().clone())],
            ..effect.clone()
        };
        let foreign_sources = projection(
            &foreign_sources_effect,
            &first_response,
            &receipt,
            causal_root,
        );
        assert_ne!(
            first.completion_digest(),
            foreign_sources.completion_digest()
        );

        let mut foreign_frame_authority = first.authority.clone();
        let ReplayPayloadBindingV1::BodyFrame(frame) = &mut foreign_frame_authority.payload else {
            panic!("durable Ready Fetch authority is frame-bound")
        };
        frame.frame[0] ^= 1;
        assert_ne!(
            first.completion_digest(),
            canonical_durable_certified_fetch_completion_digest(
                first.causal_key,
                first.effect_identity,
                &foreign_frame_authority,
            )
        );
    }

    #[test]
    fn certified_pipeline_evidence_rejects_certificate_manifest_frame_and_stage_substitution() {
        let fixture = Fixture::new();
        let tag = fixture.recovered_tag();
        let manifest = fixture.proposal.manifest.clone();
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(fixture.prepare_qc.clone()),
        };
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(&fixture.serve_request),
            manifest: manifest.clone(),
            body: vec![0xB1],
            responder: 0,
            signature: vec![0xB2],
        };
        let receipt = DurableBodyReceipt::for_test(
            manifest.round.context_id,
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let fetch = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
            &fetch_effect,
            &response,
            &receipt,
        )
        .expect("certified substitution fixture");

        let mut wrong_certificate = fetch.clone();
        let BodyPipelineOriginV1::Certified { certificate, .. } =
            &mut wrong_certificate.family.source.origin
        else {
            panic!("certified fixture retains its QC")
        };
        certificate.aggregate_signature[0] ^= 1;
        assert!(!wrong_certificate.exactly_matches_signed_response_for_test(
            &fetch_effect,
            &response,
            &receipt,
        ));

        response.manifest.chunk_root = Hash::new(b"substituted response manifest");
        assert!(!fetch.exactly_matches_signed_response_for_test(
            &fetch_effect,
            &response,
            &receipt,
        ));

        let mut wrong_frame = fetch.clone();
        wrong_frame.family.body_frame.frame[0] ^= 1;
        assert!(!wrong_frame.exactly_matches_signed_response_for_test(
            &fetch_effect,
            &wire::CertifiedBodyResponse {
                manifest,
                ..response.clone()
            },
            &receipt,
        ));

        let store_effect = AdapterEffect::StoreBody {
            tag,
            round: receipt.round(),
            subject: receipt.subject(),
        };
        let store = fetch
            .project_store_for_test(&store_effect, &receipt)
            .expect("exact Store stage fixture");
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round: receipt.round(),
            subject: receipt.subject(),
        };
        assert!(!store.exactly_matches_store(&validate_effect, &receipt));
        let store_pending = pending_binding(&store_effect, tag, 83);
        let validate_pending = store_pending
            .project_store_validate_successor(&store_effect, &validate_effect)
            .expect("Store pending projects one exact Validate root");
        let validate = store
            .project_validate(&store_effect, &receipt, &validate_effect, &validate_pending)
            .expect("exact Validate stage fixture");
        assert!(!validate.exactly_matches_validate_pending(
            &store_effect,
            &receipt,
            &validate_pending,
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn local_body_pre_intent_seal_rejects_owner_manifest_frame_and_stage_substitution() {
        let fixture = Fixture::new();
        let tag = fixture.recovered_tag();
        let manifest = fixture.proposal.manifest.clone();
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let store_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&store_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 70)],
        )
        .expect("bind exact local Store owner")
        .pop()
        .expect("one local Store owner");
        let store_pending = store_ownership
            .pending_adapter_effect_binding(&store_effect)
            .expect("local Store owner projects one pending seal");
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let validate_pending = store_pending
            .project_store_validate_successor(&store_effect, &validate_effect)
            .expect("local Store owner projects one Validate successor");
        let receipt = DurableBodyReceipt::for_test(
            manifest.round.context_id,
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let seal =
            LocalBodyPreIntentReplaySealV1::for_test(&store_effect, store_pending, &manifest)
                .expect("mint test-only local pre-intent seal");
        assert!(seal.exactly_projects_validate(
            &store_effect,
            &manifest,
            &receipt,
            &validate_effect,
            &validate_pending,
        ));

        let foreign_pending = pending_binding(&validate_effect, tag, 71);
        assert!(!seal.exactly_projects_validate(
            &store_effect,
            &manifest,
            &receipt,
            &validate_effect,
            &foreign_pending,
        ));
        let seal = seal
            .bind_and_project_validate(
                &store_effect,
                &manifest,
                &receipt,
                &validate_effect,
                &foreign_pending,
            )
            .expect_err("foreign owner returns the original move-only seal");
        let mut foreign_manifest = manifest.clone();
        foreign_manifest.chunk_root = Hash::new(b"foreign local replay manifest");
        let foreign_receipt = DurableBodyReceipt::for_test(
            manifest.round.context_id,
            manifest.round,
            manifest.subject,
            HashOf::new(&foreign_manifest),
        );
        assert!(!seal.exactly_projects_validate(
            &store_effect,
            &manifest,
            &foreign_receipt,
            &validate_effect,
            &validate_pending,
        ));

        let mut validate = seal
            .bind_and_project_validate(
                &store_effect,
                &manifest,
                &receipt,
                &validate_effect,
                &validate_pending,
            )
            .expect("exact local durability joins Validate replay evidence");
        assert!(validate.exactly_matches_validate(&validate_effect, &receipt));
        validate.family.body_frame.frame[0] ^= 1;
        assert!(!validate.exactly_matches_validate(&validate_effect, &receipt));
        validate.family.body_frame.frame = [0; 32];
        assert!(
            validate
                .family
                .is_exact_for_stage(LifecycleStageKind::ValidateBody),
            "zero-valued digest bytes remain structurally valid rather than sentinel values"
        );
        assert!(!validate.exactly_matches_validate(&store_effect, &receipt));

        let validate_ownership = store_ownership
            .rebind_as_inherited_adapter_effect(&validate_effect)
            .expect("local Store root rebinds to its exact Validate effect");
        let second_store_pending = store_ownership
            .pending_adapter_effect_binding(&store_effect)
            .expect("local Store root retains its exact pending projection");
        let second_validate_pending = validate_ownership
            .pending_adapter_effect_binding(&validate_effect)
            .expect("local Validate root retains its exact pending projection");
        let exact_validate = LocalBodyPreIntentReplaySealV1::for_test(
            &store_effect,
            second_store_pending,
            &manifest,
        )
        .expect("remint an independent test-only local seal")
        .bind_and_project_validate(
            &store_effect,
            &manifest,
            &receipt,
            &validate_effect,
            &second_validate_pending,
        )
        .expect("exact local Store evidence advances to Validate");
        let validated_receipt = ValidatedBodyReceipt::for_test(receipt.clone());
        let command_identity = LocalProposalReadyCommandIdentity::from_exact_handoff(
            tag,
            &manifest,
            &receipt,
            &validated_receipt,
            &validate_ownership,
        )
        .expect("exact Validate completion has one inert command identity");
        let ready = exact_validate
            .complete_local_proposal(
                &validate_effect,
                &manifest,
                validated_receipt,
                command_identity,
            )
            .expect("exact Validate completion retains local replay evidence");
        let mut unsigned_proposal = fixture.proposal.clone();
        unsigned_proposal.signature.clear();
        let proposal_intent = AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(unsigned_proposal),
        };
        let proposal_ownership = validate_ownership
            .rebind_as_inherited_adapter_effect(&proposal_intent)
            .expect("local Validate root rebinds to exact ProposalIntent");
        let foreign_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&proposal_intent),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 72)],
        )
        .expect("bind foreign ProposalIntent owner")
        .pop()
        .expect("one foreign ProposalIntent owner");
        assert!(!ready.exactly_matches_proposal_intent(
            command_identity,
            &proposal_intent,
            &foreign_ownership,
        ));
        let intent = ready
            .bind_proposal_intent(command_identity, &proposal_intent, &proposal_ownership)
            .expect("exact command consumes into one inseparable ProposalIntent composite");
        assert!(intent.exactly_matches_proposal_intent(
            command_identity,
            &proposal_intent,
            &proposal_ownership,
        ));
        drop(intent);
    }

    #[test]
    fn local_body_replay_authority_is_linear_nondecode_and_closed_to_fixed_joins() {
        let source = include_str!("v2_lifecycle_replay_authority.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("replay authority has one production prefix");
        let local = production
            .split("pub(in crate::sumeragi) struct LocalBodyPreIntentReplaySealV1")
            .nth(1)
            .expect("local replay seal has one declaration")
            .split(
                "/// Selector-authenticated origin awaiting one exact durable body-frame binding.",
            )
            .next()
            .expect("certified replay evidence follows local replay authority");
        for required in [
            "store_pending: PendingRuntimeEffectBinding",
            "pub(in crate::sumeragi) struct LocalValidateReplayEvidenceV1",
            "pub(in crate::sumeragi) struct LocalProposalReadyReplayEvidenceV1",
            "pub(in crate::sumeragi) struct LocalProposalIntentReplayEvidenceV1",
            "fn bind_and_project_validate(",
            "project_store_validate_successor(store_effect, validate_effect)",
            "fn complete_local_proposal(",
            "command_identity: LocalProposalReadyCommandIdentity",
            "exactly_matches_proposal_intent(",
            "exactly_matches_proposal_intent_effect(",
            "fn bind_proposal_intent(",
            "BodyPipelineOriginV1::LocalBody(manifest.clone())",
        ] {
            assert!(
                local.contains(required),
                "local replay authority omitted {required}"
            );
        }
        for forbidden in [
            "#[derive(Clone",
            "#[derive(Copy",
            "Decode",
            "pub(in crate::sumeragi) fn source(",
            "pub(in crate::sumeragi) fn receipt(",
            "pub(in crate::sumeragi) fn pending(",
            "pub(in crate::sumeragi) fn manifest(",
            "pub(in crate::sumeragi) fn into_parts(",
            "Arc<LocalBodyPreIntentReplaySealV1>",
            "Arc<LocalValidateReplayEvidenceV1>",
            "Arc<LocalProposalIntentReplayEvidenceV1>",
            "!= [0; 32]",
            "== [0; 32]",
            "is_zero()",
        ] {
            assert!(
                !local.contains(forbidden),
                "local replay authority exposed or reserved {forbidden}"
            );
        }

        let runtime = include_str!("v2_runtime.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("runtime has one production prefix");
        let executor = include_str!("v2_effects.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("effect executor has one production prefix");
        assert_eq!(
            runtime.matches("LocalBodyReplayMintPermit::new()").count(),
            1
        );
        assert_eq!(
            runtime
                .matches("LocalProposalEffectOwnership::from_exact_assemble_body(")
                .count(),
            2,
            "only the active-view and fresh local producer branches mint the composite"
        );
        for required in [
            "local_store_replay: BTreeMap<EffectWorkId, LocalProposalEffectOwnership>",
            "local_validate_replay: BTreeMap<EffectWorkId, LocalValidateReplayEvidenceV1>",
            "BTreeMap<LocalProposalReadyCommandIdentity, LocalProposalReadyReplayEvidenceV1>",
            "BTreeMap<LocalProposalReadyCommandIdentity, LocalProposalIntentReplayEvidenceV1>",
            ".project_exact_validate(",
            ".complete_local_proposal(",
            ".bind_proposal_intent(",
            "plan_local_proposal_replay_consumptions(",
            "retire_local_proposal_ready_replay(",
        ] {
            assert!(
                executor.contains(required),
                "executor omitted local replay cut {required}"
            );
        }
        assert!(!production.contains("BodyPipelineOriginV1::ProposalIntent"));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn certified_serve_replay_pair_is_opaque_exact_and_fixed_admission_only() {
        let source = include_str!("v2_lifecycle_replay_authority.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("replay authority has one production prefix");
        let evidence = production
            .split("struct CertifiedServeStorageReplayFamilyV1 {")
            .nth(1)
            .expect("Certified-Serve replay family has one declaration")
            .split("struct CertifiedBodyPipelineReplayFamilyV1 {")
            .next()
            .expect("certified body replay family follows Serve evidence");
        for required in [
            "source: CertifiedServeStorageSourceV1",
            "struct CertifiedServeReplayEvidenceV1",
            "struct CertifiedServeProducerTurnReplayEvidenceV1",
            "pub(super) struct CertifiedServeReplayEvidencePairV1",
            "family: Arc<CertifiedServeStorageReplayFamilyV1>",
            "pub(super) fn from_post_fsync_pending(",
            "receipt.exactly_matches_pending(authenticated)",
            "pub(super) fn from_authenticated_recovery(",
            "recovered.exactly_matches_persisted_payload()",
            "recovered.local_retainer()",
            "binary_search(&local_retainer)",
            "pub(super) fn exactly_matches_serve_record(",
            "pub(super) fn exactly_matches_terminal_serve_record(",
            "pub(super) fn exactly_matches_producer_record(",
            "pub(super) fn admission_candidate(",
            "pub(super) fn exactly_matches_serve_carrier(",
            "pub(super) fn exactly_matches_producer_carrier(",
            "Some(CandidateAdmission::new(",
            "Arc::ptr_eq(&self.serve.family, &self.producer.family)",
            "LifecycleStageKind::CertifiedServe",
            "LifecycleStageKind::ProducerTurn",
            "producer_turn_key_for_serve(serve.key)",
        ] {
            assert!(
                evidence.contains(required),
                "Certified-Serve replay evidence omitted {required}"
            );
        }
        for runtime_seal in [
            "CertifiedServeStorageReplayFamilyV1",
            "CertifiedServeReplayEvidenceV1",
            "CertifiedServeProducerTurnReplayEvidenceV1",
            "CertifiedServeReplayEvidencePairV1",
        ] {
            let derive = production
                .split(runtime_seal)
                .next()
                .expect("Serve runtime seal has a declaration prefix")
                .rsplit("#[derive(")
                .next()
                .expect("Serve runtime seal derive is inspectable")
                .split(")]")
                .next()
                .expect("Serve runtime seal derive is bounded");
            assert!(
                !derive.contains("Decode") && !derive.contains("Encode"),
                "Serve runtime seal {runtime_seal} became codec-constructible"
            );
        }
        for forbidden in [
            "pub(super) fn from_parts(",
            "pub(super) fn into_parts(",
            "pub(super) fn source(",
            "pub(super) fn request(",
            "pub(super) fn certificate(",
            "pub(super) fn payload_hash(",
            "pub(super) fn local_retainer(",
            "pub(super) fn encoded(",
            "pub(super) fn authority(",
            "pub(super) fn serve(",
            "pub(super) fn producer(",
            "into_concrete_evidence",
            "pub(super) fn into_admission(",
            "impl Drop for CertifiedServe",
        ] {
            assert!(
                !evidence.contains(forbidden),
                "Certified-Serve replay evidence exposed {forbidden}"
            );
        }

        let storage = production
            .split("struct CertifiedServeStorageSourceV1 {")
            .nth(1)
            .expect("Certified-Serve storage source has one declaration")
            .split("enum ReplayPayloadBindingV1 {")
            .next()
            .expect("replay payload binding follows Serve storage source");
        for required in [
            "local_retainer >= wire::MAX_VALIDATORS_PER_HEIGHT",
            ".binary_search(&self.local_retainer)",
            "LifecycleStageKind::CertifiedServe => LifecyclePhase::Serve",
            "LifecycleStageKind::ProducerTurn => LifecyclePhase::ProducerTurn",
        ] {
            assert!(
                storage.contains(required),
                "canonical Certified-Serve source omitted {required}"
            );
        }

        let payload_store = include_str!("v2_certified_serve_payload_store.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("payload store has one production prefix");
        for required in [
            "local_retainer: wire::ValidatorIndex",
            "pub(crate) const fn local_retainer(&self)",
            "pub(crate) fn exactly_matches_persisted_payload(&self)",
            "local_retainer,\n                state",
        ] {
            assert!(
                payload_store.contains(required),
                "authenticated payload recovery omitted {required}"
            );
        }

        for outside in [
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_lifecycle_ledger.rs"),
            include_str!("v2.rs"),
            include_str!("v2_runtime.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_runner.rs"),
        ] {
            let outside = outside
                .split("\n#[cfg(test)]\nmod tests {")
                .next()
                .expect("outside production prefix is bounded");
            assert!(!outside.contains("CertifiedServeReplayEvidencePairV1"));
            assert!(!outside.contains("CertifiedServeReplayEvidenceV1"));
            assert!(!outside.contains("CertifiedServeProducerTurnReplayEvidenceV1"));
        }
        let registry = include_str!("v2_lifecycle_work_registry.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry production prefix is bounded");
        for required in [
            "CertifiedServeReplayEvidencePairV1",
            "Arc<CertifiedServeReplayEvidencePairV1>",
            "Arc::clone(&replay_evidence)",
            "exactly_matches_serve_carrier(",
            "exactly_matches_terminal_serve_record(",
            "exactly_matches_producer_carrier(",
        ] {
            assert!(
                registry.contains(required),
                "whole-pair registry ownership omitted {required}"
            );
        }
        for forbidden in [
            "CertifiedServeReplayEvidenceV1,",
            "CertifiedServeProducerTurnReplayEvidenceV1,",
            "into_concrete_evidence",
        ] {
            assert!(
                !registry.contains(forbidden),
                "registry decomposed Certified-Serve replay authority through {forbidden}"
            );
        }
        let projection = include_str!("v2_lifecycle_projection.rs")
            .split("\n#[cfg(test)]\nmod wait_source_tests {")
            .next()
            .expect("projection production prefix is bounded");
        for required in [
            "CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(",
            "CertifiedServeReplayEvidencePairV1::from_authenticated_recovery(",
            "replay\n        .admission_candidate(active_context)",
            "impl super::ProductionLifecycleOwnerV1",
            "fn admit_selected_certified_serve(",
            "target: super::LifecycleIngressIoTargetSeal",
            "target.matches_certified_serve_request(authenticated.request_hash())",
            "prepare_certified_serve_admission(",
            ".install_certified_serve_fresh_batch_before_publication(",
            "|| self.coordinator.persist_exact_staged_successor(&staged)",
            "fn certified_serve_terminal_replay_decision(",
            ".exactly_matches_certified_serve_publication(authenticated, publication.receipt())",
            "CertifiedServeConcreteAdmissionV1::restart_required(",
            "fn into_safe_continuation(",
            "fn settle_certified_serve_completed(",
            "fn settle_certified_serve_negative(",
            ".persist_completed_with_exact_body(",
            ".persist_negative_for_authenticated_request(authenticated, outcome)",
            "fn preflight_certified_serve_terminal(",
            "fn publish_certified_serve_terminal(",
            "CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(",
            "CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(",
            ".prepare_certified_serve_terminal_transition(",
            ".publish_certified_serve_terminal_transition(",
        ] {
            assert!(
                projection.contains(required),
                "fixed Certified-Serve admission omitted {required}"
            );
        }
        for cfg_test_only in [
            "#[cfg(test)]\n    pub(crate) fn persist_and_admit_certified_serve(",
            "#[cfg(test)]\npub(super) fn certified_serve_admission_request(",
        ] {
            assert!(
                projection.contains(cfg_test_only),
                "raw Certified-Serve surface escaped its fixture gate: {cfg_test_only}"
            );
        }
        let settlement = include_str!("v2_lifecycle_settlement.rs");
        assert!(
            settlement.contains(
                "#[cfg(test)]\n    pub(super) fn settle_turn_with_durable_serve_terminal("
            )
        );
        assert_eq!(
            settlement
                .matches("fn settle_turn_with_durable_serve_terminal(")
                .count(),
            1,
            "the raw terminal reducer wrapper must remain a single test fixture"
        );
        for outside in [
            include_str!("v2.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_lifecycle_ledger.rs"),
            include_str!("v2_lifecycle_open.rs"),
            include_str!("v2_lifecycle_settlement.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            let outside = outside
                .split("\n#[cfg(test)]\nmod tests {")
                .next()
                .expect("outside terminal production prefix is bounded");
            assert!(!outside.contains(".persist_completed_with_exact_body("));
            assert!(!outside.contains(".publish_certified_serve_terminal_transition("));
        }
        let coordinator = include_str!("v2_lifecycle_coordinator.rs");
        assert!(coordinator.contains("#[cfg(test)]\n    pub(super) fn admit_certified_serve("));
        assert!(coordinator.contains(
            "CertifiedServeTerminalSettlementErrorV1, CertifiedServeTerminalSettlementFailureV1,"
        ));
        assert!(!projection.contains("CertifiedServeSettlementError"));
        assert_eq!(
            projection
                .matches("fn settle_certified_serve_completed(")
                .count(),
            1,
            "only the production owner may expose completed Serve settlement"
        );
        assert_eq!(
            projection
                .matches("fn settle_certified_serve_negative(")
                .count(),
            1,
            "only the production owner may expose negative Serve settlement"
        );
        for terminal_method in [
            "settle_certified_serve_completed",
            "settle_certified_serve_negative",
        ] {
            let signature_marker = format!("fn {terminal_method}(");
            let signature = projection
                .split(signature_marker.as_str())
                .nth(1)
                .expect("terminal owner transaction has one signature")
                .split(") -> Result<(), CertifiedServeTerminalSettlementErrorV1>")
                .next()
                .expect("terminal owner signature is bounded");
            for forbidden in [
                "receipt:",
                "payload_id",
                "CandidateAdmission",
                "ordinal:",
                "digest:",
                "ReplayEvidence",
                "parts",
                "route",
                "effect",
                "pending",
            ] {
                assert!(
                    !signature.contains(forbidden),
                    "terminal owner transaction accepted raw {forbidden}"
                );
            }
        }
        let fresh_signature = projection
            .split("fn admit_selected_certified_serve(")
            .nth(1)
            .expect("fresh Serve owner transaction has one signature")
            .split(") -> CertifiedServeConcreteAdmissionV1")
            .next()
            .expect("fresh Serve owner signature is bounded");
        for forbidden in [
            "CandidateAdmission",
            "AdapterEffect",
            "PendingRuntimeEffectBinding",
            "ReplayEvidence",
            "ordinal:",
            "digest:",
            "route",
            "queue",
        ] {
            assert!(
                !fresh_signature.contains(forbidden),
                "fresh Serve owner transaction accepted raw {forbidden}"
            );
        }
        assert!(projection.contains("struct CertifiedServeConcreteAdmissionV1 {"));
        assert!(!projection.contains("pub(crate) enum CertifiedServeConcreteAdmissionV1"));
        let opaque_result = projection
            .split("impl CertifiedServeConcreteAdmissionV1 {")
            .nth(1)
            .expect("opaque Serve outcome has one implementation")
            .split("impl CertifiedServeConcreteAdmissionContinuationV1 {")
            .next()
            .expect("safe continuation follows opaque Serve outcome");
        assert!(!opaque_result.contains("fn into_target("));

        for required in [
            "impl Drop for DurableCertifiedServeAdmissionPublication",
            "fresh_pending: bool",
            "fn can_abort_fresh_pending(&self)",
            "fn exactly_matches_authenticated_request(",
            "enum CertifiedServePayloadRetentionError",
            "PublicationAmbiguous(CertifiedServePayloadStoreError)",
            "PublishedButUnsynchronized(CertifiedServePayloadStoreError)",
            "fn persist_completed_with_exact_body(",
            "body_store.owns_receipt(durable_body)",
            "fn persist_negative_for_authenticated_request(",
            "#[cfg(test)]\n    pub(crate) fn persist_negative(",
        ] {
            assert!(
                payload_store.contains(required),
                "payload-first Serve ownership omitted {required}"
            );
        }
        for required in [
            "fn exactly_matches_fresh_staged_append(",
            "exactly_covers_recovered_ready_work(current)",
            "exactly_covers_recovered_ready_work_and_wal_authority(current)",
            "carrier.matches_record(record, metadata, work.digest)",
            "fn matches_current_ready_record(",
            "fn exact_optional_recovered_wal_authority(",
            "fn preflight_certified_serve_terminal_owner_state(",
            "fn preflights_exact_staged_successor(",
            "staged.high_water == current.high_water",
            "staged.producer_debts == expected_debts",
            "staged.capacity_used == expected_capacity_used",
            "fn publish_certified_serve_terminal_transition",
        ] {
            assert!(
                registry.contains(required),
                "fresh Serve whole-census preflight omitted {required}"
            );
        }

        let authority = include_str!("v2_lifecycle_authority.rs");
        assert!(authority.contains(
            "#[cfg(test)]\n#[derive(Clone, Debug, PartialEq, Eq)]\npub(crate) struct RolloverSnapshot"
        ));
        assert!(!authority.contains("fn verified_successor("));
        assert!(coordinator.contains("#[cfg(test)]\npub(crate) use authority::RolloverSnapshot;"));
        let lifecycle_open = include_str!("v2_lifecycle_open.rs");
        for test_only_rollover in [
            "#[cfg(test)]\n    pub(crate) fn rollover(",
            "#[cfg(test)]\n    pub(crate) fn rollover_with_payload_store(",
            "#[cfg(test)]\n    fn rollover_inner(",
            "#[cfg(test)]\n    fn serve_cancellation_receipts_are_exact(",
            "#[cfg(test)]\n    fn retire_for_rollover(",
        ] {
            assert!(
                lifecycle_open.contains(test_only_rollover),
                "raw terminal-receipt rollover escaped its test gate: {test_only_rollover}"
            );
        }
    }

    #[test]
    fn certified_pipeline_replay_evidence_is_normalized_inert_and_stage_fixed() {
        let source = include_str!("v2_lifecycle_replay_authority.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("replay authority has one production prefix");
        let evidence = production
            .split("pub(super) struct AuthenticatedCertifiedFetchReplayOriginV1")
            .nth(1)
            .expect("certified Fetch replay origin has one declaration")
            .split("fn exact_recovered_wal_vote_authority(")
            .next()
            .expect("recovered WAL authority follows certified body evidence");
        for required in [
            "from_completion_authority(\n        authority: &CertifiedFetchCompletionAuthority<'_>",
            "candidate_pending()\n                .exactly_binds_adapter_effect(effect)",
            "pub(super) fn bind_durable_body(",
            "pub(super) struct CertifiedFetchReplayEvidenceV1",
            "pub(super) struct CertifiedStoreReplayEvidenceV1",
            "pub(in crate::sumeragi) struct CertifiedValidateReplayEvidenceV1",
            "validate_pending: DirectSignedPendingBindingV1",
            "pub(super) fn project_store(",
            "pub(super) fn project_validate(",
            "validate_pending: &PendingRuntimeEffectBinding",
            "fn exactly_matches_validate_pending(",
            "fn is_exact_for_stage(&self, stage: LifecycleStageKind)",
            "LifecycleStageKind::FetchBody\n            | LifecycleStageKind::StoreBody",
            "ReplayPayloadBindingV1::BodyFrame(self.body_frame)",
            "family.is_exact_for_stage(stage)",
        ] {
            assert!(
                evidence.contains(required),
                "certified body replay evidence omitted {required}"
            );
        }

        let family = evidence
            .split("struct CertifiedBodyPipelineReplayFamilyV1 {")
            .nth(1)
            .expect("certified body replay family has one declaration")
            .split('}')
            .next()
            .expect("certified body replay family declaration is bounded");
        assert!(family.contains("source: BodyPipelineReplaySourceV1"));
        assert!(family.contains("body_frame: BodyFrameBindingV1"));
        assert_eq!(family.lines().filter(|line| line.contains(':')).count(), 2);
        assert!(!family.contains("LifecycleReplayAuthorityV1"));

        for runtime_seal in [
            "AuthenticatedCertifiedFetchReplayOriginV1",
            "CertifiedFetchReplayEvidenceV1",
            "CertifiedStoreReplayEvidenceV1",
            "CertifiedValidateReplayEvidenceV1",
            "CertifiedBodyPipelineReplayFamilyV1",
        ] {
            let derive = production
                .split(runtime_seal)
                .next()
                .expect("runtime seal has a declaration prefix")
                .rsplit("#[derive(")
                .next()
                .expect("runtime seal derive is inspectable")
                .split(")]")
                .next()
                .expect("runtime seal derive is bounded");
            assert!(
                !derive.contains("Decode") && !derive.contains("Encode"),
                "runtime seal {runtime_seal} became codec-constructible"
            );
        }
        for forbidden in [
            "pub(crate) struct CertifiedFetchReplayEvidenceV1",
            "pub(crate) struct CertifiedStoreReplayEvidenceV1",
            "pub(crate) struct CertifiedValidateReplayEvidenceV1",
            "pub(super) fn encoded(",
            "pub(super) fn into_parts(",
            "pub(super) fn from_parts(",
            "pub(super) fn certificate(",
            "pub(super) fn manifest(",
            "pub(super) fn receipt(",
            "pub(super) fn body_frame(",
            "[0_u8; 32]",
            "!= [0; 32]",
            "== [0; 32]",
        ] {
            assert!(
                !evidence.contains(forbidden),
                "certified body replay evidence exposed or reserved {forbidden}"
            );
        }
        assert!(
            evidence.contains("#[cfg(test)]\n    pub(super) fn from_signed_response_for_test(")
        );
        assert_eq!(
            production
                .matches("#[cfg(test)]\n    pub(super) fn project_candidate_for_test(")
                .count(),
            3,
            "body-transition candidate helpers must remain test-only"
        );
        for helper in [
            "exact_live_wal_body_successor_candidate_for_test",
            "exact_invalid_body_report_candidate_for_test",
        ] {
            assert!(
                production.contains(&format!("#[cfg(test)]\npub(super) fn {helper}(")),
                "transition fixture helper {helper} lost its test-only gate"
            );
        }

        for caller in [
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_lifecycle_ledger.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller.contains("CertifiedFetchReplayEvidenceV1"));
            assert!(!caller.contains("CertifiedStoreReplayEvidenceV1"));
            assert!(!caller.contains("CertifiedValidateReplayEvidenceV1"));
        }
    }

    #[test]
    fn direct_signed_broadcast_evidence_covers_all_seven_fixed_stages() {
        let fixture = Fixture::new();
        let effects = signed_broadcast_effects(&fixture);
        assert_eq!(effects.len(), 7);
        for (ordinal, effect) in (1_u128..).zip(effects) {
            let pending = pending_binding(&effect, fixture.recovered_tag(), ordinal);
            let evidence = SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &pending)
                .expect("signed broadcast has one canonical replay envelope");
            assert!(evidence.exactly_matches_effect(&effect, &pending));
        }

        let zero_digest_binding = DirectSignedPendingBindingV1 {
            causal_lifecycle_key: [0; 32],
            effect_identity: [0; 32],
        };
        assert_eq!(zero_digest_binding.causal_lifecycle_key, [0; 32]);
        assert_eq!(zero_digest_binding.effect_identity, [0; 32]);
    }

    #[test]
    fn direct_signed_broadcast_evidence_rejects_signature_message_and_pending_substitution() {
        let fixture = Fixture::new();
        let mut effects = signed_broadcast_effects(&fixture);
        let effect = effects.remove(0);
        let pending = pending_binding(&effect, fixture.recovered_tag(), 11);
        let evidence = SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &pending)
            .expect("signed proposal broadcast replay evidence");

        let AdapterEffect::Broadcast(message) = &effect else {
            unreachable!("first signed broadcast fixture is a Proposal")
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            unreachable!("first signed broadcast fixture is a Proposal")
        };
        let mut re_signed = proposal.clone();
        re_signed.signature = vec![0xD1];
        let re_signed = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(re_signed),
        ));
        let re_signed_pending = pending_binding(&re_signed, fixture.recovered_tag(), 12);
        assert!(!evidence.exactly_matches_effect(&re_signed, &re_signed_pending));

        let substituted = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(fixture.conflicting_proposal.clone()),
        ));
        let substituted_pending = pending_binding(&substituted, fixture.recovered_tag(), 13);
        assert!(!evidence.exactly_matches_effect(&substituted, &substituted_pending));
        assert!(
            SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &substituted_pending)
                .is_none()
        );

        let foreign_tag = EventTag::new(
            fixture.recovered_tag().height(),
            fixture.recovered_tag().view() + 1,
            Generation::new(9),
        );
        let foreign_pending = pending_binding(&effect, foreign_tag, 14);
        assert!(!evidence.exactly_matches_effect(&effect, &foreign_pending));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn direct_signed_equivocation_evidence_covers_all_three_fixed_pairs() {
        let fixture = Fixture::new();
        let effects = vec![
            AdapterEffect::ReportEquivocation {
                evidence: AdapterEquivocationEvidence::proposal_for_test(
                    fixture.proposal.clone(),
                    fixture.conflicting_proposal.clone(),
                ),
            },
            AdapterEffect::ReportEquivocation {
                evidence: AdapterEquivocationEvidence::vote_for_test(
                    fixture.prepare_vote.clone(),
                    fixture.conflicting_vote.clone(),
                ),
            },
            AdapterEffect::ReportEquivocation {
                evidence: AdapterEquivocationEvidence::timeout_vote_for_test(
                    fixture.timeout_vote.clone(),
                    fixture.conflicting_timeout_vote.clone(),
                ),
            },
        ];
        assert_eq!(effects.len(), 3);
        for (ordinal, effect) in (21_u128..).zip(effects) {
            let pending = pending_binding(&effect, fixture.recovered_tag(), ordinal);
            let evidence = SignedEquivocationReplayEvidenceV1::from_exact_effect(&effect, &pending)
                .expect("authenticated conflict has one canonical replay envelope");
            assert!(evidence.exactly_matches_effect(&effect, &pending));
        }
    }

    #[test]
    fn direct_signed_equivocation_evidence_rejects_pair_order_signature_and_pending_drift() {
        let fixture = Fixture::new();
        let forward = AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::vote_for_test(
                fixture.prepare_vote.clone(),
                fixture.conflicting_vote.clone(),
            ),
        };
        let pending = pending_binding(&forward, fixture.recovered_tag(), 31);
        let evidence = SignedEquivocationReplayEvidenceV1::from_exact_effect(&forward, &pending)
            .expect("authenticated vote conflict replay evidence");

        let reversed = AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::vote_for_test(
                fixture.conflicting_vote.clone(),
                fixture.prepare_vote.clone(),
            ),
        };
        let reversed_pending = pending_binding(&reversed, fixture.recovered_tag(), 32);
        assert!(!evidence.exactly_matches_effect(&reversed, &reversed_pending));

        let mut re_signed = fixture.prepare_vote.clone();
        re_signed.signature = vec![0xD2];
        let re_signed = AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::vote_for_test(
                re_signed,
                fixture.conflicting_vote.clone(),
            ),
        };
        let re_signed_pending = pending_binding(&re_signed, fixture.recovered_tag(), 33);
        assert!(!evidence.exactly_matches_effect(&re_signed, &re_signed_pending));
        assert!(
            SignedEquivocationReplayEvidenceV1::from_exact_effect(&forward, &re_signed_pending)
                .is_none()
        );

        let foreign_tag = EventTag::new(
            fixture.recovered_tag().height(),
            fixture.recovered_tag().view() + 1,
            Generation::new(10),
        );
        let foreign_pending = pending_binding(&forward, foreign_tag, 34);
        assert!(!evidence.exactly_matches_effect(&forward, &foreign_pending));
    }

    #[test]
    fn direct_signed_replay_wrappers_are_opaque_nondecodable_and_fixed_class() {
        let source = include_str!("v2_lifecycle_replay_authority.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("replay authority has one production prefix");
        let direct = production
            .split("pub(super) struct SignedBroadcastReplayEvidenceV1")
            .nth(1)
            .expect("signed Broadcast wrapper has one declaration")
            .split(
                "/// Selector-authenticated origin awaiting one exact durable body-frame binding.",
            )
            .next()
            .expect("certified body replay follows direct signed evidence");
        for required in [
            "pub(super) struct SignedEquivocationReplayEvidenceV1",
            "pending: DirectSignedPendingBindingV1",
            "causal_lifecycle_key: [u8; 32]",
            "effect_identity: [u8; 32]",
            "pub(super) fn from_exact_effect(\n        effect: &AdapterEffect,\n        pending: &PendingRuntimeEffectBinding",
            "pub(super) fn exactly_matches_effect(",
            "pending.exactly_binds_adapter_effect(effect)",
            "exact_signed_broadcast_authority(effect)",
            "exact_signed_equivocation_authority(effect)",
            "LifecycleReplaySourceV1::ConsensusBroadcast(message.clone())",
            "LifecycleReplaySourceV1::Equivocation(evidence)",
            "canonical_replay_authority(",
        ] {
            assert!(
                direct.contains(required),
                "direct signed replay wrapper omitted {required}"
            );
        }

        for runtime_seal in [
            "SignedBroadcastReplayEvidenceV1",
            "SignedEquivocationReplayEvidenceV1",
            "DirectSignedPendingBindingV1",
        ] {
            let derive = production
                .split(runtime_seal)
                .next()
                .expect("direct signed seal has a declaration prefix")
                .rsplit("#[derive(")
                .next()
                .expect("direct signed seal derive is inspectable")
                .split(")]")
                .next()
                .expect("direct signed seal derive is bounded");
            assert!(
                !derive.contains("Decode") && !derive.contains("Encode"),
                "runtime seal {runtime_seal} became codec-constructible"
            );
        }
        for forbidden in [
            "pub(crate) struct SignedBroadcastReplayEvidenceV1",
            "pub(crate) struct SignedEquivocationReplayEvidenceV1",
            "pub(super) fn source(",
            "pub(super) fn message(",
            "pub(super) fn evidence(",
            "pub(super) fn encoded(",
            "pub(super) fn into_parts(",
            "pub(super) fn pending(",
            "pub(super) fn effect_identity(",
            "!= [0; 32]",
            "== [0; 32]",
            "is_zero()",
        ] {
            assert!(
                !direct.contains(forbidden),
                "direct signed replay wrapper exposed or reserved {forbidden}"
            );
        }

        for caller in [
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_lifecycle_ledger.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller.contains("SignedBroadcastReplayEvidenceV1"));
            assert!(!caller.contains("SignedEquivocationReplayEvidenceV1"));
        }
    }

    #[test]
    fn remote_proposal_replay_wrappers_are_opaque_exact_and_have_one_runtime_mint() {
        let source = include_str!("v2_lifecycle_replay_authority.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("replay authority has one production prefix");
        let remote = production
            .split("pub(in crate::sumeragi) struct RemoteProposalFetchReplayEvidenceV1")
            .nth(1)
            .expect("remote Proposal Fetch wrapper has one declaration")
            .split("/// Move-only pre-intent replay seal for one exact local")
            .next()
            .expect("local body replay follows remote Proposal replay");
        for required in [
            "RemoteProposalStoreReplayEvidenceV1",
            "RemoteProposalStoredReplayEvidenceV1",
            "RemoteProposalValidateReplayEvidenceV1",
            "from_exact_authenticated_proposal(",
            "RemoteProposalReplayMintPermit",
            "ingress.exactly_matches_authenticated(authenticated)",
            "certificate: None",
            "certified_sources.is_empty()",
            "pending.exactly_binds_adapter_effect(effect)",
            "project_proposal_fetch_store_successor",
            "project_store_validate_successor",
            "bind_durable_body(",
            "durable_body_frame_reference",
            "ReplayPayloadBindingV1::BodyFrame",
            "LifecycleStageKind::FetchBody",
            "LifecycleStageKind::StoreBody",
            "LifecycleStageKind::ValidateBody",
            "canonical_replay_authority(",
        ] {
            assert!(
                remote.contains(required),
                "remote Proposal replay wrapper omitted {required}"
            );
        }
        for wrapper in [
            "RemoteProposalFetchReplayEvidenceV1",
            "RemoteProposalStoreReplayEvidenceV1",
            "RemoteProposalStoredReplayEvidenceV1",
            "RemoteProposalValidateReplayEvidenceV1",
        ] {
            let derive = production
                .split(wrapper)
                .next()
                .expect("remote Proposal wrapper has a declaration prefix")
                .rsplit("#[derive(")
                .next()
                .expect("remote Proposal wrapper derive is inspectable")
                .split(")]")
                .next()
                .expect("remote Proposal wrapper derive is bounded");
            assert!(
                !derive.contains("Decode") && !derive.contains("Encode"),
                "runtime replay wrapper {wrapper} became codec-constructible"
            );
        }
        for forbidden in [
            "pub(crate) struct RemoteProposal",
            "pub(in crate::sumeragi) fn authenticated(",
            "pub(in crate::sumeragi) fn ingress(",
            "pub(in crate::sumeragi) fn source(",
            "pub(in crate::sumeragi) fn proposal(",
            "pub(in crate::sumeragi) fn pending(",
            "pub(in crate::sumeragi) fn receipt(",
            "pub(in crate::sumeragi) fn into_parts(",
            "!= [0; 32]",
            "== [0; 32]",
            "is_zero()",
        ] {
            assert!(
                !remote.contains(forbidden),
                "remote Proposal replay wrapper exposed or reserved {forbidden}"
            );
        }

        let runtime = include_str!("v2_runtime.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("runtime has one production prefix");
        assert_eq!(
            runtime
                .matches("RemoteProposalFetchReplayEvidenceV1::from_exact_authenticated_proposal(")
                .count(),
            1,
            "only authenticated runtime dispatch mints remote Proposal evidence"
        );
        for required in [
            "remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>",
            "deferred_remote_proposal_replay",
            "DeferredEventKind::ProposalReceived",
            "bind_remote_proposal_fetch_replay(",
            "certificate: None",
            "exact_remote_proposal_fetch_replay(",
        ] {
            assert!(
                runtime.contains(required),
                "runtime remote Proposal transport omitted {required}"
            );
        }
        for outside in [
            include_str!("v2_lifecycle_ledger.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            let outside = outside
                .split("\n#[cfg(test)]\nmod tests {")
                .next()
                .expect("outside production prefix is bounded");
            assert!(!outside.contains("RemoteProposalFetchReplayEvidenceV1"));
            assert!(!outside.contains("PreparedRemoteProposalFetchReplayPreAdmission"));
        }
    }

    #[test]
    fn invalid_body_runtime_evidence_is_nondecodable_exact_and_fixed_join_only() {
        let source = include_str!("v2_lifecycle_replay_authority.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("replay authority has one production prefix");
        let invalid = production
            .split("pub(in crate::sumeragi) enum DurableValidateReplayEvidenceV1")
            .nth(1)
            .expect("durable Validate replay enum has one declaration")
            .split("fn exact_certified_fetch_coordinates(")
            .next()
            .expect("certified Fetch projection follows invalid-body evidence");
        for required in [
            "Certified(CertifiedValidateReplayEvidenceV1)",
            "RemoteProposal(RemoteProposalValidateReplayEvidenceV1)",
            "pub(in crate::sumeragi) struct InvalidBodyReportReplayEvidenceV1",
            "authority: LifecycleReplayAuthorityV1",
            "validate_origin: DurableValidateReplayEvidenceV1",
            "report_pending: DirectSignedPendingBindingV1",
            "pub(in crate::sumeragi) fn seal_invalid_body_report(",
            "capability: RegisteredPrepareInvalidBodyReportCapability",
            "capability.exactly_matches_report(report_effect)",
            "validate_origin.exactly_matches_validate_pending(",
            "validate_pending: &PendingRuntimeEffectBinding",
            ".project_validate_report_invalid_certified_body_successor(",
            ".project_validate_report_invalid_certified_body_with_registered_prepare(",
            "DirectSignedPendingBindingV1::from_exact_effect(report_effect, report_pending)",
            "const CANONICAL_REJECTION_CODE: u8 = 0",
            "LifecycleReplaySourceV1::InvalidCertifiedBody",
            "body_frame_hash: *receipt.frame_hash().as_ref()",
            "LifecycleStageKind::ReportInvalidBody",
            "ReplayPayloadBindingV1::None",
            "project_sealed_invalid_body_report_candidate(",
            "_permit: &SealedInvalidBodyReportProjectionPermit",
            "authority_free_admission_projection(",
            "self.authority.clone()",
        ] {
            assert!(
                invalid.contains(required),
                "invalid-body runtime evidence omitted {required}"
            );
        }
        let persisted_invalid = production
            .split("struct InvalidBodyReplaySourceV1 {")
            .nth(1)
            .expect("persisted invalid-body source has one declaration")
            .split("struct CertifiedServeStorageSourceV1 {")
            .next()
            .expect("Certified Serve source follows invalid-body source");
        for required in [
            "validation_origin: BodyPipelineReplaySourceV1",
            "self.validation_origin.project(",
            "LifecycleStageKind::ValidateBody",
            "self.certificate.round != self.certificate.proposal_round",
            "BodyPipelineOriginV1::Proposal(proposal)",
            "certificate == &self.certificate && manifest == &self.outcome.manifest",
            "BodyPipelineOriginV1::LocalBody(_)",
            "origin_shape.key.context() != context.id()",
        ] {
            assert!(
                persisted_invalid.contains(required),
                "persisted invalid-body source omitted {required}"
            );
        }
        for runtime_seal in [
            "DurableValidateReplayEvidenceV1",
            "InvalidBodyReportReplayEvidenceV1",
        ] {
            let derive = production
                .split(runtime_seal)
                .next()
                .expect("runtime seal has a declaration prefix")
                .rsplit("#[derive(")
                .next()
                .expect("runtime seal derive is inspectable")
                .split(")]")
                .next()
                .expect("runtime seal derive is bounded");
            assert!(
                !derive.contains("Decode") && !derive.contains("Encode"),
                "runtime seal {runtime_seal} became codec-constructible"
            );
        }
        for forbidden in [
            "fn from_parts(",
            "fn into_parts(",
            "fn certificate(",
            "fn manifest(",
            "fn receipt(",
            "fn pending(",
            "fn source(",
            "fn encoded(",
            "fn candidate(",
            "!= [0; 32]",
            "== [0; 32]",
            "is_zero()",
        ] {
            assert!(
                !invalid.contains(forbidden),
                "invalid-body evidence exposed or reserved {forbidden}"
            );
        }

        let adapter = include_str!("v2.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("adapter production prefix is bounded");
        assert_eq!(
            adapter
                .matches("DurableValidateReplayEvidenceV1::seal_invalid_body_report(")
                .count(),
            1,
            "only the fixed adapter preview mints invalid-body evidence"
        );
        for required in [
            "struct RegisteredPrepareInvalidBodyReportCapability",
            "report_effect: AdapterEffect",
            "fn registered_prepare_report_capability(",
            ".project_validate_report_invalid_certified_body_with_registered_prepare(",
            "PreparedInvalidBodyReportAdapterReplay",
            "projected.as_ref() == Some(&self.child_pending)",
            "project_invalid_body_report_candidate(",
            "permit: &SealedInvalidBodyReportProjectionPermit",
            ".project_sealed_invalid_body_report_candidate(",
        ] {
            assert!(
                adapter.contains(required),
                "adapter invalid-body seal omitted {required}"
            );
        }
        let capability = adapter
            .split("pub(in crate::sumeragi) struct RegisteredPrepareInvalidBodyReportCapability")
            .nth(1)
            .expect("registered Prepare capability has one declaration")
            .split("/// Closed classification of one direct deterministic validation rejection.")
            .next()
            .expect("direct rejection classification follows its capability");
        for forbidden in [
            "derive(Clone",
            "fn into_parts(",
            "fn certificate(",
            "fn statement(",
            "RegisteredPrepareInvalidBodyReportLinearity",
            "impl Drop for RegisteredPrepareInvalidBodyReportCapability",
        ] {
            assert!(
                !capability.contains(forbidden),
                "registered Prepare capability exposed {forbidden}"
            );
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn live_wal_replay_seal_is_linear_nondecodable_and_has_two_closed_production_mints() {
        let source = include_str!("v2_lifecycle_replay_authority.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("replay authority has one production prefix");
        let live = production
            .split("struct LiveWalPersistedReplaySealV1")
            .nth(1)
            .expect("live WAL replay seal has one declaration")
            .split("/// Canonical inert replay evidence for one exact signed broadcast effect.")
            .next()
            .expect("direct signed evidence follows live WAL seal");
        for required in [
            "LiveWalPersistedReplayStateV1::ApplyPending",
            "LiveWalPersistedPendingV1::PayloadFree",
            "LiveWalPersistedPendingV1::ValidateSignBound",
            "LiveWalPersistedPendingV1::ApplyPending",
            "LiveWalPersistedPendingV1::ApplyBound",
            "from_exact_live_append(\n        cause: ExactLiveWalPersistedContinuationCause",
            "exactly_binds_payload_free_pending(&self)",
            "bind_exact_validate_sign_pending(",
            "exactly_binds_validate_sign_pending(&self)",
            "project_validate_apply_successor(predecessor_effect, &self.effect)",
            "exactly_matches_apply_effect(&self.effect, receipt)",
            "ReplayWalRoleV1::PROPOSAL_INTENT",
            "ReplayWalRoleV1::PREPARE_INTENT",
            "ReplayWalRoleV1::LOCK_AND_COMMIT",
            "ReplayWalRoleV1::TIMEOUT_INTENT",
            "ReplayWalRoleV1::DECISION",
            "ReplayWalRoleV1::INSTALL_TIMEOUT",
        ] {
            assert!(live.contains(required), "live WAL seal omitted {required}");
        }
        for forbidden in [
            "#[derive(Clone",
            "#[derive(Copy",
            "Decode",
            "pub(super) fn locator(",
            "pub(super) fn action(",
            "pub(super) fn source(",
            "pub(super) fn effect(",
            "pub(super) fn pending(",
            "into_parts",
            "RecoveredWalFrameIdentity",
            "!= [0; 32]",
            "== [0; 32]",
            "is_zero()",
        ] {
            assert!(
                !live.contains(forbidden),
                "live WAL seal exposed or reserved forbidden surface {forbidden}"
            );
        }

        let adapter = include_str!("v2.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("adapter has one production prefix");
        let runtime = include_str!("v2_runtime.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("runtime has one production prefix");
        let work_registry = include_str!("v2_lifecycle_work_registry.rs")
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("work registry has one production prefix");
        assert_eq!(
            adapter
                .matches("SealedLiveWalPersistedEffectV1::from_exact_live_append(")
                .count(),
            2,
            "only the generic persisted cut and sealed Ready-Sign cut mint live replay authority"
        );
        assert_eq!(
            adapter
                .matches("PendingRuntimeEffectBinding::from_exact_live_wal_append(")
                .count(),
            2,
            "the same two closed post-fsync cuts derive frame-bound placeholder owners"
        );
        let ready_sign = adapter
            .split("// READY_DURABLE_VALIDATE_LIVE_SIGN_BEGIN")
            .nth(1)
            .expect("sealed Ready-Sign segment exists")
            .split("// READY_DURABLE_VALIDATE_LIVE_SIGN_END")
            .next()
            .expect("sealed Ready-Sign segment is bounded");
        assert_eq!(
            ready_sign
                .matches("SealedLiveWalPersistedEffectV1::from_exact_live_append(")
                .count(),
            1
        );
        assert_eq!(
            ready_sign
                .matches("PendingRuntimeEffectBinding::from_exact_live_wal_append(")
                .count(),
            1
        );
        assert!(ready_sign.contains("LiveWalFrameIdentity::from_append_receipt("));
        assert!(ready_sign.contains("bind_exact_validate_sign_pending(child_pending)"));
        let generic = adapter
            .split("fn drive_exact_persisted_continuation(")
            .nth(1)
            .expect("generic exact persisted cut exists")
            .split("fn live_wal_record_exactly_owns_effect(")
            .next()
            .expect("generic exact persisted cut is bounded");
        assert_eq!(
            generic
                .matches("SealedLiveWalPersistedEffectV1::from_exact_live_append(")
                .count(),
            1
        );
        assert_eq!(
            generic
                .matches("PendingRuntimeEffectBinding::from_exact_live_wal_append(")
                .count(),
            1
        );
        assert_eq!(
            adapter
                .matches("drive_exact_persisted_continuation(")
                .count(),
            1,
            "the inert live cut has no production caller yet"
        );
        assert_eq!(runtime.matches("fn from_exact_live_wal_append(").count(), 1);
        assert_eq!(
            work_registry.matches(".complete_exact_apply(").count(),
            1,
            "only the retained Validate completion supplies an Apply receipt"
        );
        assert!(!adapter.contains("RecoveredWalFrameIdentity::for_test"));
        for outside in [
            include_str!("v2_lifecycle_ledger.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!outside.contains("SealedLiveWalPersistedEffectV1"));
            assert!(!outside.contains("drive_exact_persisted_continuation"));
        }
    }

    #[test]
    fn record_matching_rejects_substitution_of_every_external_coordinate() {
        let fixture = Fixture::new();
        let case = fixture
            .cases()
            .into_iter()
            .next()
            .expect("fixture has cases");
        let foreign_context =
            LifecycleContext::new(LifecycleDigest::new([0xFF; 32]), fixture.context.height());
        assert!(
            case.authority
                .validate_record(
                    foreign_context,
                    case.key,
                    case.work_class,
                    case.stage,
                    case.payload,
                )
                .is_err()
        );
        let wrong_key = LifecycleKey::new(
            case.key.context(),
            case.key.round(),
            case.key.proposal_round(),
            case.key.subject(),
            LifecyclePhase::BroadcastProposal,
            case.key.execution_commitment(),
        );
        assert_eq!(
            case.authority.validate_record(
                fixture.context,
                wrong_key,
                case.work_class,
                case.stage,
                case.payload,
            ),
            Err(ReplayAuthorityValidationError::RecordMismatch)
        );
        assert!(
            case.authority
                .validate_record(
                    fixture.context,
                    case.key,
                    LifecycleWorkClass::Broadcast,
                    case.stage,
                    case.payload,
                )
                .is_err()
        );
        assert!(
            case.authority
                .validate_record(
                    fixture.context,
                    case.key,
                    case.work_class,
                    LifecycleStage::new(
                        LifecycleStageKind::SignPrepareVote,
                        PredecessorScope::Independent,
                    ),
                    case.payload,
                )
                .is_err()
        );
        assert_eq!(
            case.authority.validate_record(
                fixture.context,
                case.key,
                case.work_class,
                case.stage,
                fixture.body_payload,
            ),
            Err(ReplayAuthorityValidationError::PayloadMismatch)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn typed_sources_reject_locator_role_signature_and_outcome_drift() {
        let fixture = Fixture::new();
        let wal_case = fixture.cases().remove(0);
        let mut wrong_locator = wal_case.authority.clone();
        let LifecycleReplaySourceV1::Wal(source) = &mut wrong_locator.source else {
            panic!("first fixture authority is WAL-backed")
        };
        source.locator = RecoveredWalFrameIdentity::for_test(8, 10, [0x21; 32]).persisted_locator();
        assert!(
            wrong_locator
                .validate_record(
                    fixture.context,
                    wal_case.key,
                    wal_case.work_class,
                    wal_case.stage,
                    wal_case.payload,
                )
                .is_err()
        );

        let mut wrong_role = wal_case.authority;
        let LifecycleReplaySourceV1::Wal(source) = &mut wrong_role.source else {
            panic!("first fixture authority is WAL-backed")
        };
        source.role = ReplayWalRoleV1::DECISION;
        assert!(
            wrong_role
                .validate_record(
                    fixture.context,
                    wal_case.key,
                    wal_case.work_class,
                    wal_case.stage,
                    wal_case.payload,
                )
                .is_err()
        );

        let mut broadcast = fixture.cases().remove(8).authority;
        let LifecycleReplaySourceV1::ConsensusBroadcast(message) = &mut broadcast.source else {
            panic!("ninth fixture authority is a broadcast")
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut message.payload else {
            panic!("ninth fixture authority broadcasts a proposal")
        };
        proposal.signature.clear();
        let broadcast_case = fixture.cases().remove(8);
        assert!(
            broadcast
                .validate_record(
                    fixture.context,
                    broadcast_case.key,
                    broadcast_case.work_class,
                    broadcast_case.stage,
                    broadcast_case.payload,
                )
                .is_err()
        );

        let invalid_case = fixture.cases().remove(19);
        let mut invalid = invalid_case.authority.clone();
        let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut invalid.source else {
            panic!("twentieth fixture authority is an invalid-body report")
        };
        source.outcome.rejection_code = 1;
        assert!(
            invalid
                .validate_record(
                    fixture.context,
                    invalid_case.key,
                    invalid_case.work_class,
                    invalid_case.stage,
                    invalid_case.payload,
                )
                .is_err()
        );

        let mut wrong_report_round = invalid_case.authority.clone();
        let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut wrong_report_round.source
        else {
            panic!("invalid-body fixture retains one report certificate")
        };
        source.certificate.round.view = source.certificate.round.view.saturating_add(1);
        assert!(
            wrong_report_round
                .validate_record(
                    fixture.context,
                    invalid_case.key,
                    invalid_case.work_class,
                    invalid_case.stage,
                    invalid_case.payload,
                )
                .is_err(),
            "the report QC round cannot diverge from its validation origin"
        );

        let mut wrong_remote_origin = invalid_case.authority.clone();
        let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut wrong_remote_origin.source
        else {
            panic!("invalid-body fixture retains one validation origin")
        };
        source.validation_origin.origin =
            BodyPipelineOriginV1::Proposal(fixture.conflicting_proposal.clone());
        assert!(
            wrong_remote_origin
                .validate_record(
                    fixture.context,
                    invalid_case.key,
                    invalid_case.work_class,
                    invalid_case.stage,
                    invalid_case.payload,
                )
                .is_err(),
            "a report cannot splice a different signed Proposal origin"
        );

        let mut local_origin = invalid_case.authority.clone();
        let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut local_origin.source else {
            panic!("invalid-body fixture retains one validation origin")
        };
        source.validation_origin.origin =
            BodyPipelineOriginV1::LocalBody(source.outcome.manifest.clone());
        assert!(
            local_origin
                .validate_record(
                    fixture.context,
                    invalid_case.key,
                    invalid_case.work_class,
                    invalid_case.stage,
                    invalid_case.payload,
                )
                .is_err(),
            "local body authority cannot stand in for a reported remote/certified origin"
        );

        let mut certified_origin = invalid_case.authority.clone();
        let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut certified_origin.source
        else {
            panic!("invalid-body fixture retains one validation origin")
        };
        source.validation_origin.origin = BodyPipelineOriginV1::Certified {
            certificate: fixture.prepare_qc.clone(),
            manifest: source.outcome.manifest.clone(),
            fetch_manifest_present: true,
            certified_sources: Vec::new(),
        };
        assert!(
            certified_origin
                .validate_record(
                    fixture.context,
                    invalid_case.key,
                    invalid_case.work_class,
                    invalid_case.stage,
                    invalid_case.payload,
                )
                .is_ok(),
            "the exact certified Validate origin remains canonical"
        );
        let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut certified_origin.source
        else {
            unreachable!("certified invalid-body fixture retains its source")
        };
        let BodyPipelineOriginV1::Certified { certificate, .. } =
            &mut source.validation_origin.origin
        else {
            unreachable!("certified invalid-body fixture retains its QC")
        };
        *certificate = fixture.commit_qc.clone();
        assert!(
            certified_origin
                .validate_record(
                    fixture.context,
                    invalid_case.key,
                    invalid_case.work_class,
                    invalid_case.stage,
                    invalid_case.payload,
                )
                .is_err(),
            "a certified origin must retain the report's exact PrepareQC"
        );

        let serve_case = fixture.cases().remove(20);
        let mut invalid_retainer = serve_case.authority.clone();
        let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut invalid_retainer.source
        else {
            panic!("twenty-first fixture authority is Certified-Serve storage")
        };
        source.local_retainer =
            u32::try_from(wire::MAX_VALIDATORS_PER_HEIGHT).expect("validator bound fits u32");
        assert!(
            invalid_retainer
                .validate_record(
                    fixture.context,
                    serve_case.key,
                    serve_case.work_class,
                    serve_case.stage,
                    serve_case.payload,
                )
                .is_err()
        );

        let local_store = fixture.cases().remove(5);
        let LifecycleReplaySourceV1::BodyPipeline(local_source) = local_store.authority.source
        else {
            panic!("sixth fixture authority is a local body source")
        };
        assert!(matches!(
            local_source.project(
                fixture.context,
                LifecycleStageKind::FetchBody,
                &ReplayPayloadBindingV1::None,
            ),
            Err(ReplayAuthorityValidationError::RecordMismatch)
        ));
    }
}

#[cfg(test)]
pub(super) use tests::{
    exact_body_record_fixture, exact_certified_fetch_record_fixture,
    exact_local_body_record_fixture, exact_record_fixture,
    foreign_certified_serve_family_authority_fixture,
};
