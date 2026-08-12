//! Closed, codec-only authority envelope for future lifecycle replay.
//!
//! This module deliberately performs structural matching only. Decoding this
//! envelope does not authenticate its consensus artifacts or make executable
//! work. A future admission transaction must first reauthenticate the retained
//! source against the verified height context and its owning durable store.

use std::sync::Arc;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll as _, Encode};

use crate::sumeragi::{
    v2::{
        AdapterEffect, ExactLiveWalPersistedContinuationCause, LiveWalFrameIdentity,
        PersistedWalFrameLocatorV1, RecoveredWalFrameIdentity,
        RegisteredPrepareInvalidBodyReportCapability, SignRequest, VerifiedHeightContext,
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
        RecoveredWalCandidateProjectionPermit, RemoteProposalReplayMintPermit,
        RuntimeEffectOwnership, RuntimeIngressOwnershipEvidence,
    },
    v2_transport::AuthenticatedCertifiedBodyRequest,
};

use super::{
    body_pipeline_transition::SealedInvalidBodyReportProjectionPermit,
    projection::{
        AdapterEffectAdmissionError, block_subject, certified_serve_key_subject,
        durable_body_frame_reference, execution_commitment,
    },
    schema::{
        CandidateAdmission, CausalRoot, DurableBodyFrameReference, DurablePayloadReference,
        DurableRecordMetadata, DurableServeNegativeOutcome, InitialLifecycleState,
        LifecycleContext, LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRecord,
        LifecycleRound, LifecycleStage, LifecycleStageKind, LifecycleWorkClass, OwnerId,
        PhysicalGeometry, PredecessorScope, ProducerTurnAdmission, TerminalOutcome,
        serve_and_producer_keys_match,
    },
    selector::CertifiedFetchCompletionAuthority,
    work_registry::{InstalledBodyCandidateProjectionPermit, SealedBodySuccessorProjectionPermit},
};

const REPLAY_AUTHORITY_FORMAT_VERSION: u16 = 1;
const MAX_REPLAY_AUTHORITY_BYTES: usize = 4 * 1024 * 1024;
const EQUIVOCATION_SUBJECT_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:equivocation-subject:v1";

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

    #[cfg(test)]
    /// Test whether this authority retains one exact Certified-Serve frame hash.
    pub(super) fn certified_serve_frame_hash_is(&self, expected: Hash) -> bool {
        matches!(
            &self.source,
            LifecycleReplaySourceV1::CertifiedServeStorage(source)
                if source.payload_hash == *expected.as_ref()
        )
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
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "Certified-Serve replay evidence must remain with its reserved producer turn"]
pub(super) struct CertifiedServeReplayEvidenceV1 {
    family: Arc<CertifiedServeStorageReplayFamilyV1>,
    payload: ReplayPayloadBindingV1,
}

/// Opaque replay evidence for the dormant ProducerTurn reserved beside one
/// exact Certified-Serve request.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "ProducerTurn replay evidence must remain with its Certified-Serve origin"]
pub(super) struct CertifiedServeProducerTurnReplayEvidenceV1 {
    family: Arc<CertifiedServeStorageReplayFamilyV1>,
}

/// Closed pair preserving one common post-fsync storage origin across the
/// Certified-Serve record and its reserved ProducerTurn.
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Consume one exact shared storage family into the adjacent durable
    /// admission pair. Semantic keys, stages, payloads, and authorities are
    /// derived here; callers supply only already-checked physical geometry.
    pub(super) fn into_admission(
        self,
        active_context: LifecycleContext,
        serve_geometry: PhysicalGeometry,
        producer_geometry: PhysicalGeometry,
        storage_payload_hash: Hash,
    ) -> Option<CandidateAdmission> {
        let storage_payload = self.serve.payload.durable_payload()?;
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
        Some(CandidateAdmission::new(
            serve_shape.key,
            CausalRoot::new(reconstruction_source),
            LifecycleWorkClass::CertifiedServe,
            serve_stage,
            InitialLifecycleState::Ready,
            reconstruction_source,
            serve_payload,
            serve_authority,
            serve_geometry,
            Some(ProducerTurnAdmission::new(
                producer_shape.key,
                producer_stage,
                reconstruction_source,
                producer_authority,
                producer_geometry,
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
    /// Compare two opaque Serve evidence values without exposing their origin.
    pub(super) fn exactly_matches(&self, other: &Self) -> bool {
        self == other
    }

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
    /// Compare two opaque ProducerTurn values without exposing their origin.
    pub(super) fn exactly_matches(&self, other: &Self) -> bool {
        self == other
    }

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
            ReplayPayloadBindingV1::from_payload(DurablePayloadReference::CertifiedServeNegative {
                request: LifecycleDigest::new(*request_hash.as_ref()),
                certificate: LifecycleDigest::new(*certificate_hash.as_ref()),
                outcome,
            })
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
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableCertifiedFetchBodyReceipt,
        store_effect: &AdapterEffect,
    ) -> Option<CertifiedStoreReplayEvidenceV1> {
        (self.exactly_matches_fetch(fetch_effect, response, receipt)
            && certified_body_stage_matches(
                &self.family,
                store_effect,
                receipt.durable_body(),
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
            manifest: Some(manifest.clone()),
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
            LifecycleStageKind::FetchBody => ReplayPayloadBindingV1::None,
            LifecycleStageKind::StoreBody | LifecycleStageKind::ValidateBody => {
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
        manifest: Some(manifest),
    } = &family.source.origin
    else {
        return None;
    };
    Some(CertifiedBodyPipelineCoordinatesV1 {
        tag: family.source.tag,
        certificate: certificate.clone(),
        manifest: manifest.clone(),
    })
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

fn exact_recovered_wal_vote_authority(
    locator: RecoveredWalFrameIdentity,
    tag: EventTag,
    vote: &wire::Vote,
) -> Option<LifecycleReplayAuthorityV1> {
    if !locator.is_exact() || tag.height() != vote.round.height || tag.view() != vote.round.view {
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
                if !self.role.matches(role)
                    || !vote_shape(context, vote, false)
                    || !self.tag.matches_round(context, vote.round)
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
        manifest: Option<wire::PayloadManifest>,
    },
    #[codec(index = 2)]
    LocalBody(wire::PayloadManifest),
}

impl BodyPipelineReplaySourceV1 {
    fn project(
        &self,
        context: LifecycleContext,
        requested_stage: LifecycleStageKind,
        payload: &ReplayPayloadBindingV1,
    ) -> Result<ReplayShape, ReplayAuthorityValidationError> {
        let (round, proposal_round, subject, commitment, manifest, local_body) = match &self.origin
        {
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
                )
            }
            BodyPipelineOriginV1::Certified {
                certificate,
                manifest,
            } => {
                if !qc_shape(context, certificate)
                    || manifest.as_ref().is_some_and(|manifest| {
                        !manifest_matches_origin(
                            context,
                            manifest,
                            certificate.proposal_round,
                            certificate.subject,
                        )
                    })
                {
                    return Err(ReplayAuthorityValidationError::InvalidSource);
                }
                (
                    certificate.round,
                    certificate.proposal_round,
                    certificate.subject,
                    Some(execution_commitment(certificate.execution_commitment)),
                    manifest.as_ref(),
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
                )
            }
        };
        if !self.tag.matches_round(context, round) {
            return Err(ReplayAuthorityValidationError::InvalidSource);
        }
        let (phase, work_class) = match requested_stage {
            LifecycleStageKind::FetchBody => (LifecyclePhase::Fetch, LifecycleWorkClass::Fetch),
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
            LifecycleStageKind::FetchBody if payload.is_none() => {}
            LifecycleStageKind::StoreBody | LifecycleStageKind::ValidateBody
                if manifest.is_some_and(|manifest| {
                    payload.matches_exact_body(context, proposal_round, subject, manifest)
                }) => {}
            _ => return Err(ReplayAuthorityValidationError::PayloadMismatch),
        }
        Ok(ReplayShape::new(key, work_class, requested_stage))
    }
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
                manifest: Some(manifest),
            } if certificate == &self.certificate && manifest == &self.outcome.manifest => {}
            BodyPipelineOriginV1::Proposal(_)
            | BodyPipelineOriginV1::Certified { .. }
            | BodyPipelineOriginV1::LocalBody(_) => {
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

include!("v2_lifecycle_replay_authority_payload_projection.rs");
#[cfg(test)]
mod tests {
    include!("tests/v2_lifecycle_replay_authority_fixtures.rs");
    include!("tests/v2_lifecycle_replay_authority_cases.rs");
}

#[cfg(test)]
pub(super) use tests::{
    exact_body_record_fixture, exact_certified_fetch_record_fixture,
    exact_local_body_record_fixture, exact_record_fixture,
    foreign_certified_serve_family_authority_fixture,
};
