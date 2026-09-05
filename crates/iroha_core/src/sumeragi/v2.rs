//! Production adapter for the executable Sumeragi v2 reducer, whose crate has no codec,
//! cryptography, filesystem, or networking dependencies. This module binds it to canonical
//! data-model wire types and the crash-safe safety WAL. A complete WAL frame is encoded, appended,
//! flushed, and synchronised before its exact persistence identifier is acknowledged. No caller
//! can observe a causally later signing, broadcast, view-change, or apply effect before that point.
use super::v2_core as reducer;
#[path = "v2_leader_wire_consumer.rs"]
mod leader_wire_consumer;
pub(crate) use leader_wire_consumer::{
    LeaderWireRecoveryAuthority, vote_statement_hash as leader_wire_vote_statement_hash,
};
#[path = "v2_pending_kura_recovery.rs"]
mod pending_kura_recovery;
pub(crate) use pending_kura_recovery::{
    DeferredPendingKuraValidatedMarkerV1, DeferredReleasedLifecycleValidatedMarkerV1,
    PendingKuraValidatedApplySuccessorV1,
};
pub(in crate::sumeragi) use pending_kura_recovery::{
    InstalledPendingKuraApplyV1, PreparedPendingKuraValidatedApplyV1,
    PreparedRecoveredPendingKuraApplyReplayV1, PreparedReleasedLifecycleValidatedApplyV1,
    RecoveredPendingKuraApplyReplayV1, ReleasedLifecycleValidateTerminalProofV1,
};

#[cfg(test)]
use super::v2_lifecycle_coordinator::{
    AuthenticatedLifecycleRecoveryCut, DurableAuthenticatedRecoveredWalValidateLifecycleRepair,
    InstalledRecoveredWalSignRegistryCut, OpenedRecoveredWalSignLifecycleCut,
    RecoveredWalSignInstallError, RecoveredWalSignLifecycleOpenError,
    RecoveredWalValidateLedgerPersistError, RecoveredWalValidateRegistryCut,
    RecoveredWalValidateRegistryJoinError, append_same_owner_foreign_terminal_for_test,
    substitute_recovered_control_replay_authority_for_test,
    substitute_recovered_decision_fetch_owner_for_test,
    substitute_recovered_decision_fetch_replay_authority_for_test,
};
#[cfg(all(test, feature = "bls"))]
use super::v2_lifecycle_coordinator::{
    control_timeout_supersession_persistence_failure_for_test,
    control_timeout_supersession_summary_for_test,
    install_non_timeout_broadcast_before_current_control_for_test,
    install_timeout_broadcasts_before_current_control_for_test,
};
use super::{
    safety_wal::{
        RecoveredRecord, SafetyWal, SafetyWalAppendReceipt, SafetyWalError,
        SafetyWalLeaderWireStoreAuthority,
    },
    serviced_candidate_store::{
        LeaderWireLifecycleRestore, LeaderWireLifecycleStoreGate, ProducerContinuationAddress,
        ProducerContinuationHandoffToken, ProducerContinuationIdentity, ProducerContinuationRecord,
        ProducerContinuationReservation, ProducerContinuationSourceClass,
        ProducerContinuationStatus, ProducerContinuationTerminalToken,
        SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE, ServicedCandidateKey, ServicedCandidateStore,
        serviced_candidate_stage_for_kind_code,
    },
    v2_body_store::{
        DurableBodyReceipt, RecoveredDecisionApplyAdapterPreviewPermit,
        RecoveredLifecycleColdProposalOutputMintPermitV1,
        RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitV1,
        RecoveredLifecycleNextVoteBodyColdPreviewBindPermitV1, V2BodyStoreInstanceIdentity,
        ValidatedBodyReceipt,
    },
    v2_lifecycle_coordinator::{
        AdapterEffectAdmissionError, AuthenticatedRecoveredLifecycleSuccessorFloorV1,
        AuthenticatedRecoveredReleasedValidateNoSuccessorV1,
        AuthenticatedRecoveredWalControlProjection,
        AuthenticatedRecoveredWalDecisionFetchProjection,
        AuthenticatedRecoveredWalValidateLifecycleRepair, CandidateAdmission,
        DurableValidateReplayEvidenceV1, ExactStoreRecoveredWalPersistError,
        ExactStoreRecoveredWalSignInstallError, InstalledRecoveredWalSignStorage,
        InvalidBodyReportReplayEvidenceV1, LifecycleContext,
        LifecycleDecisionApplyCompletionProjectionPermitV1,
        LifecycleDecisionApplyDispatchIdentityV1, LifecycleDecisionApplyDispatchKeyV1,
        LifecycleDecisionApplyLineageV1, LifecycleDigest, LifecycleLedgerError, LifecycleLedgerV1,
        LifecycleWorkRegistryHolder, LiveValidateApplyRegistryReservation,
        LiveValidateApplyWorkProjectionPermit, LiveValidateReportRegistryReservation,
        LiveValidateReportWorkProjectionPermit, LiveValidateSignRegistryReservation,
        LiveValidateSignWorkProjectionPermit, LocalProposalIntentReplayEvidenceV1,
        OpenedRecoveredWalValidateLedger, PendingLiveWalSignAdmissionV1,
        PersistedRecoveredWalValidateLedger, PreparedLiveValidateApplyRegistryWork,
        PreparedLiveValidateReportRegistryWork, PreparedLiveValidateSignRegistryWork,
        ProductionLifecycleOwnerV1, ProductionOpenedRecoveredWalSignLifecycleCut,
        ProductionRecoveredWalStorageError, PublishedFinalizedLifecycleRetainedFloorV1,
        ReadyRejectedAdapterAuthority, ReadyValidateApplyPredecessorAuthority,
        ReadyValidateSignPredecessorAuthority, ReadyValidatedAdapterAuthority,
        RecoveredDecisionApplyCandidateLineageV1, RecoveredDecisionApplyPendingLineageV1,
        RecoveredDecisionApplyRegistryProjectionPermit, RecoveredDecisionFetchStoreProjectionV1,
        RecoveredDecisionValidateProjectionV1, RecoveredLifecycleNextWalVoteSealV1,
        RecoveredWalControlReplayEvidenceV1, RecoveredWalDecisionFetchReplayEvidenceV1,
        RecoveredWalParentFactoryError, RecoveredWalProductionOwnerOpenV1,
        RecoveredWalVoteReplayEvidenceV1, SealedInvalidBodyReportProjectionPermit,
        SealedLiveWalPersistedEffectV1, SealedValidateApplyProjectionPermit,
        SealedValidateSignProjectionPermit,
    },
    v2_runtime::{
        PendingRuntimeEffectBinding, RecoveredWalCandidateProjectionPermit,
        RecoveredWalControlPendingMintPermit, RecoveredWalDecisionFetchPendingMintPermit,
    },
};
use iroha_crypto::{Hash, HashOf, KeyPair, PublicKey, Signature};
use iroha_data_model::{account::AccountId, block::consensus_v2 as wire, peer::PeerId};
use norito::codec::{Decode, Encode};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use thiserror::Error;
// Keep wire admission and reducer capacity identical; mismatches fail at compile time.
const _: [(); wire::MAX_VALIDATORS_PER_HEIGHT] = [(); reducer::MAX_VOTING_ROSTER_LEN];
use crate::kura::{
    Kura, KuraInstanceIdentity, KuraSafetyWalDirectoryAuthority, KuraV2CommitReceipt,
};
const AGGREGATE_TOKEN_PREFIX: &[u8] = b"sumeragi-v2:verified-aggregate\0";
const MAX_DEFERRED_INPUTS: usize = 1024;
const MAX_DEFERRED_PROGRESS_INPUTS: usize = wire::MAX_VALIDATORS_PER_HEIGHT * 3 + 3;
const MAX_INGRESS_SEMANTIC_KEYS: usize = 1024;
// Scheduler priority is physical ownership evidence, not part of the logical
// reducer occurrence. One canonical key makes Normal/Progress rerouting
// coalesce to the same service identity.
const ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS: u8 = u8::MAX;
/// Maximum adapter effects returned by one serialized runtime invocation.
///
/// A reducer transition without persistence already has this exact source
/// bound. Persistence is acknowledged synchronously, but the record-specific
/// budgets below prove that replacing the sole `Persist` effect with its
/// causal `Persisted` continuation produces at most four effects. Keeping the
/// adapter bound equal to the reducer bound therefore matches the executor's
/// retained-batch contract without inflating either queue.
const MAX_ADAPTER_EFFECTS_PER_MACRO_STEP: usize = reducer::MAX_EFFECTS_PER_STEP;
/// Maximum validation-marker identities which can be authoritative at restart.
///
/// The replayed adapter contributes at most one durable highest Prepare, one
/// durable lock, and one durable decision in addition to its already-bounded
/// startup effect batch. Historical view-local markers outside this frontier
/// remain body-availability evidence, but cannot force synchronous execution or
/// restore vote authority.
const MAX_RECOVERED_VALIDATION_AUTHORITIES: usize = MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 3;
/// Largest record-specific `Persist -> Persisted` flattened batch.
///
/// The witness is locally formed `InstallTimeout`: its individual TimeoutVote
/// is subsumed by the certificate, so the source emits only `Persist`; the
/// acknowledgement can emit `EnterView`, one protected-body fetch, the TC
/// broadcast, and one reconstructed locked Commit signature. Thus the exact
/// maximum is four.
const MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP: usize = 4;
// Every persistence-flattened batch must fit the executor's already verified
// source-transition capacity. A future record shape which breaks this
// relation fails at compile time as well as at the runtime checks below.
const _: () =
    assert!(MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
include!("v2_adapter_persistence_and_wal_types.rs");
impl RecoveredWalFrameIdentity {
    fn from_recovered_record(record: &RecoveredRecord, persistence_id: u64) -> Option<Self> {
        let identity = Self {
            frame_sequence: record.sequence(),
            persistence_id,
            frame_hash: record.frame_hash(),
        };
        identity.is_exact().then_some(identity)
    }
    /// Return whether this sealed frame has the canonical reducer persistence relation.
    pub(crate) const fn is_exact(self) -> bool {
        match self.frame_sequence.checked_add(1) {
            Some(next) => next == self.persistence_id,
            None => false,
        }
    }
    /// Match another sealed WAL-frame identity without exposing its parts.
    pub(crate) fn exactly_matches(self, other: Self) -> bool {
        self.frame_sequence == other.frame_sequence
            && self.persistence_id == other.persistence_id
            && self.frame_hash == other.frame_hash
    }
    /// Project inert codec evidence without making the runtime seal decodable.
    pub(crate) const fn persisted_locator(self) -> PersistedWalFrameLocatorV1 {
        PersistedWalFrameLocatorV1 {
            frame_sequence: self.frame_sequence,
            persistence_id: self.persistence_id,
            frame_hash: self.frame_hash,
        }
    }
    #[cfg(test)]
    fn exactly_matches_record(self, record: &RecoveredRecord) -> bool {
        self.frame_sequence == record.sequence()
            && self.frame_hash == record.frame_hash()
            && self.is_exact()
    }
    /// Construct an exact scalar fixture without widening the production mint.
    #[cfg(test)]
    pub(crate) const fn for_test(
        frame_sequence: u64,
        persistence_id: u64,
        frame_hash: [u8; 32],
    ) -> Self {
        Self {
            frame_sequence,
            persistence_id,
            frame_hash,
        }
    }
}
/// Codec-only V1 projection of a verified WAL-frame identity.
///
/// Decoding this value establishes only a structural locator. It is never
/// accepted as runtime WAL authority; the executable recovery seal remains
/// [`RecoveredWalFrameIdentity`] and has no decoding implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct PersistedWalFrameLocatorV1 {
    frame_sequence: u64,
    persistence_id: u64,
    frame_hash: [u8; 32],
}
impl PersistedWalFrameLocatorV1 {
    /// Check the canonical reducer persistence relation without authenticating provenance.
    pub(crate) const fn is_exact(self) -> bool {
        match self.frame_sequence.checked_add(1) {
            Some(next) => next == self.persistence_id,
            None => false,
        }
    }
    /// Compare inert persisted evidence with one sealed runtime identity.
    pub(crate) fn exactly_matches_runtime(self, runtime: RecoveredWalFrameIdentity) -> bool {
        self.frame_sequence == runtime.frame_sequence
            && self.persistence_id == runtime.persistence_id
            && self.frame_hash == runtime.frame_hash
    }
}
/// Opaque authority for one startup vote whose local safety intent is already durable.
///
/// The adapter is the sole constructor. Minting consumes a private, non-clone
/// startup wrapper, removes the exact reducer-owned `Sign` effect before any
/// raw batch can escape, and reauthenticates the latest exact matching WAL
/// envelope after independently checking the terminal frontier.
/// Ordinary code therefore cannot retain both the raw effect and this
/// restart-only authority. Dropping the token is inert; the sole consuming
/// runtime projection converts it into one exact causal successor.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a recovered WAL vote-sign authority must be joined or deliberately abandoned"]
pub(crate) struct RecoveredWalVoteSign {
    wal_identity: RecoveredWalFrameIdentity,
    replay_evidence: RecoveredWalVoteReplayEvidenceV1,
    tag: reducer::EventTag,
    vote: wire::Vote,
    prepare_certificate: Option<wire::QuorumCertificate>,
}
/// Opaque authority for one payload-free control signature owned by an exact WAL frame.
///
/// The adapter consumes the sole matching startup effect into this move-only
/// value. No effect, pending binding, locator part, encoded authority, or
/// ordinal can be extracted. Its only consuming path is the runtime-private
/// recovered-frame projection below.
#[must_use = "a recovered WAL control Sign must enter the lifecycle startup transaction"]
pub(crate) struct RecoveredWalControlSign {
    wal_identity: RecoveredWalFrameIdentity,
    replay_evidence: RecoveredWalControlReplayEvidenceV1,
    effect: AdapterEffect,
}
/// Opaque authority for the exact certified Fetch emitted by a durable Decision.
///
/// This token retains the authenticated Decision frame, complete CommitQC,
/// frozen ordered archive roster, exact Fetch effect, and canonical V1 replay
/// evidence. It has no effect, certificate, locator, pending, candidate, or
/// parts accessor; only the runtime-private consuming projection can use it.
#[must_use = "a recovered Decision Fetch must enter lifecycle startup recovery"]
pub(crate) struct RecoveredWalDecisionFetch {
    wal_identity: RecoveredWalFrameIdentity,
    replay_evidence: RecoveredWalDecisionFetchReplayEvidenceV1,
    effect: AdapterEffect,
}
impl RecoveredWalControlSign {
    /// Consume both runtime one-shot permits into the sealed lifecycle projection.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_lifecycle_projection(
        self,
        pending_permit: RecoveredWalControlPendingMintPermit,
        candidate_permit: RecoveredWalCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
    ) -> Result<AuthenticatedRecoveredWalControlProjection, Self> {
        if !self
            .replay_evidence
            .exactly_matches_recovered_control(self.wal_identity, &self.effect)
        {
            return Err(self);
        }
        let Some(pending) = PendingRuntimeEffectBinding::from_exact_recovered_wal_frame(
            pending_permit,
            self.wal_identity,
            &self.effect,
        ) else {
            return Err(self);
        };
        let Some(candidate) = self.replay_evidence.project_recovered_control_candidate(
            candidate_permit,
            verified,
            self.wal_identity,
            &self.effect,
            &pending,
        ) else {
            return Err(self);
        };
        Ok(
            AuthenticatedRecoveredWalControlProjection::from_runtime_projection(
                self.wal_identity,
                self.replay_evidence,
                self.effect,
                pending,
                candidate,
            ),
        )
    }
}
impl RecoveredWalDecisionFetch {
    /// Consume both private permits into one sealed lifecycle projection.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_lifecycle_projection(
        self,
        pending_permit: RecoveredWalDecisionFetchPendingMintPermit,
        candidate_permit: RecoveredWalCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
    ) -> Result<AuthenticatedRecoveredWalDecisionFetchProjection, Self> {
        if !self
            .replay_evidence
            .exactly_matches_recovered_decision_fetch(verified, self.wal_identity, &self.effect)
        {
            return Err(self);
        }
        let Some(pending) = self.replay_evidence.reconstruct_pending(
            pending_permit,
            verified,
            self.wal_identity,
            &self.effect,
        ) else {
            return Err(self);
        };
        let Some(candidate) = self
            .replay_evidence
            .project_recovered_decision_fetch_candidate(
                candidate_permit,
                verified,
                self.wal_identity,
                &self.effect,
                &pending,
            )
        else {
            return Err(self);
        };
        Ok(
            AuthenticatedRecoveredWalDecisionFetchProjection::from_runtime_projection(
                self.wal_identity,
                self.replay_evidence,
                self.effect,
                pending,
                candidate,
            ),
        )
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredWalVoteSign {
    /// Return the complete opaque WAL-frame identity for sealed recovery joins.
    pub(crate) const fn wal_identity(&self) -> RecoveredWalFrameIdentity {
        self.wal_identity
    }
    /// Borrow the inert canonical replay evidence attached by the WAL mint.
    pub(crate) const fn replay_evidence(&self) -> &RecoveredWalVoteReplayEvidenceV1 {
        &self.replay_evidence
    }
    /// Revalidate the attached evidence against this complete recovered vote.
    pub(crate) fn replay_evidence_is_exact(&self) -> bool {
        self.replay_evidence
            .exactly_matches_recovered_vote(self.wal_identity, self.tag, &self.vote)
    }
    /// Revalidate this authority against the same verified recovered record.
    #[cfg(test)]
    fn exactly_matches_wal_record(&self, record: &RecoveredRecord) -> bool {
        self.wal_identity.exactly_matches_record(record)
    }
    /// Return the exact replay incarnation which owns the startup signature.
    pub(crate) const fn tag(&self) -> reducer::EventTag {
        self.tag
    }
    /// Borrow the exact canonical unsigned Prepare or Commit vote.
    pub(crate) const fn vote(&self) -> &wire::Vote {
        &self.vote
    }
    /// Borrow the exact authenticated PrepareQC carried by `LockAndCommit`.
    ///
    /// A `PrepareIntent` authority returns `None`.
    pub(crate) fn prepare_certificate(&self) -> Option<&wire::QuorumCertificate> {
        self.prepare_certificate.as_ref()
    }
}
/// Unpublished startup effects retained beside the adapter which produced them.
///
/// Fields are private and the value is not cloneable. Its only consuming
/// authentication validates the terminal WAL frontier independently, then
/// classifies the reducer's exact current work as no authority, one phase vote,
/// one control Sign, or the exact certificate-backed Decision Fetch. The latest
/// exact matching authenticated frame owns a signature; the exact Decision
/// frame owns its Fetch. Later durable frames may own the reducer's sealed
/// signature FIFO. Every other residual inventory is rejected before the
/// already-revalidated body cut is unsealed or Serve/Ledger opens. Caller-owned
/// effect bytes therefore cannot mint or duplicate any recovered authority.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "recovered adapter startup must be authenticated before effects are exposed"]
pub(crate) struct RecoveredAdapterStartup {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
}
/// Startup cut after the WAL frontier and current WAL work are authenticated.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "authenticated adapter startup must enter the unified durable recovery owner"]
pub(crate) struct AuthenticatedRecoveredAdapterStartup {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
    authority: RecoveredWalStartupAuthorityV1,
    validation_authority: RecoveredValidationAuthority,
    factory_owner: Arc<AuthenticatedRecoveredAdapterFactoryOwnerV1>,
}
struct AuthenticatedRecoveredAdapterFactoryOwnerV1;
/// Exhaustive and mutually exclusive current recovered-WAL work authority.
#[allow(variant_size_differences)]
enum RecoveredWalStartupAuthorityV1 {
    None,
    PhaseVote(RecoveredWalVoteSign),
    ControlSign(RecoveredWalControlSign),
    DecisionFetch(RecoveredWalDecisionFetch),
}

/// Opaque recovered ownership of one already-attempted local Proposal.
///
/// The replay-authenticated current-round [`reducer::WalRecord::ProposalIntent`]
/// is the sole mint. The token exposes only a fixed comparison against the
/// reducer's current proposal directive, so the runner can suppress duplicate
/// local assembly without receiving a replayed effect, tag, round, or subject.
#[must_use = "recovered local Proposal ownership must initialize runner proposal state"]
pub(in crate::sumeragi) struct RecoveredLifecycleLocalProposalAttemptV1 {
    tag: reducer::EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
}

impl RecoveredLifecycleLocalProposalAttemptV1 {
    /// Project one already-attempted current-round Proposal from the exact
    /// replay-authenticated durable reducer frontier.
    ///
    /// A later Prepare/Timeout record or residual phase-vote/control owner must
    /// not hide the earlier same-view ProposalIntent. Conversely, an intent
    /// from an older view is only durable non-equivocation history and cannot
    /// suppress a proposal in the reducer's successor view.
    fn from_authenticated_durable_current_round(
        adapter: &SumeragiV2Adapter,
    ) -> Result<Option<Self>, AdapterError> {
        let tag = adapter.reducer.current_tag();
        let round = reducer::Round::new(tag.height(), tag.view());
        let Some(proposal) = adapter.reducer.durable_state().proposal_intent(round) else {
            return Ok(None);
        };
        let wire_round = adapter.registry.round_to_wire(proposal.round());
        let subject = adapter.registry.subject(proposal.manifest().subject())?;
        if wire_round.context_id != adapter.wire_context.id()
            || wire_round.height != tag.height()
            || wire_round.view != tag.view()
        {
            return Err(AdapterError::RecoveredStartupEffectMismatch);
        }
        Ok(Some(Self {
            tag,
            round: wire_round,
            subject,
        }))
    }

    fn from_control(control: &RecoveredWalControlSign) -> Option<Self> {
        let AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } = &control.effect
        else {
            return None;
        };
        Some(Self {
            tag: *tag,
            round: proposal.round,
            subject: proposal.subject,
        })
    }

    /// Compare only against reducer-owned current proposal constraints.
    pub(in crate::sumeragi) fn exactly_matches_directive(
        &self,
        current: LocalProposalDirective,
    ) -> bool {
        self.tag == current.tag()
            && self.round.height == self.tag.height()
            && self.round.view == self.tag.view()
            && current.decided_subject().is_none()
            && current.locked_body().is_none_or(|(locked_round, _)| {
                self.round.context_id == locked_round.context_id
                    && self.round.height == locked_round.height
            })
    }

    /// Build a comparison-only recovery owner for focused boundary tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn for_test(
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Self {
        Self {
            tag,
            round,
            subject,
        }
    }
}
/// Adapter and residual replay effects retained by the sole lifecycle owner.
///
/// The fields have no extraction surface. A consuming launch moves this state
/// directly into the serialized runtime while status remains unpublished, so
/// neither recovery branch can release a free-standing adapter startup.
#[must_use = "adapter startup authority must remain inside its lifecycle owner"]
pub(crate) struct ProductionLifecycleAdapterStartupV1 {
    state: ProductionLifecycleAdapterStartupStateV1,
}
/// One exact reducer input that must be replayed before a durable ordinary
/// certified-body successor can become executable after restart.
///
/// The lifecycle storage join constructs these steps from the authenticated
/// terminal parent rows and their sealed body frames. They are deliberately
/// ordinal-bearing so cold replay has one canonical order even when Fetch,
/// Store, and Validate capacity classes contain different owners.
#[derive(Clone, Debug)]
pub(in crate::sumeragi) struct CertifiedBodyPipelineColdReplayStepV1 {
    ordinal: u128,
    kind: CertifiedBodyPipelineColdReplayKindV1,
}
#[derive(Clone, Debug)]
enum CertifiedBodyPipelineColdReplayKindV1 {
    BodyAvailable {
        tag: reducer::EventTag,
        manifest: wire::PayloadManifest,
        expected_store: AdapterEffect,
    },
    BodyStored {
        tag: reducer::EventTag,
        durable: DurableBodyReceipt,
        expected_validate: AdapterEffect,
    },
}
impl CertifiedBodyPipelineColdReplayStepV1 {
    /// Stable cold-replay order. Standalone Proposal Validate recovery uses
    /// both reducer inputs at one immutable lifecycle ordinal.
    pub(in crate::sumeragi) const fn order_key(&self) -> (u128, u8) {
        let sequence = match &self.kind {
            CertifiedBodyPipelineColdReplayKindV1::BodyAvailable { .. } => 0,
            CertifiedBodyPipelineColdReplayKindV1::BodyStored { .. } => 1,
        };
        (self.ordinal, sequence)
    }
    /// Seal one terminal Fetch input and its exact live Store effect.
    pub(in crate::sumeragi) fn body_available(
        ordinal: u128,
        tag: reducer::EventTag,
        manifest: wire::PayloadManifest,
        expected_store: AdapterEffect,
    ) -> Option<Self> {
        let AdapterEffect::StoreBody {
            tag: store_tag,
            round,
            subject,
        } = &expected_store
        else {
            return None;
        };
        (ordinal != 0
            && *store_tag == tag
            && *round == manifest.round
            && *subject == manifest.subject)
            .then_some(Self {
                ordinal,
                kind: CertifiedBodyPipelineColdReplayKindV1::BodyAvailable {
                    tag,
                    manifest,
                    expected_store,
                },
            })
    }

    /// Seal one terminal Store input and its exact live Validate effect.
    pub(in crate::sumeragi) fn body_stored(
        ordinal: u128,
        tag: reducer::EventTag,
        durable: DurableBodyReceipt,
        expected_validate: AdapterEffect,
    ) -> Option<Self> {
        let AdapterEffect::ValidateBody {
            tag: validate_tag,
            round,
            subject,
        } = &expected_validate
        else {
            return None;
        };
        (ordinal != 0
            && *validate_tag == tag
            && *round == durable.round()
            && *subject == durable.subject())
        .then_some(Self {
            ordinal,
            kind: CertifiedBodyPipelineColdReplayKindV1::BodyStored {
                tag,
                durable,
                expected_validate,
            },
        })
    }

    /// Return the terminal parent ordinal that orders this replay input.
    pub(in crate::sumeragi) const fn ordinal(&self) -> u128 {
        self.ordinal
    }

    #[cfg(test)]
    fn is_structurally_exact_for_test(&self) -> bool {
        match &self.kind {
            CertifiedBodyPipelineColdReplayKindV1::BodyAvailable {
                tag,
                manifest,
                expected_store,
            } => matches!(
                expected_store,
                AdapterEffect::StoreBody {
                    tag: store_tag,
                    round,
                    subject,
                } if store_tag == tag
                    && *round == manifest.round
                    && *subject == manifest.subject
            ),
            CertifiedBodyPipelineColdReplayKindV1::BodyStored {
                tag,
                durable,
                expected_validate,
            } => matches!(
                expected_validate,
                AdapterEffect::ValidateBody {
                    tag: validate_tag,
                    round,
                    subject,
                } if validate_tag == tag
                    && *round == durable.round()
                    && *subject == durable.subject()
            ),
        }
    }
}
/// Move-only Kura- and recovery-bound lifecycle storage authority.
///
/// Recovery is the sole production mint. It freezes the exact Kura instance,
/// context-addressed lifecycle/body roots, and authenticated block-signature
/// policy before any height-local store is opened. The owner factory consumes
/// the whole seal and never accepts those values independently.
#[must_use = "recovered lifecycle storage authority must enter the unified owner"]
pub(crate) struct RecoveredLifecycleStorageAuthorityV1 {
    kura_identity: KuraInstanceIdentity,
    genesis_account: AccountId,
    predecessor: Option<RecoveredLifecyclePredecessorStorageIdentityV1>,
    context_id: wire::HeightContextId,
    height: wire::Height,
    wal_path: PathBuf,
    lifecycle_root: PathBuf,
    body_store_root: PathBuf,
    signature_policy: super::v2_body_store::BlockSignaturePolicy,
    serve_payload_directory_authority:
        Option<crate::kura::KuraV2CertifiedServePayloadDirectoryAuthority>,
    successor_floor: Option<AuthenticatedRecoveredLifecycleSuccessorFloorV1>,
}
/// Exact predecessor address retained by one recovered successor storage seal.
struct RecoveredLifecyclePredecessorStorageIdentityV1 {
    context_id: wire::HeightContextId,
    height: wire::Height,
    lifecycle_root: PathBuf,
}
/// Kura-bound finalized predecessor floor awaiting its exact successor seal.
#[must_use = "the finalized lifecycle floor must bind recovered successor storage"]
pub(in crate::sumeragi) struct FinalizedLifecycleRetainedFloorV1 {
    kura_identity: KuraInstanceIdentity,
    published: PublishedFinalizedLifecycleRetainedFloorV1,
}
/// Closed H/H+1 target projected only by recovered lifecycle storage authority.
///
/// The lifecycle ledger consumes this token as a whole; no caller can supply a
/// raw root, context, or ordinal floor to successor initialization.
#[must_use = "the recovered lifecycle successor target must consume its finalized floor"]
pub(in crate::sumeragi) struct RecoveredLifecycleSuccessorFloorTargetV1 {
    predecessor: RecoveredLifecyclePredecessorStorageIdentityV1,
    successor_context: LifecycleContext,
    successor_root: PathBuf,
}
/// Move-only execution and storage inputs for the recovered lifecycle owner.
///
/// The authenticated adapter and runner-private dependency permit jointly mint
/// this cut. It consumes the recovered storage authority only after the
/// supplied State and Kura are the exact live instances for the recovered
/// network. No dependency or storage component can be projected back out.
#[must_use = "recovered lifecycle factory inputs must enter the unified owner"]
pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryInputsV1 {
    adapter_owner: Arc<AuthenticatedRecoveredAdapterFactoryOwnerV1>,
    storage: RecoveredLifecycleStorageAuthorityV1,
    state: Arc<crate::state::State>,
    queue: Arc<crate::queue::Queue>,
    kura: Arc<Kura>,
    provider_ingest_finalized_archive:
        Option<Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>>,
    reputation_finalized_archive:
        Option<Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>>,
    block_cadence: Duration,
    events_sender: crate::EventsSender,
    local_signer: KeyPair,
}
/// Opaque live-Kura binding retained after the storage authority is consumed.
///
/// Only the production storage factory can construct this seal. It exposes
/// fixed comparison oracles, never the underlying Kura identity or raw roots.
#[must_use = "the recovered Kura binding must remain inside its lifecycle owner"]
pub(crate) struct RecoveredLifecycleOwnerKuraBindingV1 {
    kura_identity: KuraInstanceIdentity,
    wal_path: PathBuf,
    local_signer: Option<PublicKey>,
}
/// Canonical launch paths projected only after the live Kura rejoins recovery.
#[must_use = "recovered launch storage paths must enter the sealed launch"]
pub(in crate::sumeragi) struct RecoveredLifecycleLaunchStoragePathsV1 {
    wal_path: PathBuf,
}
impl RecoveredLifecycleLaunchStoragePathsV1 {
    /// Borrow the exact recovery-derived safety-WAL path for adapter binding.
    pub(in crate::sumeragi) fn wal_path(&self) -> &std::path::Path {
        &self.wal_path
    }
}
impl RecoveredLifecycleOwnerKuraBindingV1 {
    /// Compare the retained recovery owner with one live Kura instance.
    pub(in crate::sumeragi) fn matches_kura(&self, kura: &Kura) -> bool {
        self.kura_identity.matches(kura)
    }
    /// Compare the recovery-bound Kura and local signer with one launch input.
    pub(in crate::sumeragi) fn matches_launch_identity(
        &self,
        kura: &Kura,
        key_pair: &KeyPair,
    ) -> bool {
        self.matches_kura(kura)
            && self
                .local_signer
                .as_ref()
                .is_some_and(|public_key| public_key == key_pair.public_key())
    }
    /// Compare the retained recovery owner with another sealed Kura identity.
    pub(in crate::sumeragi) fn matches_identity(&self, identity: &KuraInstanceIdentity) -> bool {
        self.kura_identity.same_instance(identity)
    }
    /// Project canonical height-local paths only for the exact live Kura.
    pub(in crate::sumeragi) fn storage_paths_for_launch(
        &self,
        kura: &Kura,
    ) -> Option<RecoveredLifecycleLaunchStoragePathsV1> {
        self.matches_kura(kura)
            .then(|| RecoveredLifecycleLaunchStoragePathsV1 {
                wal_path: self.wal_path.clone(),
            })
    }
    /// Join the just-retired LedgerV1 floor to this exact live Kura instance.
    pub(in crate::sumeragi) fn bind_finalized_lifecycle_floor(
        self,
        published: PublishedFinalizedLifecycleRetainedFloorV1,
    ) -> FinalizedLifecycleRetainedFloorV1 {
        FinalizedLifecycleRetainedFloorV1 {
            kura_identity: self.kura_identity,
            published,
        }
    }
    #[cfg(test)]
    /// Build a Kura binding for unlaunchable raw-root fixtures or exact-signer tests.
    pub(in crate::sumeragi) fn for_test(kura: &Kura, local_signer: Option<&KeyPair>) -> Self {
        Self {
            kura_identity: kura.instance_identity(),
            wal_path: kura
                .sumeragi_v2_storage_root()
                .join("wal")
                .join(format!("{:020}.wal", 1_u64)),
            local_signer: local_signer.map(|key_pair| key_pair.public_key().clone()),
        }
    }
}

impl RecoveredLifecycleSuccessorFloorTargetV1 {
    /// Compare the finalized frame with the exact predecessor retained by H+1.
    pub(in crate::sumeragi) fn authorizes_predecessor(
        &self,
        root: &Path,
        context: LifecycleContext,
    ) -> bool {
        self.predecessor.lifecycle_root == root
            && self.predecessor.context_id.0.as_ref() == context.id().as_bytes()
            && self.predecessor.height == context.height()
            && self.predecessor.height.checked_add(1) == Some(self.successor_context.height())
            && self.predecessor.context_id.0.as_ref() != self.successor_context.id().as_bytes()
            && self.predecessor.lifecycle_root != self.successor_root
            && self.predecessor.lifecycle_root.parent() == self.successor_root.parent()
    }

    /// Consume the already-authenticated target into its canonical successor.
    pub(in crate::sumeragi) fn into_successor_target(self) -> (PathBuf, LifecycleContext) {
        (self.successor_root, self.successor_context)
    }
}

impl RecoveredLifecycleStorageAuthorityV1 {
    fn predecessor_storage_identity(
        storage_root: &Path,
        verified: &VerifiedHeightContext,
    ) -> Option<RecoveredLifecyclePredecessorStorageIdentityV1> {
        verified.verified_predecessor_context().map(|context| {
            RecoveredLifecyclePredecessorStorageIdentityV1 {
                context_id: context.id(),
                height: context.height,
                lifecycle_root: storage_root
                    .join("lifecycle-v1")
                    .join(hex::encode(context.id().0.as_ref())),
            }
        })
    }

    /// Mint the sole production storage seal from authenticated height recovery.
    pub(in crate::sumeragi) fn mint_from_recovered_height(
        kura: &Kura,
        verified: &VerifiedHeightContext,
        signature_policy: &super::v2_body_store::BlockSignaturePolicy,
        genesis_account: &AccountId,
        permit: super::v2_recovery::RecoveredLifecycleStorageMintPermitV1,
    ) -> Result<Self, crate::kura::Error> {
        assert!(permit.authorizes(kura, verified, signature_policy, genesis_account));
        let storage_root = kura.sumeragi_v2_storage_root();
        let context = verified.context();
        let serve_payload_directory_authority = if kura.emergency_fast_startup_enabled() {
            None
        } else {
            Some(kura.mint_v2_certified_serve_payload_directory_authority(context)?)
        };
        Ok(Self {
            kura_identity: kura.instance_identity(),
            genesis_account: genesis_account.clone(),
            predecessor: Self::predecessor_storage_identity(&storage_root, verified),
            context_id: context.id(),
            height: context.height,
            wal_path: storage_root
                .join("wal")
                .join(format!("{:020}.wal", context.height)),
            lifecycle_root: storage_root
                .join("lifecycle-v1")
                .join(hex::encode(context.id().0.as_ref())),
            body_store_root: storage_root.join("bodies"),
            signature_policy: signature_policy.clone(),
            serve_payload_directory_authority,
            successor_floor: None,
        })
    }
    /// Bind the exact finalized H floor, initialize/authenticate H+1, and
    /// retain that frame proof until the production coordinator opens it.
    pub(in crate::sumeragi) fn bind_finalized_predecessor_floor(
        mut self,
        floor: FinalizedLifecycleRetainedFloorV1,
    ) -> Result<Self, LifecycleLedgerError> {
        if self.successor_floor.is_some()
            || !self.kura_identity.same_instance(&floor.kura_identity)
            || !matches!(
                &self.signature_policy,
                super::v2_body_store::BlockSignaturePolicy::RotatingLeader
            )
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "finalized lifecycle floor changed its recovered Kura or policy".to_owned(),
            ));
        }
        let predecessor = self.predecessor.take().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered successor storage has no authenticated predecessor".to_owned(),
            )
        })?;
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(self.context_id.0.as_ref());
        let target = RecoveredLifecycleSuccessorFloorTargetV1 {
            predecessor,
            successor_context: LifecycleContext::new(LifecycleDigest::new(context_id), self.height),
            successor_root: self.lifecycle_root.clone(),
        };
        self.successor_floor = Some(floor.published.initialize_successor(target)?);
        Ok(self)
    }
    /// Build the exact sealed storage input for a production-factory fixture.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        kura: &Kura,
        verified: &VerifiedHeightContext,
        signature_policy: super::v2_body_store::BlockSignaturePolicy,
        genesis_account: AccountId,
    ) -> Self {
        let permit = super::v2_recovery::RecoveredLifecycleStorageMintPermitV1::for_test(
            kura,
            verified,
            &signature_policy,
            &genesis_account,
        );
        Self::mint_from_recovered_height(
            kura,
            verified,
            &signature_policy,
            &genesis_account,
            permit,
        )
        .expect("mint fixture Certified-Serve payload directory authority")
    }
}
#[allow(variant_size_differences)]
enum ProductionLifecycleAdapterStartupStateV1 {
    Recovered {
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        pending_kura_apply: Option<RecoveredPendingKuraApplyReplayV1>,
        local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>,
        leader_wire_launch_prepared: bool,
    },
    #[cfg(test)]
    Fixture,
}
/// Move-only adapter projection for opening the adjacent leader-wire gate.
///
/// The wrapper retains the descriptor-bound WAL sibling authority together
/// with the exact replay cut, node owner, producer terminals, and restored
/// producer ordinal. Callers cannot assemble those independently or reuse them
/// for another gate.
#[must_use = "leader-wire launch authority must open the adjacent durable gate"]
pub(in crate::sumeragi) struct ProductionLeaderWireLaunchAuthorityV1 {
    storage: SafetyWalLeaderWireStoreAuthority,
    owner: [u8; 32],
    recovery_authority: LeaderWireRecoveryAuthority,
    producer_terminals: Vec<ProducerContinuationTerminalToken>,
    restored_producer_ordinal_high_watermark: Option<u128>,
}
impl ProductionLeaderWireLaunchAuthorityV1 {
    /// Largest producer ordinal authenticated by the adapter's adjacent store.
    pub(in crate::sumeragi) const fn restored_producer_ordinal_high_watermark(
        &self,
    ) -> Option<u128> {
        self.restored_producer_ordinal_high_watermark
    }
    /// Consume the complete adapter projection into one canonical gate open.
    pub(in crate::sumeragi) fn open_gate(
        self,
        context: &wire::HeightContext,
        body_store: &super::v2_body_store::V2BodyStore,
    ) -> Result<
        (
            Arc<LeaderWireLifecycleStoreGate>,
            LeaderWireLifecycleRestore,
            LeaderWireRecoveryAuthority,
        ),
        String,
    > {
        if !body_store.matches_context(context) {
            return Err("leader-wire body store changed its sealed height context".to_owned());
        }
        let durable_bodies = body_store
            .recovery_catalog()
            .map_err(|error| error.to_string())?
            .into_values()
            .map(|(_, receipt)| receipt)
            .collect::<Vec<_>>();
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunk_count = context.da_layout.max_chunk_count;
        let capacity =
            LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunk_count)?;
        let (gate, restore) = LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(
            self.storage,
            context.id(),
            context.height,
            self.owner,
            roster,
            capacity,
            max_chunk_count,
            self.recovery_authority,
            &self.producer_terminals,
            &durable_bodies,
        )?;
        Ok((gate, restore, self.recovery_authority))
    }
}
impl ProductionLifecycleAdapterStartupV1 {
    fn recovered(adapter: SumeragiV2Adapter, effects: Vec<AdapterEffect>) -> Self {
        Self {
            state: ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter,
                effects,
                pending_kura_apply: None,
                local_proposal_attempt: None,
                leader_wire_launch_prepared: false,
            },
        }
    }

    fn recovered_with_local_proposal_attempt(
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>,
    ) -> Self {
        Self {
            state: ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter,
                effects,
                pending_kura_apply: None,
                local_proposal_attempt,
                leader_wire_launch_prepared: false,
            },
        }
    }
    /// Replay the exact terminal ordinary certified-body prefix retained by
    /// LedgerV1 before any Store or Validate successor is exposed as Ready.
    ///
    /// Each input is prepared on cloned reducer state and must apply with the
    /// exact effect already authenticated by the lifecycle/body-store join.
    /// Busy, stale, duplicate, missing-work, or effect-shape outcomes are all
    /// startup-fatal: accepting any of them would publish a concrete carrier
    /// whose reducer predecessor was not reconstructed in this process.
    pub(in crate::sumeragi) fn replay_certified_body_pipeline(
        self,
        steps: &[CertifiedBodyPipelineColdReplayStepV1],
    ) -> Result<Self, &'static str> {
        if steps
            .windows(2)
            .any(|pair| pair[0].order_key() >= pair[1].order_key())
        {
            return Err("certified body cold replay steps are not canonically ordered");
        }
        if steps.is_empty() {
            return Ok(self);
        }
        let (
            mut adapter,
            effects,
            pending_kura_apply,
            local_proposal_attempt,
            leader_wire_launch_prepared,
        ) = match self.state {
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter,
                effects,
                pending_kura_apply,
                local_proposal_attempt,
                leader_wire_launch_prepared,
            } => (
                adapter,
                effects,
                pending_kura_apply,
                local_proposal_attempt,
                leader_wire_launch_prepared,
            ),
            #[cfg(test)]
            ProductionLifecycleAdapterStartupStateV1::Fixture => {
                if steps
                    .iter()
                    .all(CertifiedBodyPipelineColdReplayStepV1::is_structurally_exact_for_test)
                {
                    return Ok(Self {
                        state: ProductionLifecycleAdapterStartupStateV1::Fixture,
                    });
                }
                return Err("fixture adapter cannot replay certified body pipeline");
            }
        };
        if !effects.is_empty()
            || leader_wire_launch_prepared
            || adapter.pending_persistence_id.is_some()
            || !adapter.deferred_completions.is_empty()
            || !adapter.deferred_progress_inputs.is_empty()
            || !adapter.deferred_inputs.is_empty()
            || adapter.ensure_ingress().is_err()
        {
            return Err("certified body cold replay adapter is not pristine");
        }
        for step in steps {
            match &step.kind {
                CertifiedBodyPipelineColdReplayKindV1::BodyAvailable {
                    tag,
                    manifest,
                    expected_store,
                } => {
                    let prepared = adapter
                        .prepare_direct_certified_body_available(*tag, manifest)
                        .map_err(|_| "certified body cold BodyAvailable replay failed")?;
                    let DirectCertifiedBodyAvailablePreparation::Applied(prepared) = prepared
                    else {
                        return Err("certified body cold BodyAvailable replay did not apply");
                    };
                    if prepared.store_effect() != expected_store {
                        return Err("certified body cold BodyAvailable emitted a foreign Store");
                    }
                    let emitted = prepared.commit();
                    if emitted != *expected_store {
                        return Err("certified body cold BodyAvailable commit changed Store");
                    }
                }
                CertifiedBodyPipelineColdReplayKindV1::BodyStored {
                    tag,
                    durable,
                    expected_validate,
                } => {
                    let prepared = adapter
                        .prepare_direct_body_stored(
                            *tag,
                            durable.round(),
                            durable.subject(),
                            durable,
                        )
                        .map_err(|_| "certified body cold BodyStored replay failed")?;
                    let DirectBodyStoredPreparation::Applied(prepared) = prepared else {
                        return Err("certified body cold BodyStored replay did not apply");
                    };
                    if prepared.validate_effect() != expected_validate {
                        return Err("certified body cold BodyStored emitted a foreign Validate");
                    }
                    let emitted = prepared.commit();
                    if emitted != *expected_validate {
                        return Err("certified body cold BodyStored commit changed Validate");
                    }
                }
            }
        }
        Ok(Self {
            state: ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter,
                effects,
                pending_kura_apply,
                local_proposal_attempt,
                leader_wire_launch_prepared,
            },
        })
    }

    /// Exercise the sealed certified-body replay join without exposing the
    /// production startup state enum to focused descendant tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn replay_certified_body_pipeline_for_test(
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        steps: &[CertifiedBodyPipelineColdReplayStepV1],
    ) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), &'static str> {
        let replayed = Self::recovered(adapter, effects).replay_certified_body_pipeline(steps)?;
        match replayed.state {
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter,
                effects,
                pending_kura_apply: None,
                local_proposal_attempt: None,
                leader_wire_launch_prepared: false,
            } => Ok((adapter, effects)),
            ProductionLifecycleAdapterStartupStateV1::Recovered { .. }
            | ProductionLifecycleAdapterStartupStateV1::Fixture => {
                Err("certified-body test replay retained foreign startup ownership")
            }
        }
    }

    /// Compare the sealed adapter startup with one exact verified owner.
    pub(in crate::sumeragi) fn authorizes_verified_context(
        &self,
        verified: &VerifiedHeightContext,
    ) -> bool {
        match &self.state {
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter, effects, ..
            } => {
                effects.is_empty()
                    && &adapter.wire_context == verified.context()
                    && adapter.proofs_of_possession.as_slice() == verified.proofs_of_possession()
                    && adapter.current_tag().height() == verified.context().height
            }
            #[cfg(test)]
            ProductionLifecycleAdapterStartupStateV1::Fixture => true,
        }
    }
    /// Preview one already-fsynced Broadcast-plus-next-Sign crash cut.
    ///
    /// WAL recovery has authenticated the historical unsigned request and its
    /// signed Broadcast. This consuming preview independently verifies the
    /// signature and replays the reducer on clones, accepting only the exact
    /// `Broadcast` then `Sign(Vote)` successor. The original cold adapter stays
    /// unchanged and sealed inside the result until the next Vote's durable
    /// body and WAL owner have also rejoined.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_signed_broadcast_and_sign(
        self,
        verified: &VerifiedHeightContext,
        authority: RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1,
    ) -> Result<PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1, &'static str> {
        let ProductionLifecycleAdapterStartupStateV1::Recovered {
            adapter,
            effects,
            pending_kura_apply: None,
            local_proposal_attempt,
            leader_wire_launch_prepared: false,
        } = self.state
        else {
            return Err("recovered Broadcast-and-Sign preview startup is not pristine");
        };
        if !effects.is_empty()
            || &adapter.wire_context != verified.context()
            || adapter.proofs_of_possession.as_slice() != verified.proofs_of_possession()
            || adapter.current_tag().height() != verified.context().height
            || adapter.pending_persistence_id.is_some()
            || !adapter.deferred_completions.is_empty()
            || !adapter.deferred_progress_inputs.is_empty()
            || !adapter.deferred_inputs.is_empty()
            || adapter.ensure_ingress().is_err()
        {
            return Err("recovered Broadcast-and-Sign preview changed its exact context");
        }
        let RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1 {
            tag,
            request,
            signature,
            broadcast,
        } = authority;
        if adapter.current_tag() != tag {
            return Err("recovered Broadcast-and-Sign preview tag changed across restart");
        }
        let AdapterEffect::Broadcast(message) = &broadcast else {
            return Err("recovered Broadcast-and-Sign preview lost its signed Broadcast");
        };
        verified
            .verify_consensus_message(message)
            .map_err(|_| "recovered Broadcast-and-Sign preview signature is not authorized")?;
        let mut next_registry = adapter.registry.clone();
        let awaiting = adapter.reducer.awaiting_signature().ok_or(
            "recovered Broadcast-and-Sign preview is not awaiting its historical signature",
        )?;
        let awaiting_request = match awaiting {
            reducer::SignableMessage::Proposal(proposal) => SignRequest::Proposal(
                next_registry
                    .unsigned_proposal_to_wire(proposal, adapter.aggregator.as_ref())
                    .map_err(|_| {
                        "recovered Broadcast-and-Sign preview Proposal could not be reconstructed"
                    })?,
            ),
            reducer::SignableMessage::Vote(vote) => SignRequest::Vote(
                next_registry
                    .unsigned_vote_to_wire(*vote)
                    .map_err(|_| {
                        "recovered Broadcast-and-Sign preview vote could not be reconstructed"
                    })?,
            ),
            reducer::SignableMessage::TimeoutVote(_) => {
                return Err("recovered Broadcast-and-Sign preview cannot start from timeout Sign");
            }
        };
        if awaiting_request != request {
            return Err("recovered Broadcast-and-Sign preview request changed across restart");
        }
        let event = reducer::Event::Signed {
            tag,
            signature: reducer::OpaqueSignature::new(signature),
        };
        let mut next_reducer = adapter.reducer.clone();
        let outcome = next_reducer
            .step(event)
            .map_err(|_| "recovered Broadcast-and-Sign preview reducer replay failed")?;
        if outcome.disposition() != reducer::StepDisposition::Applied {
            return Err("recovered Broadcast-and-Sign preview reducer did not apply");
        }
        let core_effects = outcome.into_effects();
        let [
            reducer::Effect::Broadcast(replayed_message),
            reducer::Effect::Sign {
                tag: replayed_next_tag,
                message: reducer::SignableMessage::Vote(replayed_next_vote),
            },
        ] = core_effects.as_slice()
        else {
            return Err("recovered Broadcast-and-Sign preview has another reducer successor");
        };
        let replayed_broadcast = AdapterEffect::Broadcast(
            next_registry
                .message_to_wire(replayed_message.clone(), adapter.aggregator.as_ref())
                .map_err(|_| "recovered Broadcast-and-Sign preview wire projection failed")?,
        );
        let next_sign = AdapterEffect::Sign {
            tag: *replayed_next_tag,
            request: SignRequest::Vote(
                next_registry
                    .unsigned_vote_to_wire(*replayed_next_vote)
                    .map_err(|_| {
                        "recovered Broadcast-and-Sign preview next Vote could not be reconstructed"
                    })?,
            ),
        };
        if replayed_broadcast != broadcast
            || next_reducer.pending_persistence_record().is_some()
            || next_reducer.awaiting_signature()
                != Some(&reducer::SignableMessage::Vote(*replayed_next_vote))
        {
            return Err("recovered Broadcast-and-Sign preview children changed across restart");
        }
        let expected_proposal_manifest_hash = match &message.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                Some(HashOf::new(&proposal.manifest))
            }
            wire::ConsensusMessageV2Payload::Vote(vote)
                if vote.phase == wire::GlobalPhase::Prepare =>
            {
                None
            }
            _ => {
                return Err(
                    "recovered Broadcast-and-Sign preview parent is not Proposal or Prepare",
                );
            }
        };
        Ok(
            PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1 {
                startup: Self::recovered_with_local_proposal_attempt(
                    adapter,
                    effects,
                    local_proposal_attempt,
                ),
                broadcast,
                next_sign,
                expected_proposal_manifest_hash,
                body_store_identity: None,
                cold_proposal_output: None,
                body_lookup_minted: false,
            },
        )
    }
    /// Rebuild the reducer's already-fsynced recovered Fetch-to-Store crash cut.
    ///
    /// The adapter transition is installed only after the opaque body and WAL
    /// Fetch project the exact Store candidate found in LedgerV1. No ordinary
    /// runtime work identity or free-standing effect leaves this method.
    pub(in crate::sumeragi) fn advance_recovered_decision_fetch_store(
        self,
        verified: &VerifiedHeightContext,
        fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
        body: super::v2_body_store::RecoveredDecisionFetchStoreBodyAuthorityV1,
    ) -> Result<(Self, RecoveredDecisionFetchStoreProjectionV1), &'static str> {
        let ProductionLifecycleAdapterStartupStateV1::Recovered {
            mut adapter,
            effects,
            pending_kura_apply: None,
            local_proposal_attempt: None,
            leader_wire_launch_prepared: false,
        } = self.state
        else {
            return Err("recovered Decision Store adapter startup is not pristine");
        };
        if !effects.is_empty()
            || &adapter.wire_context != verified.context()
            || adapter.proofs_of_possession.as_slice() != verified.proofs_of_possession()
            || adapter.current_tag().height() != verified.context().height
        {
            return Err("recovered Decision Store adapter startup changed its exact context");
        }
        let projection_body = body.clone();
        let authority = fetch
            .project_store_adapter_authority(body)
            .ok_or("recovered Decision Store body does not bind the WAL Fetch")?;
        let preview = adapter
            .prepare_recovered_decision_fetch_store(authority)
            .map_err(|_| "recovered Decision Store reducer preview is inconsistent")?;
        let store = fetch
            .project_decision_fetch_store(verified, projection_body, preview.store_effect())
            .ok_or("recovered Decision Store projection is inconsistent")?;
        preview.commit_after_durable_settlement();
        Ok((Self::recovered(adapter, effects), store))
    }
}
include!("v2_recovered_decision_validate_adapter_startup.rs");
impl ProductionLifecycleAdapterStartupV1 {
    /// Rebuild one already-fsynced Vote/Timeout `Signed` transition.
    ///
    /// The cold authority has already joined the WAL Sign, its exact
    /// Sign-to-Broadcast LedgerV1 continuation, and the verified roster. This
    /// method independently replays the reducer on cloned state and accepts
    /// only the single-Broadcast shape which the current durable transaction
    /// can recover. No status, WAL, output, or worker acknowledgement occurs.
    pub(in crate::sumeragi) fn advance_recovered_lifecycle_signed_broadcast(
        self,
        verified: &VerifiedHeightContext,
        authority: RecoveredLifecycleSignColdAdapterAuthorityV1,
    ) -> Result<Self, &'static str> {
        let ProductionLifecycleAdapterStartupStateV1::Recovered {
            mut adapter,
            effects,
            pending_kura_apply: None,
            local_proposal_attempt,
            leader_wire_launch_prepared: false,
        } = self.state
        else {
            return Err("recovered signed Broadcast adapter startup is not pristine");
        };
        if !effects.is_empty()
            || &adapter.wire_context != verified.context()
            || adapter.proofs_of_possession.as_slice() != verified.proofs_of_possession()
            || adapter.current_tag().height() != verified.context().height
            || adapter.pending_persistence_id.is_some()
            || !adapter.deferred_completions.is_empty()
            || !adapter.deferred_progress_inputs.is_empty()
            || !adapter.deferred_inputs.is_empty()
        {
            return Err("recovered signed Broadcast adapter startup changed its exact context");
        }
        let RecoveredLifecycleSignColdAdapterAuthorityV1 {
            tag,
            request,
            signature,
            broadcast,
        } = authority;
        if adapter.ensure_ingress().is_err() || adapter.current_tag() != tag {
            return Err("recovered signed Broadcast tag no longer matches the reducer");
        }
        let signer = match &request {
            SignRequest::Vote(vote) => vote.signer,
            SignRequest::TimeoutVote(vote) => vote.signer,
            SignRequest::Proposal(_) => {
                // Proposal recovery is owned by the specialized path which
                // authenticates its body and fsyncs the Prepare-WAL successor;
                // the generic signed-Broadcast path must reject it.
                return Err("Proposal cold replay requires its body and Prepare WAL successor");
            }
        };
        let local_signer = adapter
            .reducer
            .local_validator()
            .map(|validator| adapter.registry.validator_index(validator))
            .transpose()
            .map_err(|_| "recovered signed Broadcast local signer is inconsistent")?
            .ok_or("recovered signed Broadcast has no local signer")?;
        if signer != local_signer
            || verify_individual_signature(
                &adapter.wire_context,
                signer,
                &signature,
                &request.signature_preimage(),
            )
            .is_err()
        {
            return Err("recovered signed Broadcast signature is not locally authorized");
        }
        let mut next_registry = adapter.registry.clone();
        let awaiting = adapter
            .reducer
            .awaiting_signature()
            .ok_or("recovered signed Broadcast reducer is not awaiting a signature")?;
        let awaiting_request = match awaiting {
            reducer::SignableMessage::Vote(vote) => SignRequest::Vote(
                next_registry
                    .unsigned_vote_to_wire(*vote)
                    .map_err(|_| "recovered signed vote could not be reconstructed")?,
            ),
            reducer::SignableMessage::TimeoutVote(vote) => SignRequest::TimeoutVote(
                next_registry
                    .unsigned_timeout_vote_to_wire(vote, adapter.aggregator.as_ref())
                    .map_err(|_| "recovered signed timeout could not be reconstructed")?,
            ),
            reducer::SignableMessage::Proposal(_) => {
                return Err("Proposal cold replay requires its body and Prepare WAL successor");
            }
        };
        if awaiting_request != request {
            return Err("recovered signed Broadcast request changed across restart");
        }
        let event = reducer::Event::Signed {
            tag,
            signature: reducer::OpaqueSignature::new(signature),
        };
        let mut next_reducer = adapter.reducer.clone();
        let outcome = next_reducer
            .step(event.clone())
            .map_err(|_| "recovered signed Broadcast reducer replay failed")?;
        if outcome.disposition() != reducer::StepDisposition::Applied {
            return Err("recovered signed Broadcast reducer did not apply");
        }
        let core_effects = outcome.into_effects();
        let [reducer::Effect::Broadcast(message)] = core_effects.as_slice() else {
            return Err("recovered signed Broadcast has another reducer successor");
        };
        let replayed = AdapterEffect::Broadcast(
            next_registry
                .message_to_wire(message.clone(), adapter.aggregator.as_ref())
                .map_err(|_| "recovered signed Broadcast wire projection failed")?,
        );
        if replayed != broadcast
            || next_reducer.pending_persistence_record().is_some()
            || next_reducer.awaiting_signature().is_some()
        {
            return Err("recovered signed Broadcast is not the exact single-child successor");
        }
        let next_fence = ReducerFenceProjection {
            pending_persistence: None,
            awaiting_signature: None,
            replay_complete: adapter.replay_complete,
        };
        let next_fence_generation = if next_fence == adapter.reducer_fence_projection() {
            adapter.reducer_fence_generation
        } else {
            adapter
                .reducer_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or("recovered signed Broadcast exhausted reducer fence generation")?
        };
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &core_effects);
        adapter.log_body_progress(
            &event,
            reducer::StepDisposition::Applied,
            core_effects.len(),
        );
        Ok(Self::recovered_with_local_proposal_attempt(
            adapter,
            effects,
            local_proposal_attempt,
        ))
    }
    /// Rejoin an already-fsynced Broadcast-plus-next-Sign pair to the cold adapter.
    ///
    /// WAL recovery has already authenticated both durable children and retained
    /// their executable registry authority. The cold reducer still awaits the
    /// historical signature whose worker result was lost at the crash. This
    /// method independently verifies that signature, replays it on cloned state,
    /// and accepts only the exact `Broadcast`-then-`Sign(Vote)` successor already
    /// present in LedgerV1. It publishes no status or output and mutates no WAL.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn advance_recovered_lifecycle_signed_broadcast_and_sign(
        self,
        verified: &VerifiedHeightContext,
        authority: RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1,
    ) -> Result<Self, &'static str> {
        let ProductionLifecycleAdapterStartupStateV1::Recovered {
            adapter,
            effects,
            pending_kura_apply: None,
            local_proposal_attempt,
            leader_wire_launch_prepared: false,
        } = self.state
        else {
            return Err("recovered Broadcast-and-Sign adapter startup is not pristine");
        };
        if !effects.is_empty()
            || &adapter.wire_context != verified.context()
            || adapter.proofs_of_possession.as_slice() != verified.proofs_of_possession()
            || adapter.current_tag().height() != verified.context().height
            || adapter.pending_persistence_id.is_some()
            || !adapter.deferred_completions.is_empty()
            || !adapter.deferred_progress_inputs.is_empty()
            || !adapter.deferred_inputs.is_empty()
            || adapter.ensure_ingress().is_err()
        {
            return Err("recovered Broadcast-and-Sign adapter startup changed its exact context");
        }
        let RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1 {
            broadcast,
            next_sign,
        } = authority;
        let AdapterEffect::Broadcast(message) = &broadcast else {
            return Err("recovered Broadcast-and-Sign authority lost its Broadcast");
        };
        verified.verify_consensus_message(message).map_err(
            |_| "recovered Broadcast-and-Sign message failed frozen-roster verification",
        )?;
        let (request, signature) = match &message.payload {
            wire::ConsensusMessageV2Payload::Proposal(signed) => {
                let mut unsigned = signed.clone();
                let signature = core::mem::take(&mut unsigned.signature);
                (SignRequest::Proposal(unsigned), signature)
            }
            wire::ConsensusMessageV2Payload::Vote(signed) => {
                let mut unsigned = signed.clone();
                let signature = core::mem::take(&mut unsigned.signature);
                (SignRequest::Vote(unsigned), signature)
            }
            _ => {
                return Err(
                    "recovered Broadcast-and-Sign message is not a Proposal or Prepare vote",
                );
            }
        };
        let AdapterEffect::Sign {
            request: SignRequest::Vote(_),
            ..
        } = &next_sign
        else {
            return Err("recovered Broadcast-and-Sign authority lost its next Vote Sign");
        };
        let mut next_registry = adapter.registry.clone();
        let awaiting = adapter.reducer.awaiting_signature().ok_or(
            "recovered Broadcast-and-Sign reducer is not awaiting its historical signature",
        )?;
        let awaiting_request = match awaiting {
            reducer::SignableMessage::Proposal(proposal) => SignRequest::Proposal(
                next_registry
                    .unsigned_proposal_to_wire(proposal, adapter.aggregator.as_ref())
                    .map_err(
                        |_| "recovered Broadcast-and-Sign Proposal could not be reconstructed",
                    )?,
            ),
            reducer::SignableMessage::Vote(vote) => SignRequest::Vote(
                next_registry
                    .unsigned_vote_to_wire(*vote)
                    .map_err(|_| "recovered Broadcast-and-Sign vote could not be reconstructed")?,
            ),
            reducer::SignableMessage::TimeoutVote(_) => {
                return Err("recovered Broadcast-and-Sign cannot start from a timeout Sign");
            }
        };
        if awaiting_request != request {
            return Err("recovered Broadcast-and-Sign historical request changed across restart");
        }
        let event = reducer::Event::Signed {
            tag: adapter.current_tag(),
            signature: reducer::OpaqueSignature::new(signature),
        };
        let mut next_reducer = adapter.reducer.clone();
        let outcome = next_reducer
            .step(event.clone())
            .map_err(|_| "recovered Broadcast-and-Sign reducer replay failed")?;
        if outcome.disposition() != reducer::StepDisposition::Applied {
            return Err("recovered Broadcast-and-Sign reducer did not apply");
        }
        let core_effects = outcome.into_effects();
        let [
            reducer::Effect::Broadcast(replayed_message),
            reducer::Effect::Sign {
                tag: replayed_next_tag,
                message: reducer::SignableMessage::Vote(replayed_next_vote),
            },
        ] = core_effects.as_slice()
        else {
            return Err("recovered Broadcast-and-Sign has another reducer successor");
        };
        let replayed_broadcast = AdapterEffect::Broadcast(
            next_registry
                .message_to_wire(replayed_message.clone(), adapter.aggregator.as_ref())
                .map_err(|_| "recovered Broadcast-and-Sign wire projection failed")?,
        );
        let replayed_next_sign = AdapterEffect::Sign {
            tag: *replayed_next_tag,
            request: SignRequest::Vote(
                next_registry
                    .unsigned_vote_to_wire(*replayed_next_vote)
                    .map_err(|_| "recovered next Vote Sign could not be reconstructed")?,
            ),
        };
        if replayed_broadcast != broadcast
            || replayed_next_sign != next_sign
            || next_reducer.pending_persistence_record().is_some()
            || next_reducer.awaiting_signature()
                != Some(&reducer::SignableMessage::Vote(*replayed_next_vote))
        {
            return Err("recovered Broadcast-and-Sign children changed across restart");
        }
        let next_fence = ReducerFenceProjection {
            pending_persistence: None,
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: adapter.replay_complete,
        };
        let next_fence_generation = if next_fence == adapter.reducer_fence_projection() {
            adapter.reducer_fence_generation
        } else {
            adapter
                .reducer_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or("recovered Broadcast-and-Sign exhausted reducer fence generation")?
        };
        let mut adapter = adapter;
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &core_effects);
        adapter.log_body_progress(
            &event,
            reducer::StepDisposition::Applied,
            core_effects.len(),
        );
        Ok(Self::recovered_with_local_proposal_attempt(
            adapter,
            effects,
            local_proposal_attempt,
        ))
    }
    /// Seal every adapter-owned input required by the adjacent gate open.
    ///
    /// The comparison occurs before the owner opens any lifecycle store. It is
    /// repeated here so launch cannot move the adapter across a substituted
    /// WAL target even if its retained storage seal were corrupted in memory.
    pub(in crate::sumeragi) fn prepare_leader_wire_launch(
        &mut self,
        expected_wal_path: &std::path::Path,
    ) -> Result<ProductionLeaderWireLaunchAuthorityV1, &'static str> {
        match &mut self.state {
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter,
                effects,
                leader_wire_launch_prepared,
                ..
            } if effects.is_empty()
                && !*leader_wire_launch_prepared
                && adapter.wal.matches_path(expected_wal_path) =>
            {
                let storage = adapter
                    .mint_leader_wire_store_authority(expected_wal_path)
                    .map_err(|_| "adapter could not bind its safety-WAL adjacent directory")?;
                let recovery_authority = adapter
                    .leader_wire_recovery_authority()
                    .map_err(|_| "adapter could not derive its leader-wire replay authority")?;
                let launch = ProductionLeaderWireLaunchAuthorityV1 {
                    storage,
                    owner: adapter.fingerprints.node.into(),
                    recovery_authority,
                    producer_terminals: adapter.durable_producer_terminal_tokens(),
                    restored_producer_ordinal_high_watermark: adapter
                        .restored_producer_continuation_ordinal_high_watermark(),
                };
                *leader_wire_launch_prepared = true;
                Ok(launch)
            }
            ProductionLifecycleAdapterStartupStateV1::Recovered { effects, .. }
                if !effects.is_empty() =>
            {
                Err("adapter retained unadmitted startup effects")
            }
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                leader_wire_launch_prepared: true,
                ..
            } => Err("adapter leader-wire launch authority was already consumed"),
            ProductionLifecycleAdapterStartupStateV1::Recovered { .. } => {
                Err("adapter safety WAL changed its recovery-sealed path")
            }
            #[cfg(test)]
            ProductionLifecycleAdapterStartupStateV1::Fixture => {
                Err("fixture adapter cannot mint production leader-wire authority")
            }
        }
    }
    /// Consume one pristine recovered Apply startup into its exact runtime for
    /// executor-lineage non-substitution tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn into_lifecycle_apply_runtime_for_lineage_test(
        mut self,
        lifecycle_ordinals: crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource,
    ) -> crate::sumeragi::v2_runtime::SerializedV2Runtime {
        match &mut self.state {
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                effects,
                pending_kura_apply,
                local_proposal_attempt,
                leader_wire_launch_prepared,
                ..
            } if effects.is_empty()
                && pending_kura_apply.is_none()
                && local_proposal_attempt.is_none()
                && !*leader_wire_launch_prepared =>
            {
                *leader_wire_launch_prepared = true;
            }
            ProductionLifecycleAdapterStartupStateV1::Recovered { .. }
            | ProductionLifecycleAdapterStartupStateV1::Fixture => {
                panic!("lineage test requires one pristine recovered Apply startup")
            }
        }
        let (runtime, pending_kura_apply, local_proposal_attempt) = self
            .into_serialized_runtime(
                std::time::Instant::now(),
                std::time::Duration::from_secs(10),
                crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
                lifecycle_ordinals,
            )
            .expect("open exact recovered Apply runtime for lineage test");
        assert!(pending_kura_apply.is_none());
        assert!(local_proposal_attempt.is_none());
        runtime
    }
    #[cfg(test)]
    pub(in crate::sumeragi) const fn fixture_for_test() -> Self {
        Self {
            state: ProductionLifecycleAdapterStartupStateV1::Fixture,
        }
    }
    #[cfg(test)]
    pub(in crate::sumeragi) fn is_exact_for_test(&self) -> bool {
        match &self.state {
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter, effects, ..
            } => {
                effects.is_empty() && adapter.current_tag().height() == adapter.wire_context.height
            }
            ProductionLifecycleAdapterStartupStateV1::Fixture => true,
        }
    }
}
// The lifecycle completion transaction consumes the current recovered Sign,
// executes its reducer acknowledgement through this retained adapter, and
// authenticates only the next `awaiting_signature` emitted by that step. Queued
// signatures deliberately have no eager lifecycle token or pre-bound event tag.
struct ProductionRecoveredLifecycleOwnerAssemblyLinearity;
impl Drop for ProductionRecoveredLifecycleOwnerAssemblyLinearity {
    fn drop(&mut self) {}
}
/// One-shot proof that adapter startup and recovered lifecycle open remained paired.
///
/// Only the private combined startup wrapper in this module can mint this
/// permit. The lifecycle-owner constructor consumes it, so sibling modules
/// cannot combine an adapter from one recovered startup with another open.
pub(in crate::sumeragi) struct ProductionRecoveredLifecycleOwnerAssemblyPermitV1 {
    _linearity: ProductionRecoveredLifecycleOwnerAssemblyLinearity,
}
impl ProductionRecoveredLifecycleOwnerAssemblyPermitV1 {
    fn mint_paired() -> Self {
        Self {
            _linearity: ProductionRecoveredLifecycleOwnerAssemblyLinearity,
        }
    }
}
#[must_use = "the paired recovered startup must enter its lifecycle owner"]
struct ProductionRecoveredLifecycleOwnerStartupV1 {
    adapter_startup: ProductionLifecycleAdapterStartupV1,
    opened: RecoveredWalProductionOwnerOpenV1,
}
impl ProductionRecoveredLifecycleOwnerStartupV1 {
    fn into_owner(
        self,
        registry: LifecycleWorkRegistryHolder,
        payload_store: super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
        body_store: super::v2_body_store::V2BodyStore,
    ) -> Result<
        ProductionLifecycleOwnerV1,
        super::v2_lifecycle_coordinator::ProductionLifecycleStartupErrorV1,
    > {
        let Self {
            adapter_startup,
            opened,
        } = self;
        ProductionLifecycleOwnerV1::from_recovered_wal_open(
            ProductionRecoveredLifecycleOwnerAssemblyPermitV1::mint_paired(),
            opened,
            registry,
            payload_store,
            body_store,
            adapter_startup,
        )
    }
}
/// Owned startup failure from the sole first-release lifecycle-owner factory.
#[derive(Debug, Error)]
#[error("{kind}")]
#[must_use = "failed production lifecycle startup requires process restart"]
pub(crate) struct ProductionLifecycleOwnerStartupErrorV1 {
    kind: ProductionLifecycleOwnerStartupErrorKindV1,
}
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
enum ProductionLifecycleOwnerStartupErrorKindV1 {
    #[error("recovered adapter retained unadmitted startup effects")]
    ResidualEffects,
    #[error("recovered adapter safety WAL changed its Kura-derived storage path")]
    StorageLayout,
    #[error("recovered lifecycle successor ordinal floor failed: {0}")]
    SuccessorFloor(#[source] LifecycleLedgerError),
    #[error("recovered lifecycle execution dependencies changed identity")]
    ExecutionIdentity,
    #[error("recovered body marker replay failed: {0}")]
    MarkerReplay(#[source] super::v2_apply::V2ApplyError),
    #[error("recovered body-store handoff failed: {0}")]
    BodyStore(#[source] super::v2_body_store::V2BodyStoreError),
    #[error("Certified-Serve payload store open failed: {0}")]
    ServeStore(#[source] super::v2_certified_serve_payload_store::CertifiedServePayloadStoreError),
    #[error("Certified-Serve payload authentication failed: {0}")]
    ServeRecovery(
        #[source] super::v2_certified_serve_payload_store::CertifiedServePayloadRecoveryError,
    ),
    #[error("storage-only lifecycle owner open failed: {0}")]
    StorageOnly(#[source] super::v2_lifecycle_coordinator::ProductionLifecycleStartupErrorV1),
    #[error("recovered WAL parent authentication failed: {0}")]
    RecoveredParent(&'static str),
    #[error("recovered WAL lifecycle persistence failed: {0}")]
    Persist(&'static str),
    #[error("recovered WAL Sign installation failed: {0}")]
    SignInstall(&'static str),
    #[error("recovered WAL lifecycle open failed: {0}")]
    RecoveredOpen(&'static str),
    #[error("recovered WAL lifecycle owner join failed: {0}")]
    RecoveredOwner(#[source] super::v2_lifecycle_coordinator::ProductionLifecycleStartupErrorV1),
    #[error("recovered WAL control Sign startup failed: {0}")]
    RecoveredControl(&'static str),
    #[error("recovered local Proposal attempt startup failed: {0}")]
    RecoveredLocalProposal(&'static str),
    #[error("recovered WAL Decision Fetch startup failed: {0}")]
    RecoveredDecisionFetch(&'static str),
    #[error("recovered WAL Decision body preflight failed: {0}")]
    RecoveredDecisionBody(#[source] super::v2_body_store::RecoveredDecisionApplyBodyCutError),
    #[error("recovered WAL Decision Apply startup is unavailable: {0}")]
    RecoveredDecisionApply(&'static str),
}
impl ProductionLifecycleOwnerStartupErrorV1 {
    fn new(kind: ProductionLifecycleOwnerStartupErrorKindV1) -> Self {
        Self { kind }
    }
}
/// Startup cut whose recovered vote has joined one exact lifecycle repair.
///
/// The adapter and every remaining startup effect stay sealed beside the
/// repair. There is intentionally no parts/extraction API: a later composite
/// transaction must retain this wrapper through ledger fsync and concrete
/// child installation before normal startup can resume. Its validated parent
/// registry row has already been detached, and the lifetime-bound repair keeps
/// the registry exclusively borrowed until the splice completes. Every later
/// failure is fail-stop/restart rather than ordinary rollback.
#[cfg(test)]
#[must_use = "WAL lifecycle startup remains sealed until durable recovery completes"]
struct AuthenticatedRecoveredWalLifecycleStartup<'registry> {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
    repair: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
}
/// Storage-authenticated recovered-parent startup assembled without scheduler authority.
///
/// The exact opened LedgerV1 store/frame, adapter, unpublished effects, and
/// registry-borrowed repair remain one opaque
/// value. No raw candidate, effect, pending binding, receipt, ordinal, or
/// registry cut can be extracted.
#[allow(dead_code)]
#[must_use = "storage-authenticated WAL startup remains sealed until persistence"]
struct StorageAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
    ledger: OpenedRecoveredWalValidateLedger,
    repair: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
}
/// Opaque recovered-parent factory failure retaining the whole startup seal.
#[allow(dead_code)]
#[must_use = "failed recovered-parent startup still owns all recovery authority"]
struct StorageAuthenticatedRecoveredWalLifecycleStartupError<'body> {
    failure: StorageAuthenticatedRecoveredWalLifecycleStartupFailure<'body>,
}
#[allow(clippy::large_enum_variant, variant_size_differences)]
enum StorageAuthenticatedRecoveredWalLifecycleStartupFailure<'body> {
    MissingVote {
        _startup: AuthenticatedRecoveredAdapterStartup,
    },
    Factory {
        _adapter: SumeragiV2Adapter,
        _effects: Vec<AdapterEffect>,
        _error: RecoveredWalParentFactoryError<'body>,
    },
}
#[allow(dead_code)]
impl StorageAuthenticatedRecoveredWalLifecycleStartupError<'_> {
    /// Return one stable classification without exposing retained authority.
    fn reason(&self) -> &'static str {
        match &self.failure {
            StorageAuthenticatedRecoveredWalLifecycleStartupFailure::MissingVote { .. } => {
                "recovered startup has no phase-vote continuation"
            }
            StorageAuthenticatedRecoveredWalLifecycleStartupFailure::Factory { _error, .. } => {
                _error.reason()
            }
        }
    }
}
/// Exact-store startup after the recovered LedgerV1 repair is durable.
#[must_use = "the durable recovered startup must install its exact Sign"]
struct PersistedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
    persisted: PersistedRecoveredWalValidateLedger<'registry>,
}
/// Cold adapter replay completed before the repaired Sign is installed.
///
/// Keeping this as a separate consuming stage prevents the large cold-replay
/// result and the mutually exclusive registry-install result from sharing one
/// startup stack frame.
#[must_use = "the cold-prepared recovered startup must install its exact Sign"]
struct ColdPreparedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    adapter_startup: ProductionLifecycleAdapterStartupV1,
    verified: VerifiedHeightContext,
    persisted: PersistedRecoveredWalValidateLedger<'registry>,
}
/// Exact-store startup after Sign installation and before final recovery open.
#[must_use = "the installed recovered startup must enter its production owner"]
struct InstalledStorageAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    adapter_startup: ProductionLifecycleAdapterStartupV1,
    verified: VerifiedHeightContext,
    installed: InstalledRecoveredWalSignStorage<'registry>,
}
#[must_use = "failed exact-store persistence requires process restart"]
struct StorageRecoveredWalPersistError<'registry> {
    _adapter: SumeragiV2Adapter,
    _effects: Vec<AdapterEffect>,
    error: ExactStoreRecoveredWalPersistError<'registry>,
}
#[must_use = "failed exact-store Sign installation requires process restart"]
struct StorageRecoveredWalSignInstallError<'registry> {
    _startup: ProductionLifecycleAdapterStartupV1,
    error: ExactStoreRecoveredWalSignInstallError<'registry>,
}
/// Fail-stop recovered lifecycle-open diagnostic with no retry authority.
#[must_use = "failed exact-store lifecycle open requires process restart"]
struct StorageRecoveredWalOpenError<'registry> {
    failure: StorageRecoveredWalOpenFailure<'registry>,
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum StorageRecoveredWalOpenFailure<'registry> {
    Storage {
        error: ProductionRecoveredWalStorageError,
    },
    OwnerSeal {
        _opened: ProductionOpenedRecoveredWalSignLifecycleCut<'registry>,
    },
}
impl StorageRecoveredWalPersistError<'_> {
    fn reason(&self) -> &'static str {
        self.error.reason()
    }
}
impl StorageRecoveredWalSignInstallError<'_> {
    fn reason(&self) -> &'static str {
        self.error.reason()
    }
}
impl StorageRecoveredWalOpenError<'_> {
    fn reason(&self) -> &'static str {
        match &self.failure {
            StorageRecoveredWalOpenFailure::Storage { error, .. } => error.reason(),
            StorageRecoveredWalOpenFailure::OwnerSeal { .. } => {
                "opened recovered lifecycle lost its exact owner seal"
            }
        }
    }
}
/// Sealed startup cut after the complete recovered LedgerV1 frame was fsynced.
///
/// The adapter and unpublished effects remain inseparable from the durable
/// repair and its exclusive registry reservation. The future child-install
/// tranche must consume this wrapper as a whole.
#[cfg(test)]
#[must_use = "durable WAL lifecycle startup has not installed its Sign child"]
struct DurableAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
    repair: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
}
/// Sealed startup cut after its exact recovered Sign child was installed.
///
/// The adapter and unpublished startup batch remain inseparable from the
/// installed registry cut. That cut keeps the registry exclusively borrowed,
/// while the complete durable repair and validated parent authority live in
/// its closed child row. No ordinary startup or raw-work escape exists here.
#[cfg(test)]
#[must_use = "installed WAL lifecycle startup has not completed publication"]
struct InstalledRecoveredWalLifecycleStartup<'registry> {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
    installed: InstalledRecoveredWalSignRegistryCut<'registry>,
}
#[cfg(test)]
#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum RecoveredWalLifecycleStartupFailure<'registry> {
    MissingVote {
        startup: AuthenticatedRecoveredAdapterStartup,
        validate: RecoveredWalValidateRegistryCut<'registry>,
    },
    RegistryJoin {
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        error: RecoveredWalValidateRegistryJoinError<'registry>,
    },
}
/// Drop-safe failure which retains the complete sealed startup cut.
///
/// No adapter, startup effect, recovered vote, or lifecycle binding is exposed
/// through this diagnostic. A failed join therefore cannot fall back to the
/// ordinary startup path or splice authority from another recovery instance.
#[cfg(test)]
#[allow(dead_code)]
#[must_use = "failed WAL lifecycle startup still owns all recovery authority"]
struct RecoveredWalLifecycleStartupError<'registry> {
    failure: Box<RecoveredWalLifecycleStartupFailure<'registry>>,
}
/// Fail-stop recovered-startup persistence error retaining every sealed input.
///
/// The error exposes only a stable diagnostic. It cannot release the adapter,
/// startup effects, registry reservation, validation result, or ledger repair,
/// regardless of whether the filesystem failed before or after publication.
#[cfg(test)]
#[allow(dead_code)]
#[must_use = "failed durable WAL startup still owns all recovery authority"]
struct RecoveredWalLifecycleLedgerPersistError<'registry> {
    _adapter: SumeragiV2Adapter,
    _effects: Vec<AdapterEffect>,
    error: RecoveredWalValidateLedgerPersistError<'registry>,
}
/// Fail-stop Sign-install error retaining the adapter, unpublished batch, and
/// complete uninstalled post-fsync authority.
#[cfg(test)]
#[allow(dead_code)]
#[must_use = "failed recovered Sign installation still owns all startup authority"]
struct RecoveredWalLifecycleSignInstallError<'registry> {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
    error: RecoveredWalSignInstallError<'registry>,
}
#[cfg(test)]
impl RecoveredWalLifecycleLedgerPersistError<'_> {
    /// Return a stable classification without exposing retained authority.
    fn reason(&self) -> &'static str {
        self.error.reason()
    }
}
#[cfg(test)]
impl RecoveredWalLifecycleSignInstallError<'_> {
    /// Return a stable classification without exposing retained authority.
    fn reason(&self) -> &'static str {
        self.error.reason()
    }
}
#[cfg(test)]
impl RecoveredWalLifecycleStartupError<'_> {
    /// Return a stable failure classification without exposing retained authority.
    fn reason(&self) -> &'static str {
        match self.failure.as_ref() {
            RecoveredWalLifecycleStartupFailure::MissingVote { .. } => {
                "recovered startup has no phase-vote continuation"
            }
            RecoveredWalLifecycleStartupFailure::RegistryJoin { error, .. } => error.reason(),
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredAdapterStartup {
    /// Exhaustively authenticate the WAL frontier and current startup authority.
    ///
    /// Failure returns the complete wrapper unchanged. Success moves the sole
    /// exact phase-vote, control-Sign, or Decision-Fetch authority into one
    /// private enum and
    /// retains an empty residual effect vector.
    #[allow(clippy::result_large_err)]
    pub(crate) fn authenticate_final_wal_startup_authority(
        mut self,
    ) -> Result<AuthenticatedRecoveredAdapterStartup, (AdapterError, Self)> {
        if let Err(error) = self.adapter.authenticate_recovered_wal_frontier() {
            return Err((error, self));
        }
        let validation_authority = match self.adapter.recovered_validation_authority(&self.effects)
        {
            Ok(authority) => authority,
            Err(error) => return Err((error, self)),
        };
        let recovered_vote = match self
            .adapter
            .authenticate_recovered_wal_vote_sign(&mut self.effects)
        {
            Ok(recovered_vote) => recovered_vote,
            Err(error) => return Err((error, self)),
        };
        if let Some(recovered_vote) = recovered_vote {
            debug_assert!(self.effects.is_empty());
            return Ok(AuthenticatedRecoveredAdapterStartup {
                adapter: self.adapter,
                effects: self.effects,
                authority: RecoveredWalStartupAuthorityV1::PhaseVote(recovered_vote),
                validation_authority,
                factory_owner: Arc::new(AuthenticatedRecoveredAdapterFactoryOwnerV1),
            });
        }
        let recovered_control = match self
            .adapter
            .authenticate_recovered_wal_control_sign(&mut self.effects)
        {
            Ok(recovered_control) => recovered_control,
            Err(error) => return Err((error, self)),
        };
        if let Some(control) = recovered_control {
            debug_assert!(self.effects.is_empty());
            return Ok(AuthenticatedRecoveredAdapterStartup {
                adapter: self.adapter,
                effects: self.effects,
                authority: RecoveredWalStartupAuthorityV1::ControlSign(control),
                validation_authority,
                factory_owner: Arc::new(AuthenticatedRecoveredAdapterFactoryOwnerV1),
            });
        }
        let recovered_fetch = match self
            .adapter
            .authenticate_recovered_wal_decision_fetch(&mut self.effects)
        {
            Ok(recovered_fetch) => recovered_fetch,
            Err(error) => return Err((error, self)),
        };
        if let Some(fetch) = recovered_fetch {
            debug_assert!(self.effects.is_empty());
            return Ok(AuthenticatedRecoveredAdapterStartup {
                adapter: self.adapter,
                effects: self.effects,
                authority: RecoveredWalStartupAuthorityV1::DecisionFetch(fetch),
                validation_authority,
                factory_owner: Arc::new(AuthenticatedRecoveredAdapterFactoryOwnerV1),
            });
        }
        if !self.effects.is_empty() {
            return Err((AdapterError::RecoveredStartupEffectMismatch, self));
        }
        Ok(AuthenticatedRecoveredAdapterStartup {
            adapter: self.adapter,
            effects: self.effects,
            authority: RecoveredWalStartupAuthorityV1::None,
            validation_authority,
            factory_owner: Arc::new(AuthenticatedRecoveredAdapterFactoryOwnerV1),
        })
    }
}
include!("v2_authenticated_recovered_adapter_startup_impl.rs");
#[cfg(test)]
impl<'registry> AuthenticatedRecoveredWalLifecycleStartup<'registry> {
    /// Consume the entire sealed startup through the focused LedgerV1 fsync
    /// fixture without exposing adapter, effects, or either repair authority.
    ///
    /// This is intentionally not a production startup transaction: it proves
    /// the opaque join can reach typed durable staging, then returns the whole
    /// post-fsync startup seal without installing or publishing it.
    #[allow(clippy::result_large_err)]
    fn persist_repair_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        (
            super::v2_lifecycle_coordinator::WalVoteLedgerRepairTestSummary,
            DurableAuthenticatedRecoveredWalLifecycleStartup<'registry>,
        ),
        RecoveredWalLifecycleLedgerPersistError<'registry>,
    > {
        let Self {
            adapter,
            effects,
            repair,
        } = self;
        match repair.persist_for_test(root) {
            Ok((summary, repair)) => Ok((
                summary,
                DurableAuthenticatedRecoveredWalLifecycleStartup {
                    adapter,
                    effects,
                    repair,
                },
            )),
            Err(error) => Err(RecoveredWalLifecycleLedgerPersistError {
                _adapter: adapter,
                _effects: effects,
                error,
            }),
        }
    }
    /// Exercise the stale-opened-snapshot path while preserving the same
    /// whole-startup ownership on both unexpected success and expected error.
    #[allow(clippy::result_large_err)]
    fn persist_stale_snapshot_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        DurableAuthenticatedRecoveredWalLifecycleStartup<'registry>,
        RecoveredWalLifecycleLedgerPersistError<'registry>,
    > {
        let Self {
            adapter,
            effects,
            repair,
        } = self;
        match repair.persist_stale_snapshot_for_test(root) {
            Ok(repair) => Ok(DurableAuthenticatedRecoveredWalLifecycleStartup {
                adapter,
                effects,
                repair,
            }),
            Err(error) => Err(RecoveredWalLifecycleLedgerPersistError {
                _adapter: adapter,
                _effects: effects,
                error,
            }),
        }
    }
    /// Re-enter the fsync seam from a fresh startup over an already-repaired
    /// ledger frame, retaining the whole startup seal.
    #[allow(clippy::result_large_err)]
    fn persist_reopened_repair_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        (
            bool,
            DurableAuthenticatedRecoveredWalLifecycleStartup<'registry>,
        ),
        RecoveredWalLifecycleLedgerPersistError<'registry>,
    > {
        let Self {
            adapter,
            effects,
            repair,
        } = self;
        match repair.persist_reopened_for_test(root) {
            Ok((changed, repair)) => Ok((
                changed,
                DurableAuthenticatedRecoveredWalLifecycleStartup {
                    adapter,
                    effects,
                    repair,
                },
            )),
            Err(error) => Err(RecoveredWalLifecycleLedgerPersistError {
                _adapter: adapter,
                _effects: effects,
                error,
            }),
        }
    }
}
#[cfg(test)]
impl<'registry> DurableAuthenticatedRecoveredWalLifecycleStartup<'registry> {
    /// Verify that the focused post-fsync startup still owns its adapter, has
    /// published no residual effects, and retains both vacant registry rows.
    fn remains_sealed_and_exact_for_test(&self, root: &std::path::Path) -> bool {
        self.effects.is_empty()
            && self.adapter.current_tag().height() == self.adapter.wire_context.height
            && self.repair.remains_exact_for_test(root)
    }
    /// Consume this whole post-fsync startup seal into the exact closed Sign
    /// registry row without publishing adapter status or mutating a
    /// coordinator.
    #[allow(clippy::result_large_err)]
    fn install_recovered_sign_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        InstalledRecoveredWalLifecycleStartup<'registry>,
        RecoveredWalLifecycleSignInstallError<'registry>,
    > {
        let Self {
            adapter,
            effects,
            repair,
        } = self;
        match repair.install_for_test(root) {
            Ok(installed) => Ok(InstalledRecoveredWalLifecycleStartup {
                adapter,
                effects,
                installed,
            }),
            Err(error) => Err(RecoveredWalLifecycleSignInstallError {
                adapter,
                effects,
                error,
            }),
        }
    }
}
#[cfg(test)]
impl InstalledRecoveredWalLifecycleStartup<'_> {
    /// Verify that startup remains sealed around one exact recovered Sign row.
    fn exact_installed_shape_for_test(&self, root: &std::path::Path) -> bool {
        self.effects.is_empty()
            && self.adapter.current_tag().height() == self.adapter.wire_context.height
            && self.installed.exact_installed_shape_for_test(root)
    }
}
#[cfg(test)]
impl RecoveredWalLifecycleSignInstallError<'_> {
    /// Verify that a failed install retains the whole startup and both vacant
    /// registry addresses against the original receipt-bound ledger root.
    fn remains_sealed_with_exact_vacancies_for_test(&self, root: &std::path::Path) -> bool {
        self.effects.is_empty()
            && self.adapter.current_tag().height() == self.adapter.wire_context.height
            && self.error.retains_exact_vacancies_for_test(root)
    }
}
// RECOVERED_WAL_VOTE_SIGN_SEAL_END
// RECOVERED_WAL_SIGN_STATUS_PUBLICATION_BEGIN
/// Fully opened recovered-WAL startup after adapter status publication.
///
/// This test-only fixture retains the adapter, unpublished residual batch,
/// installed registry borrow, authenticated recovery cut, and opened
/// coordinator in one opaque value. Production startup returns only
/// `ProductionLifecycleOwnerV1` from the unified consuming factory.
#[cfg(test)]
#[must_use = "published recovered WAL startup has not entered the production runner"]
struct PublishedRecoveredWalLifecycleStartup<'registry> {
    adapter: SumeragiV2Adapter,
    effects: Vec<AdapterEffect>,
    opened: OpenedRecoveredWalSignLifecycleCut<'registry>,
}
/// Fail-stop open/publication error retaining the complete sealed startup.
#[cfg(test)]
#[must_use = "failed recovered WAL startup publication still owns all authority"]
struct RecoveredWalLifecycleOpenPublicationError<'registry> {
    failure: RecoveredWalLifecycleOpenPublicationFailure<'registry>,
}
#[cfg(test)]
#[allow(dead_code, clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalLifecycleOpenPublicationFailure<'registry> {
    Open {
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        error: RecoveredWalSignLifecycleOpenError<'registry>,
    },
    Status {
        adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        opened: OpenedRecoveredWalSignLifecycleCut<'registry>,
        error: AdapterError,
    },
}
#[cfg(test)]
impl RecoveredWalLifecycleOpenPublicationError<'_> {
    /// Return one stable classification without exposing retained authority.
    fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalLifecycleOpenPublicationFailure::Open { error, .. } => error.reason(),
            RecoveredWalLifecycleOpenPublicationFailure::Status { error, .. } => match error {
                AdapterError::FailClosed => "adapter status publication is fail-closed",
                _ => "adapter status publication failed after exact lifecycle open",
            },
        }
    }
}
#[cfg(test)]
impl<'registry> InstalledRecoveredWalLifecycleStartup<'registry> {
    #[allow(clippy::result_large_err)]
    fn publish_open_result(
        mut adapter: SumeragiV2Adapter,
        effects: Vec<AdapterEffect>,
        opened: Result<
            OpenedRecoveredWalSignLifecycleCut<'registry>,
            RecoveredWalSignLifecycleOpenError<'registry>,
        >,
    ) -> Result<
        PublishedRecoveredWalLifecycleStartup<'registry>,
        RecoveredWalLifecycleOpenPublicationError<'registry>,
    > {
        let opened = match opened {
            Ok(opened) => opened,
            Err(error) => {
                return Err(RecoveredWalLifecycleOpenPublicationError {
                    failure: RecoveredWalLifecycleOpenPublicationFailure::Open {
                        adapter,
                        effects,
                        error,
                    },
                });
            }
        };
        // This is deliberately the sole externally visible publication and
        // runs only after recovery splice, prepared exact join, both fsyncs,
        // and the post-commit registry/coordinator/store revalidation.
        adapter.status_publication_enabled = true;
        if let Err(error) = adapter.publish_status() {
            return Err(RecoveredWalLifecycleOpenPublicationError {
                failure: RecoveredWalLifecycleOpenPublicationFailure::Status {
                    adapter,
                    effects,
                    opened,
                    error,
                },
            });
        }
        Ok(PublishedRecoveredWalLifecycleStartup {
            adapter,
            effects,
            opened,
        })
    }
    /// Open and publish one recovered coordinator using only the adapter's
    /// immutable verified context and production configuration.
    #[allow(dead_code)]
    #[allow(clippy::result_large_err)]
    fn open_coordinator_and_publish(
        self,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        payload_store: &mut super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
        recovery: super::v2_lifecycle_coordinator::AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        PublishedRecoveredWalLifecycleStartup<'registry>,
        RecoveredWalLifecycleOpenPublicationError<'registry>,
    > {
        let Self {
            adapter,
            effects,
            installed,
        } = self;
        let verified = VerifiedHeightContext {
            context: adapter.wire_context.clone(),
            proofs_of_possession: adapter.proofs_of_possession.clone(),
            parent_verification: adapter.parent_verification.clone(),
        };
        let opened = installed.open_coordinator_from_verified(
            &verified,
            config,
            reply_route_source_capacity,
            ledger_root,
            payload_store,
            recovery,
        );
        Self::publish_open_result(adapter, effects, opened)
    }
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    fn open_coordinator_and_publish_for_test(
        self,
        ledger_root: &std::path::Path,
        payload_store: &mut super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
        recovery: super::v2_lifecycle_coordinator::AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        PublishedRecoveredWalLifecycleStartup<'registry>,
        RecoveredWalLifecycleOpenPublicationError<'registry>,
    > {
        let Self {
            adapter,
            effects,
            installed,
        } = self;
        let verified = VerifiedHeightContext {
            context: adapter.wire_context.clone(),
            proofs_of_possession: adapter.proofs_of_possession.clone(),
            parent_verification: adapter.parent_verification.clone(),
        };
        let opened =
            installed.open_coordinator_for_test(&verified, ledger_root, payload_store, recovery);
        Self::publish_open_result(adapter, effects, opened)
    }
}
#[cfg(test)]
impl PublishedRecoveredWalLifecycleStartup<'_> {
    fn exact_published_join_for_test(&self) -> bool {
        self.effects.is_empty()
            && self.adapter.current_tag().height() == self.adapter.wire_context.height
            && self.opened.exact_join_for_test()
    }
}
#[cfg(test)]
impl RecoveredWalLifecycleOpenPublicationError<'_> {
    fn retains_closed_registry_row_for_test(&self) -> bool {
        match &self.failure {
            RecoveredWalLifecycleOpenPublicationFailure::Open { effects, error, .. } => {
                effects.is_empty() && error.retains_closed_registry_row_for_test()
            }
            RecoveredWalLifecycleOpenPublicationFailure::Status {
                effects, opened, ..
            } => effects.is_empty() && opened.exact_join_for_test(),
        }
    }
    fn retains_exact_installed_for_test(&self, ledger_root: &std::path::Path) -> bool {
        match &self.failure {
            RecoveredWalLifecycleOpenPublicationFailure::Open { effects, error, .. } => {
                effects.is_empty() && error.retains_exact_installed_for_test(ledger_root)
            }
            RecoveredWalLifecycleOpenPublicationFailure::Status {
                effects, opened, ..
            } => effects.is_empty() && opened.exact_join_for_test(),
        }
    }
}
// RECOVERED_WAL_SIGN_STATUS_PUBLICATION_END

// PendingKura uses its dedicated lifecycle-owned no-clock height. Every ordinary,
// applied, snapshot, and CompleteTip height now consumes this owner factory;
// those modes never construct the independent adapter/store stack.

/// Structurally and cryptographically verified immutable context for one
/// height.
///
/// The constructor verifies every roster proof of possession up front. A
/// non-genesis context additionally requires the exact durable parent
/// artifact and verifies both its CommitQC and the parent QC carried by the
/// new context under the previous frozen roster.
#[derive(Clone)]
pub(crate) struct VerifiedHeightContext {
    context: wire::HeightContext,
    proofs_of_possession: Vec<Vec<u8>>,
    parent_verification: Option<ParentVerificationContext>,
}
/// Frozen parent-roster material retained solely to authenticate the
/// parent CommitQC carried by a view-zero proposal.
#[derive(Clone)]
struct ParentVerificationContext {
    context: wire::HeightContext,
    proofs_of_possession: Vec<Vec<u8>>,
}
impl VerifiedHeightContext {
    /// Verify a genesis height context against its configured BLS roster.
    pub(crate) fn genesis(
        context: wire::HeightContext,
        proofs_of_possession: Vec<Vec<u8>>,
    ) -> Result<Self, AdapterError> {
        context.validate()?;
        if context.height != 1
            || context.parent_commit_qc.is_some()
            || context.snapshot_bootstrap.is_some()
        {
            return Err(AdapterError::InvalidGenesisContext);
        }
        verify_roster_proofs(&context, &proofs_of_possession)?;
        verify_next_epoch_snapshot_proofs(&context)?;
        Ok(Self {
            context,
            proofs_of_possession,
            parent_verification: None,
        })
    }
    /// Verify the complete first context authenticated by an audited snapshot payload.
    pub(crate) fn snapshot_bootstrap(
        record: &wire::SnapshotV2BootstrapRecord,
    ) -> Result<Self, AdapterError> {
        record.validate()?;
        if record.context.height <= 1
            || record.context.parent_commit_qc.is_some()
            || record.context.snapshot_bootstrap.is_none()
        {
            return Err(AdapterError::InvalidSnapshotBootstrapContext);
        }
        verify_roster_proofs(&record.context, &record.validator_set_pops)?;
        verify_next_epoch_snapshot_proofs(&record.context)?;
        Ok(Self {
            context: record.context.clone(),
            proofs_of_possession: record.validator_set_pops.clone(),
            parent_verification: None,
        })
    }
    /// Verify a successor context from a durable parent artifact.
    pub(crate) fn successor(
        context: wire::HeightContext,
        proofs_of_possession: Vec<Vec<u8>>,
        parent_artifact: &wire::finality::V2FinalityArtifact,
        parent_receipt: &KuraV2CommitReceipt,
        parent_proofs_of_possession: &[Vec<u8>],
    ) -> Result<Self, AdapterError> {
        context.validate()?;
        parent_artifact.validate()?;
        verify_next_epoch_snapshot_proofs(&context)?;
        if context.snapshot_bootstrap.is_some() {
            return Err(AdapterError::ParentContextMismatch);
        }
        if parent_artifact.validator_set_pops != parent_proofs_of_possession {
            return Err(AdapterError::ParentContextMismatch);
        }
        verify_roster_proofs(&parent_artifact.height_context, parent_proofs_of_possession)?;
        verify_quorum_certificate(
            &parent_artifact.height_context,
            &parent_artifact.commit_qc,
            parent_proofs_of_possession,
        )?;
        let parent_qc = context
            .parent_commit_qc
            .as_ref()
            .ok_or(AdapterError::ParentContextMismatch)?;
        let expected_height = parent_artifact
            .height
            .checked_add(1)
            .ok_or(AdapterError::ParentContextMismatch)?;
        if context.height != expected_height
            || context.network_id != parent_artifact.height_context.network_id
            || context.mode != parent_artifact.height_context.mode
            || context.da_layout != parent_artifact.height_context.da_layout
            || context.execution_policy_hash != parent_artifact.height_context.execution_policy_hash
            || !parent_qc
                .as_ref()
                .same_commit_decision(parent_artifact.commit_qc.as_ref())
            || parent_receipt.height() != parent_artifact.height
            || parent_receipt.context_id() != parent_artifact.context_id()
            || parent_receipt.block_hash() != parent_artifact.block_hash
            || parent_receipt.subject() != parent_artifact.subject
            || parent_receipt.certificate() != parent_artifact.commit_qc.as_ref()
            || parent_receipt.artifact_hash() != HashOf::new(parent_artifact)
        {
            return Err(AdapterError::ParentContextMismatch);
        }
        if let Some(snapshot) = &parent_artifact.height_context.next_epoch_snapshot {
            if context.epoch != snapshot.epoch
                || context.epoch_end_height != snapshot.epoch_end_height
                || context.mode != snapshot.mode
                || context.roster != snapshot.roster
                || context.quorum != snapshot.quorum
                || context.leader_seed != snapshot.leader_seed
                || context.kagemusha_mint_finality_epoch_id
                    != snapshot.kagemusha_mint_finality_epoch_id
                || context.kagemusha_mint_finality_epoch_roster
                    != snapshot.kagemusha_mint_finality_epoch_roster
                || proofs_of_possession.as_slice() != snapshot.validator_set_pops.as_slice()
            {
                return Err(AdapterError::EpochTransitionMismatch);
            }
        } else if context.epoch != parent_artifact.height_context.epoch
            || context.epoch_end_height != parent_artifact.height_context.epoch_end_height
            || context.roster != parent_artifact.height_context.roster
            || context.quorum != parent_artifact.height_context.quorum
            || context.leader_seed != parent_artifact.height_context.leader_seed
            || context.kagemusha_mint_finality_epoch_id
                != parent_artifact
                    .height_context
                    .kagemusha_mint_finality_epoch_id
            || context.kagemusha_mint_finality_epoch_roster
                != parent_artifact
                    .height_context
                    .kagemusha_mint_finality_epoch_roster
            || proofs_of_possession.as_slice() != parent_artifact.validator_set_pops.as_slice()
        {
            return Err(AdapterError::EpochTransitionMismatch);
        }
        verify_quorum_certificate(
            &parent_artifact.height_context,
            parent_qc,
            parent_proofs_of_possession,
        )?;
        verify_roster_proofs(&context, &proofs_of_possession)?;
        Ok(Self {
            context,
            proofs_of_possession,
            parent_verification: Some(ParentVerificationContext {
                context: parent_artifact.height_context.clone(),
                proofs_of_possession: parent_proofs_of_possession.to_vec(),
            }),
        })
    }
    /// Build a structurally exact successor owner for closed lifecycle fixtures.
    ///
    /// Production must use [`Self::successor`]. Some ledger fixtures retain
    /// intentionally non-cryptographic replay certificates and therefore need
    /// the same private predecessor/context shape without claiming voting
    /// authority.
    #[cfg(test)]
    pub(in crate::sumeragi) fn successor_fixture_for_test(
        context: wire::HeightContext,
        proofs_of_possession: Vec<Vec<u8>>,
        predecessor_context: wire::HeightContext,
        predecessor_proofs_of_possession: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            context,
            proofs_of_possession,
            parent_verification: Some(ParentVerificationContext {
                context: predecessor_context,
                proofs_of_possession: predecessor_proofs_of_possession,
            }),
        }
    }
    /// Borrow the exact frozen wire context.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
    }
    /// Borrow the exact durable predecessor context which authenticated this
    /// successor. Genesis and audited snapshot-bootstrap contexts have no such
    /// predecessor and therefore return `None`.
    pub(crate) fn verified_predecessor_context(&self) -> Option<&wire::HeightContext> {
        self.parent_verification
            .as_ref()
            .map(|parent| &parent.context)
    }
    /// Borrow proofs of possession in the exact frozen-roster order.
    pub(crate) fn proofs_of_possession(&self) -> &[Vec<u8>] {
        &self.proofs_of_possession
    }
    /// Verify one quorum certificate against this exact frozen roster and its
    /// already-authenticated proofs of possession.
    pub(crate) fn verify_quorum_certificate(
        &self,
        certificate: &wire::QuorumCertificate,
    ) -> Result<(), AdapterError> {
        verify_quorum_certificate(&self.context, certificate, &self.proofs_of_possession)
    }
}
include!("v2_verified_height_context_recovered_output_auth.rs");
/// A canonical message whose safety intent is already durable and may be signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SignRequest {
    /// Leader proposal with an empty signature field.
    Proposal(wire::Proposal),
    /// Prepare or Commit vote with an empty signature field.
    Vote(wire::Vote),
    /// Timeout vote with an empty signature field.
    TimeoutVote(wire::TimeoutVote),
}
impl SignRequest {
    /// Return the canonical bytes authorized by this durable signing request.
    pub(crate) fn signature_preimage(&self) -> Vec<u8> {
        match self {
            Self::Proposal(proposal) => proposal.signature_preimage(),
            Self::Vote(vote) => vote.signature_preimage(),
            Self::TimeoutVote(vote) => vote.signature_preimage(),
        }
    }
    /// Return the exact block subject owned by proposal or phase-vote work.
    ///
    /// Timeout votes carry only an optional high-QC report and do not own that
    /// certificate's body pipeline, so they deliberately return `None`.
    pub(crate) const fn subject(&self) -> Option<wire::BlockSubject> {
        match self {
            Self::Proposal(proposal) => Some(proposal.subject),
            Self::Vote(vote) => Some(vote.subject),
            Self::TimeoutVote(_) => None,
        }
    }
    /// Return the exact proposal/body origin owned by proposal or phase-vote work.
    ///
    /// As with [`Self::subject`], a timeout vote is view-progress work rather
    /// than ownership of the body named by its optional high QC.
    pub(crate) const fn body_round(&self) -> Option<wire::ConsensusRound> {
        match self {
            Self::Proposal(proposal) => Some(proposal.round),
            Self::Vote(vote) => Some(vote.proposal_round),
            Self::TimeoutVote(_) => None,
        }
    }
}
/// Return whether a proposal still satisfies the safe-value rule for one lock.
///
/// The exact locked subject remains safe in a later justified view. A different
/// subject is safe only when the immediately preceding timeout certificate
/// carries a strictly higher PrepareQC for that same subject.
pub(crate) fn proposal_is_safe_for_lock(
    proposal: &wire::Proposal,
    locked_round: wire::ConsensusRound,
    locked_subject: wire::BlockSubject,
) -> bool {
    if proposal.round.context_id != locked_round.context_id
        || proposal.round.height != locked_round.height
        || proposal.round.view < locked_round.view
    {
        return false;
    }
    if proposal.subject == locked_subject {
        return true;
    }
    let wire::ProposalJustification::Timeout(timeout) = &proposal.justification else {
        return false;
    };
    timeout.highest_prepare_qc.as_ref().is_some_and(|highest| {
        highest.phase == wire::GlobalPhase::Prepare
            && highest.round.context_id == locked_round.context_id
            && highest.round.height == locked_round.height
            && highest.round.view > locked_round.view
            && highest.subject == proposal.subject
            && timeout
                .timeout_certificate
                .highest_prepare_qc()
                .is_some_and(|selected| selected == highest)
    })
}
/// Effects delivered by the production adapter to asynchronous services.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AdapterEffect {
    /// Sign a canonical vote or timeout vote after its WAL intent is durable.
    Sign {
        /// Reducer incarnation tag to return with the signature completion.
        tag: reducer::EventTag,
        /// Canonical unsigned message.
        request: SignRequest,
    },
    /// Broadcast one explicitly versioned canonical v2 envelope.
    Broadcast(wire::ConsensusMessageV2),
    /// Fetch a body from ordinary or certified sources.
    FetchBody {
        /// Reducer incarnation tag for the completion.
        tag: reducer::EventTag,
        /// Proposal round.
        round: wire::ConsensusRound,
        /// Exact requested subject.
        subject: wire::BlockSubject,
        /// Manifest when the proposal supplied one.
        manifest: Option<wire::PayloadManifest>,
        /// Certified validator sources, empty for an uncertified proposal fetch.
        certified_sources: Vec<PeerId>,
        /// Full QC authorizing a certified request, absent for an uncertified
        /// leader-proposal fetch.
        certificate: Option<wire::QuorumCertificate>,
    },
    /// Durably store an already reconstructed exact body.
    StoreBody {
        /// Reducer incarnation tag for the completion.
        tag: reducer::EventTag,
        /// Proposal round.
        round: wire::ConsensusRound,
        /// Exact stored subject.
        subject: wire::BlockSubject,
    },
    /// Run deterministic validation over a durably stored exact body.
    ValidateBody {
        /// Reducer incarnation tag for the completion.
        tag: reducer::EventTag,
        /// Proposal round.
        round: wire::ConsensusRound,
        /// Exact subject to validate.
        subject: wire::BlockSubject,
    },
    /// Apply a decision only after its CommitQC decision record is durable.
    Apply {
        /// Reducer incarnation tag for the application completion.
        tag: reducer::EventTag,
        /// Exact finalized subject.
        subject: wire::BlockSubject,
        /// Canonical CommitQC authorizing application.
        certificate: wire::QuorumCertificate,
    },
    /// Reset lifecycle ownership after a persisted timeout certificate advances
    /// the view or supersedes the current view with a higher-generation lock.
    EnterView {
        /// New reducer incarnation tag.
        tag: reducer::EventTag,
        /// Canonical certificate authorizing the new view.
        certificate: wire::TimeoutCertificate,
        /// Exact authenticated post-install PrepareQC whose body pipeline must
        /// survive the transition.
        protected_lock: Option<wire::QuorumCertificate>,
    },
    /// Validate and persist exact authenticated equivocation evidence.
    ReportEquivocation {
        /// Complete authenticated conflicting signed pair retained by this process.
        evidence: AdapterEquivocationEvidence,
    },
    /// Report a deterministic validation failure for a certified body.
    ReportInvalidCertifiedBody {
        /// Rejected subject.
        subject: wire::BlockSubject,
        /// PrepareQC whose signers certified validity and availability.
        certificate: wire::QuorumCertificate,
    },
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
/// Record-checked linear cause for one exact post-fsync WAL continuation.
pub(super) enum ExactLiveWalPersistedContinuationCause {
    /// One payload-free continuation with its uniquely derived pending owner.
    PayloadFree {
        /// Exact live frame seal retained from append acknowledgement.
        wal_identity: LiveWalFrameIdentity,
        /// Exact converted continuation proved against the retained WAL record.
        effect: AdapterEffect,
        /// Pending owner derived from this frame seal and complete effect.
        pending: PendingRuntimeEffectBinding,
    },
    /// One `Apply` continuation awaiting its retained Validate predecessor.
    Apply {
        /// Exact live frame seal retained from append acknowledgement.
        wal_identity: LiveWalFrameIdentity,
        /// Exact converted `Apply` proved against the retained Decision record.
        effect: AdapterEffect,
    },
}
/// One-shot adapter/runtime handoff for an initial local Proposal Sign.
///
/// The complete live WAL seal remains private. The cloneable effect is kept
/// only as an equality coordinate for the exact returned adapter batch; it
/// does not replace the seal's locator-derived pending owner.
#[must_use = "a live ProposalIntent Sign WAL handoff must enter lifecycle admission"]
pub(crate) struct LiveProposalIntentWalSignHandoffV1 {
    effect: AdapterEffect,
    persisted: SealedLiveWalPersistedEffectV1,
}
impl LiveProposalIntentWalSignHandoffV1 {
    fn from_exact(
        effect: AdapterEffect,
        persisted: SealedLiveWalPersistedEffectV1,
    ) -> Option<Self> {
        persisted
            .exactly_binds_payload_free_proposal_sign(&effect)
            .then_some(Self { effect, persisted })
    }
    fn exactly_matches_effects(&self, effects: &[AdapterEffect]) -> bool {
        effects == core::slice::from_ref(&self.effect)
            && self
                .persisted
                .exactly_binds_payload_free_proposal_sign(&self.effect)
    }

    /// Atomically join the post-fsync WAL owner to its retained local lineage.
    ///
    /// Failure returns both move-only inputs intact. The returned admission
    /// keeps the WAL-derived pending owner; the local pending is companion
    /// provenance only.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn join_local_proposal(
        self,
        companion: LocalProposalIntentReplayEvidenceV1,
    ) -> Result<PendingLiveWalSignAdmissionV1, (Self, LocalProposalIntentReplayEvidenceV1)> {
        let exact = matches!(
            &self.effect,
            AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }
        ) && self
            .persisted
            .exactly_binds_payload_free_proposal_sign(&self.effect)
            && companion.exactly_matches_live_wal_sign_effect(&self.effect);
        if !exact {
            return Err((self, companion));
        }
        Ok(PendingLiveWalSignAdmissionV1::from_local_proposal(
            self.persisted,
            companion,
        ))
    }
}
include!("v2_adapter_equivocation_evidence.rs");
/// Result of one serialized reducer input after all synchronous WAL work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdapterOutcome {
    disposition: reducer::StepDisposition,
    effects: Vec<AdapterEffect>,
    deferred_admission_ordinal: Option<u128>,
    producer_handoff: Option<ProducerContinuationHandoffToken>,
}
/// Exact reducer fences which can make a lifecycle-owned completion return
/// `Busy` without consuming that completion.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ReducerFenceProjection {
    pending_persistence: Option<reducer::WalRecord>,
    awaiting_signature: Option<reducer::SignableMessage>,
    replay_complete: bool,
}
/// Exact process-local generation which wakes direct lifecycle body work.
///
/// The source is derived from the adapter's immutable height context and the
/// generation is sampled from the same serialized runtime.  Callers may feed
/// this opaque pair only into the lifecycle scheduler; no raw generation mint
/// is exposed outside the adapter/executor boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct LifecycleReducerFenceObservationV1 {
    source: super::v2_lifecycle_coordinator::WaitSource,
    generation: u64,
}
impl LifecycleReducerFenceObservationV1 {
    /// Construct an exact reducer-fence observation for lifecycle unit tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn for_test(
        source: super::v2_lifecycle_coordinator::WaitSource,
        generation: u64,
    ) -> Self {
        Self { source, generation }
    }
    /// Return the context-scoped external wait source.
    pub(in crate::sumeragi) const fn source(self) -> super::v2_lifecycle_coordinator::WaitSource {
        self.source
    }
    /// Return the exact sampled reducer-fence generation.
    pub(in crate::sumeragi) const fn generation(self) -> u64 {
        self.generation
    }
}
/// Borrow-bound generation snapshot for one direct completion blocked by the
/// reducer's persistence or signature fence.
///
/// Retaining the adapter borrow prevents ordinary safe code from changing the
/// sampled fence before the lifecycle transaction records its explicit
/// external-generation wait.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "the sampled reducer fence must be settled or deliberately abandoned"]
pub(in crate::sumeragi) struct PreparedReducerFenceWait<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    context_id: wire::HeightContextId,
    generation: u64,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedReducerFenceWait<'_> {
    /// Return the authenticated height-context identity used to derive the
    /// coordinator's domain-separated external wait source.
    pub(in crate::sumeragi) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }
    /// Return the exact monotone reducer-fence generation observed by this
    /// blocked attempt.
    pub(in crate::sumeragi) const fn generation(&self) -> u64 {
        self.generation
    }
}
/// Exact idempotent disposition of a direct certified-body completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectCertifiedBodyAvailableStutter {
    /// The reducer no longer owns body work for the supplied round and subject.
    NoMatchingWork,
    /// The exact body already advanced beyond the missing state.
    Duplicate,
}
/// Closed non-applied result of one direct body-completion preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectCertifiedBodyAvailableInactive {
    /// The reducer already consumed or no longer owns the exact body work.
    Stutter(DirectCertifiedBodyAvailableStutter),
    /// The effect belongs to a stale reducer incarnation or view.
    Superseded(reducer::IgnoreReason),
}
/// Borrow-bound non-applied direct-completion classification.
///
/// The lifecycle transaction settles the corresponding logical
/// record before dropping this token; retaining the adapter borrow prevents a
/// check-then-use race even on terminal or idempotent paths.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a non-applied direct completion still owns its classification cut"]
struct PreparedDirectCertifiedBodyAvailableInactive<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    disposition: DirectCertifiedBodyAvailableInactive,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectCertifiedBodyAvailableInactive<'_> {
    /// Return the exact closed non-applied disposition.
    const fn disposition(&self) -> DirectCertifiedBodyAvailableInactive {
        self.disposition
    }
}
/// Fully checked direct `BodyAvailable -> StoreBody` transition.
///
/// Preparation executes the reducer transition only on cloned state and holds
/// the exclusive adapter borrow. Consequently the post-publication tail can
/// install this exact state without another fallible reducer call or a parallel
/// producer-continuation reservation.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a prepared direct completion has not installed its reducer transition"]
struct PreparedDirectCertifiedBodyAvailable<'a> {
    adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    core_effect: reducer::Effect,
    store_effect: AdapterEffect,
    next_fence_generation: u64,
}
/// Closed adapter half of one ordinary certified Fetch-to-Store transaction.
///
/// The derived Store effect stays nested beside the cloned reducer state.  It
/// can be inspected only by the exact registry successor and committed only
/// after LedgerV1 publication.
#[must_use = "the certified Fetch adapter successor has not been published"]
pub(in crate::sumeragi) struct PreparedCertifiedFetchStoreAdapterV1<'a> {
    preview: PreparedDirectCertifiedBodyAvailable<'a>,
}
impl PreparedCertifiedFetchStoreAdapterV1<'_> {
    /// Borrow the exact Store effect for registry successor projection.
    pub(in crate::sumeragi) const fn store_effect(&self) -> &AdapterEffect {
        self.preview.store_effect()
    }
    /// Install the already-checked reducer transition after durable publication.
    pub(in crate::sumeragi) fn commit_after_durable_publication(self) {
        let expected = self.preview.store_effect().clone();
        let committed = self.preview.commit();
        assert_eq!(committed, expected);
    }
}
/// Outcome of previewing one ordinary certified Fetch completion.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[must_use = "the direct certified Fetch preview must be settled"]
pub(in crate::sumeragi) enum CertifiedFetchStoreAdapterPreparationV1<'a> {
    /// The exact Fetch-to-Store reducer transition is ready for publication.
    Applied(PreparedCertifiedFetchStoreAdapterV1<'a>),
    /// A reducer persistence/signature fence must advance before retry.
    Blocked(PreparedReducerFenceWait<'a>),
    /// The lifecycle carrier and reducer no longer describe the same live edge.
    Inactive,
}
/// Borrow-bound direct adapter preview for recovered Decision Fetch settlement.
///
/// The body authority remains opaque beside the cloned reducer transition.
/// Only the recovered registry transaction may inspect the derived Store
/// effect and consume this token after LedgerV1 publication.
#[must_use = "recovered Decision Store adapter preview has not been published"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchStoreAdapterV1<'a> {
    preview: PreparedDirectCertifiedBodyAvailable<'a>,
    body: super::v2_body_store::RecoveredDecisionFetchStoreBodyAuthorityV1,
}
impl PreparedRecoveredDecisionFetchStoreAdapterV1<'_> {
    /// Borrow the reducer-derived Store effect only for fixed registry projection.
    pub(in crate::sumeragi) const fn store_effect(&self) -> &AdapterEffect {
        self.preview.store_effect()
    }
    /// Clone only the opaque body authority for the sealed successor projection.
    pub(in crate::sumeragi) fn body_authority(
        &self,
    ) -> super::v2_body_store::RecoveredDecisionFetchStoreBodyAuthorityV1 {
        self.body.clone()
    }
    /// Install the already-checked reducer transition after durable publication.
    pub(in crate::sumeragi) fn commit_after_durable_settlement(self) {
        let expected = self.preview.store_effect().clone();
        let committed = self.preview.commit();
        assert_eq!(committed, expected);
        drop(self.body);
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectCertifiedBodyAvailable<'_> {
    /// Borrow the single exact Store effect derived by the staged reducer.
    const fn store_effect(&self) -> &AdapterEffect {
        &self.store_effect
    }
    /// Install the already-checked reducer and registry state.
    ///
    /// This method performs only infallible in-memory moves and accounting. The
    /// lifecycle transaction calls it only after every fallible
    /// lifecycle/registry/service preflight succeeds and the selected ingress
    /// occurrence is committed under the output fail-stop guard.
    // Recovered Decision Fetch settlement now reaches this commit only while
    // its opaque body authority remains in the prepared registry successor.
    // Cold open reconstructs the resulting dedicated Store carrier from the
    // same fsynced body receipt and the payload-free WAL Fetch lineage.
    fn commit(self) -> AdapterEffect {
        let Self {
            adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            store_effect,
            next_fence_generation,
        } = self;
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(
            &event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&core_effect),
        );
        adapter.log_body_progress(&event, reducer::StepDisposition::Applied, 1);
        store_effect
    }
}
/// Read-only classification of one direct certified-body completion attempt.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "the direct completion classification owns an adapter borrow when actionable"]
enum DirectCertifiedBodyAvailablePreparation<'a> {
    /// The exact reducer transition and Store successor are ready to commit.
    Applied(PreparedDirectCertifiedBodyAvailable<'a>),
    /// A reducer-owned persistence/signature fence must advance before retry.
    Blocked(PreparedReducerFenceWait<'a>),
    /// The exact attempt was an idempotent stutter or a superseded incarnation.
    Inactive(PreparedDirectCertifiedBodyAvailableInactive<'a>),
}
/// Exact idempotent disposition of one direct durable-body completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectBodyStoredStutter {
    /// The reducer no longer owns body work for the supplied round and subject.
    NoMatchingWork,
    /// The exact body already advanced beyond the available state.
    Duplicate,
}
/// Closed non-applied result of one direct durable-body completion preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectBodyStoredInactive {
    /// The reducer already consumed or no longer owns the exact body work.
    Stutter(DirectBodyStoredStutter),
    /// The effect belongs to a stale reducer incarnation or view.
    Superseded(reducer::IgnoreReason),
}
/// Borrow-bound non-applied durable-body completion classification.
///
/// Retaining the adapter borrow prevents a check-then-use race while the
/// lifecycle transaction settles the Store record as idempotent or
/// superseded.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a non-applied direct durable-body completion still owns its classification cut"]
struct PreparedDirectBodyStoredInactive<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    disposition: DirectBodyStoredInactive,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectBodyStoredInactive<'_> {
    /// Return the exact closed non-applied disposition.
    const fn disposition(&self) -> DirectBodyStoredInactive {
        self.disposition
    }
}
/// Fully checked direct `BodyStored -> ValidateBody` transition.
///
/// Preparation executes the reducer transition only on cloned state and holds
/// the exclusive adapter borrow. The Store lifecycle transaction can
/// therefore install the exact transition without consulting deferred,
/// serviced-candidate, producer-continuation, or WAL helper machinery.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a prepared direct durable-body completion has not installed its reducer transition"]
struct PreparedDirectBodyStored<'a> {
    adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    core_effect: reducer::Effect,
    validate_effect: AdapterEffect,
    next_fence_generation: u64,
}
/// Closed adapter half of one ordinary Store-to-Validate transaction.
#[must_use = "the durable Store adapter successor has not been published"]
pub(in crate::sumeragi) struct PreparedDurableStoreValidateAdapterV1<'a> {
    preview: PreparedDirectBodyStored<'a>,
}
impl PreparedDurableStoreValidateAdapterV1<'_> {
    /// Borrow the exact Validate effect for registry successor projection.
    pub(in crate::sumeragi) const fn validate_effect(&self) -> &AdapterEffect {
        self.preview.validate_effect()
    }
    /// Install the already-checked reducer transition after durable publication.
    pub(in crate::sumeragi) fn commit_after_durable_publication(self) {
        let expected = self.preview.validate_effect().clone();
        let committed = self.preview.commit();
        assert_eq!(committed, expected);
    }
}
/// Outcome of previewing one ordinary durable Store completion.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[must_use = "the direct durable Store preview must be settled"]
pub(in crate::sumeragi) enum DurableStoreValidateAdapterPreparationV1<'a> {
    /// The exact Store-to-Validate reducer transition is ready for publication.
    Applied(PreparedDurableStoreValidateAdapterV1<'a>),
    /// A reducer persistence/signature fence must advance before retry.
    Blocked(PreparedReducerFenceWait<'a>),
    /// The lifecycle carrier and reducer no longer describe the same live edge.
    Inactive,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectBodyStored<'_> {
    /// Borrow the single exact Validate effect derived by the staged reducer.
    const fn validate_effect(&self) -> &AdapterEffect {
        &self.validate_effect
    }
    /// Install the already-checked reducer and registry state.
    ///
    /// This method performs only infallible in-memory moves and accounting. The
    /// move-only Store parent-to-child registry transaction calls it only after
    /// LedgerV1 publication; cold open replays the same body-stage edge before
    /// exposing the child carrier.
    fn commit(self) -> AdapterEffect {
        let Self {
            adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            validate_effect,
            next_fence_generation,
        } = self;
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(
            &event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&core_effect),
        );
        adapter.log_body_progress(&event, reducer::StepDisposition::Applied, 1);
        validate_effect
    }
}
/// Read-only classification of one direct durable-body completion attempt.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "the direct durable-body classification owns an adapter borrow when actionable"]
enum DirectBodyStoredPreparation<'a> {
    /// The exact reducer transition and Validate successor are ready to commit.
    Applied(PreparedDirectBodyStored<'a>),
    /// A reducer-owned persistence/signature fence must advance before retry.
    Blocked(PreparedReducerFenceWait<'a>),
    /// The exact attempt was an idempotent stutter or superseded incarnation.
    Inactive(PreparedDirectBodyStoredInactive<'a>),
}
/// Exact idempotent disposition of one direct failed-validation preview.
#[derive(Debug, PartialEq, Eq)]
enum DirectValidationFailedStutter {
    /// The reducer no longer owns validation work for this round and subject.
    NoMatchingWork,
    /// The exact body was already rejected or otherwise left the durable state.
    Duplicate,
}
/// Closed non-Busy ignored result of one direct failed-validation preview.
#[derive(Debug, PartialEq, Eq)]
enum DirectValidationFailedInactive {
    /// The reducer made no change because the exact work was absent or complete.
    Stutter(DirectValidationFailedStutter),
    /// The reducer incarnation or phase no longer accepts this child effect.
    ///
    /// The sealed token retains the cloned reducer defensively so a future
    /// reducer refinement cannot silently turn this classification into a live
    /// state change.
    Superseded(reducer::IgnoreReason),
}
/// Borrow-bound failed-validation preview blocked by a reducer fence.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a blocked direct rejection still owns staged validation authority"]
struct PreparedDirectValidationFailedBusy<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_registry: WireRegistry,
    context_id: wire::HeightContextId,
    generation: u64,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectValidationFailedBusy<'_> {
    /// Return the authenticated context owning the sampled reducer fence.
    const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }
    /// Return the exact non-reserved reducer-fence generation observed.
    const fn generation(&self) -> u64 {
        self.generation
    }
}
/// Borrow-bound non-Busy ignored failed-validation preview.
///
/// Both staged authorities remain sealed even for a reducer stutter so an
/// ignored result cannot accidentally discard a future defensive state change.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "an inactive direct rejection still owns staged validation authority"]
struct PreparedDirectValidationFailedInactive<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    disposition: DirectValidationFailedInactive,
    next_fence_generation: u64,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectValidationFailedInactive<'_> {
    /// Borrow the exact closed ignored disposition.
    const fn disposition(&self) -> &DirectValidationFailedInactive {
        &self.disposition
    }
}
/// Borrow-bound applied rejection which emits no child effect.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "an effect-free direct rejection still owns staged reducer authority"]
struct PreparedDirectValidationFailedNoEffect<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    next_fence_generation: u64,
}
/// Borrow-bound applied rejection which emits one exact PrepareQC report.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a direct rejection report still owns staged reducer authority"]
struct PreparedDirectValidationFailedReport<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    core_effect: reducer::Effect,
    report_effect: AdapterEffect,
    next_fence_generation: u64,
}
/// Unforgeable move-only proof that an exact invalid-body report names the PrepareQC
/// retained by the adapter's post-rejection registry clone.
///
/// Construction is private to the fixed rejected-Validate preview. The only
/// public operation is exact report equality, so neither the certificate nor
/// its registered statement can be extracted or caller-supplied. A prepared
/// preview may reproduce this proof for internal revalidation, so the proof is
/// deliberately described as move-only rather than as a linear one-shot mint.
#[derive(Debug)]
#[must_use = "registered Prepare authority must complete its invalid-body replay projection"]
pub(in crate::sumeragi) struct RegisteredPrepareInvalidBodyReportCapability {
    report_effect: AdapterEffect,
}
impl RegisteredPrepareInvalidBodyReportCapability {
    /// Return whether this move-only proof names the exact retained report.
    pub(in crate::sumeragi) fn exactly_matches_report(&self, effect: &AdapterEffect) -> bool {
        &self.report_effect == effect
    }
}
/// Opaque proof that one staged `LockAndCommit` record retained the exact
/// registered Prepare certificate promoted by its unsigned Commit vote.
///
/// Construction is private to the Ready-Validate Persist preflight. The
/// certificate has no accessor and callers can only ask whether the complete
/// capability authorizes one exact ordinary-Validate-to-Commit refinement.
#[derive(Debug)]
#[must_use = "registered Prepare authority must bind its sealed Commit Sign successor"]
pub(in crate::sumeragi) struct RegisteredPrepareValidateSignCapability {
    prepare: wire::QuorumCertificate,
    commit_effect: AdapterEffect,
}
impl RegisteredPrepareValidateSignCapability {
    fn from_staged_lock_and_commit(
        record: &reducer::WalRecord,
        registry: &WireRegistry,
        commit_effect: &AdapterEffect,
    ) -> Option<Self> {
        let reducer::WalRecord::LockAndCommit { prepare, vote } = record else {
            return None;
        };
        let AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(commit_vote),
        } = commit_effect
        else {
            return None;
        };
        let registered = registry.certificates.get(&prepare.reference())?;
        if prepare.phase() != reducer::Phase::Prepare
            || vote.phase() != reducer::Phase::Commit
            || prepare.round() != vote.round()
            || prepare.proposal_round() != vote.proposal_round()
            || prepare.subject() != vote.subject()
            || registered.phase != wire::GlobalPhase::Prepare
            || commit_vote.phase != wire::GlobalPhase::Commit
            || registered.round != commit_vote.round
            || registered.proposal_round != commit_vote.proposal_round
            || registered.subject != commit_vote.subject
            || registered.execution_commitment != commit_vote.execution_commitment
            || tag.height() != commit_vote.round.height
            || tag.view() != commit_vote.round.view
            || !commit_vote.signature.is_empty()
            || !registry.reducer_qc_matches_wire(prepare, registered)
            || registry.unsigned_vote_to_wire(*vote).ok().as_ref() != Some(commit_vote)
        {
            return None;
        }
        Some(Self {
            prepare: registered.clone(),
            commit_effect: commit_effect.clone(),
        })
    }
    /// Return whether this unforgeable registered carrier authorizes the exact
    /// ordinary Validate predecessor and unsigned Commit successor.
    pub(in crate::sumeragi) fn authorizes_ordinary_validate_commit(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> bool {
        let (
            AdapterEffect::ValidateBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::Sign {
                tag: successor_tag,
                request: SignRequest::Vote(vote),
            },
        ) = (predecessor, successor)
        else {
            return false;
        };
        successor == &self.commit_effect
            && predecessor_tag == successor_tag
            && vote.phase == wire::GlobalPhase::Commit
            && vote.signature.is_empty()
            && vote.proposal_round == *predecessor_round
            && vote.subject == *predecessor_subject
            && self.prepare.phase == wire::GlobalPhase::Prepare
            && self.prepare.round == vote.round
            && self.prepare.proposal_round == vote.proposal_round
            && self.prepare.subject == vote.subject
            && self.prepare.execution_commitment == vote.execution_commitment
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectValidationFailedReport<'_> {
    /// Borrow the exact certified-body rejection report.
    const fn report_effect(&self) -> &AdapterEffect {
        &self.report_effect
    }
    fn registered_prepare_report_capability(
        &self,
    ) -> Option<RegisteredPrepareInvalidBodyReportCapability> {
        let reducer::Effect::ReportInvalidCertifiedBody {
            subject: core_subject,
            certificate: core_certificate,
        } = &self.core_effect
        else {
            return None;
        };
        let AdapterEffect::ReportInvalidCertifiedBody {
            subject,
            certificate,
        } = &self.report_effect
        else {
            return None;
        };
        let registered = self
            .next_registry
            .certificates
            .get(&core_certificate.reference())?;
        if registered != certificate
            || core_certificate.phase() != reducer::Phase::Prepare
            || certificate.phase != wire::GlobalPhase::Prepare
            || certificate.subject != *subject
            || core_certificate.subject() != *core_subject
        {
            return None;
        }
        Some(RegisteredPrepareInvalidBodyReportCapability {
            report_effect: self.report_effect.clone(),
        })
    }
}
/// Closed classification of one direct deterministic validation rejection.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "the direct rejection classification owns an exclusive adapter borrow"]
enum DirectValidationFailedPreparation<'a> {
    /// A reducer persistence/signature fence blocked the exact completion.
    Busy(PreparedDirectValidationFailedBusy<'a>),
    /// The exact event was ignored without emitting a child effect.
    Inactive(PreparedDirectValidationFailedInactive<'a>),
    /// Rejection applied and emitted no child effect.
    NoEffect(PreparedDirectValidationFailedNoEffect<'a>),
    /// Rejection applied and emitted one exact invalid-certified-body report.
    Report(PreparedDirectValidationFailedReport<'a>),
}
/// Exact idempotent disposition of one direct successful-validation preview.
#[derive(Debug, PartialEq, Eq)]
enum DirectValidationSucceededStutter {
    /// The reducer no longer owns validation work for this round and subject.
    NoMatchingWork,
    /// The exact body was already validated or otherwise left the durable state.
    Duplicate,
}
/// Closed non-Busy ignored result of one direct successful-validation preview.
#[derive(Debug, PartialEq, Eq)]
enum DirectValidationSucceededInactive {
    /// The reducer made no change because the exact work was absent or complete.
    Stutter(DirectValidationSucceededStutter),
    /// The reducer incarnation or phase no longer accepts a child effect.
    ///
    /// Some reducer reasons in this class still advance the cloned body state
    /// from Durable to Validated. The sealed token therefore retains that exact
    /// staged reducer instead of treating every ignored result as a state
    /// stutter.
    Superseded(reducer::IgnoreReason),
}
/// Borrow-bound successful-validation preview blocked by a reducer fence.
///
/// The staged registry retains the independently durable execution commitment
/// even though the reducer consumer must wait. Retaining the adapter borrow
/// prevents another transition from invalidating this classification cut.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a blocked direct validation still owns staged validation authority"]
struct PreparedDirectValidationSucceededBusy<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_registry: WireRegistry,
    context_id: wire::HeightContextId,
    generation: u64,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectValidationSucceededBusy<'_> {
    /// Return the authenticated context owning the sampled reducer fence.
    const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }
    /// Return the exact non-reserved reducer-fence generation observed.
    const fn generation(&self) -> u64 {
        self.generation
    }
}
/// Borrow-bound non-Busy ignored successful-validation preview.
///
/// Both staged authorities are retained because an ignored reducer outcome can
/// still advance the body from Durable to Validated before deciding that no
/// child effect belongs to the current view or validator role.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "an inactive direct validation still owns staged validation authority"]
struct PreparedDirectValidationSucceededInactive<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    disposition: DirectValidationSucceededInactive,
    next_fence_generation: u64,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectValidationSucceededInactive<'_> {
    /// Borrow the exact closed ignored disposition.
    const fn disposition(&self) -> &DirectValidationSucceededInactive {
        &self.disposition
    }
}
/// Borrow-bound applied validation which emits no child effect.
#[allow(dead_code)]
#[must_use = "an effect-free direct validation still owns staged reducer authority"]
struct PreparedDirectValidationSucceededNoEffect<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    next_fence_generation: u64,
}
/// Borrow-bound applied validation which emits one exact decision application.
#[allow(dead_code)]
#[must_use = "a direct validation Apply result still owns staged reducer authority"]
struct PreparedDirectValidationSucceededApply<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    core_effect: reducer::Effect,
    apply_effect: AdapterEffect,
    next_fence_generation: u64,
}
/// Opaque staged adapter state for the fixed recovered Decision body fast-forward.
/// It keeps intermediate effects and bindings private until the body module
/// rejoins the original Fetch projection, body cut, and replay lineage.
#[must_use = "staged recovered Decision adapter state must enter its body-owned composite"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyStagedAdapterV1 {
    adapter: SumeragiV2Adapter,
    store_effect: AdapterEffect,
    validate_effect: AdapterEffect,
    apply_effect: AdapterEffect,
    pending: RecoveredDecisionApplyPendingLineageV1,
}
/// Closed staged adapter plus exact recovered-Decision logical lineage.
/// The authenticated Fetch stays attached; consumers receive only fixed
/// comparison oracles until the final publication transaction.
#[must_use = "recovered Decision storage projection must enter exact publication"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyStagedStorageV1 {
    adapter: SumeragiV2Adapter,
    fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
    lineage: RecoveredDecisionApplyCandidateLineageV1,
    apply_effect: AdapterEffect,
    apply_pending: PendingRuntimeEffectBinding,
    validated_receipt: ValidatedBodyReceipt,
}
/// Heap-retained handoff from recovered Decision replay to durable owner open.
///
/// The adapter fast-forward and the later ledger/registry admission are kept in
/// separate stack frames while this seal preserves the exact staged projection
/// and its already-revalidated body store.
#[must_use = "prepared recovered Decision Apply startup must open its exact owner"]
struct PreparedRecoveredDecisionApplyOwnerOpenV1 {
    staged: Box<RecoveredDecisionApplyStagedStorageV1>,
    body_store: super::v2_body_store::RevalidatedV2BodyStore,
}
/// Dedicated closed registry carrier for one recovered Decision Apply.
/// It keeps the original WAL Fetch and body lineage inseparable from Apply.
#[must_use = "a recovered Decision Apply carrier must remain in exact startup"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyRegistryCarrierV1 {
    context: LifecycleContext,
    fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
    lineage: RecoveredDecisionApplyCandidateLineageV1,
    apply_effect: AdapterEffect,
    apply_pending: PendingRuntimeEffectBinding,
    validated_receipt: ValidatedBodyReceipt,
    source: RecoveredDecisionApplyLedgerSourceV1,
}
/// Durable ledger authority retained by one recovered Decision Apply carrier.
#[must_use = "recovered Decision Apply ledger authority must remain attached"]
enum RecoveredDecisionApplyLedgerSourceV1 {
    /// The current Decision owns the ordinary Fetch/Store/Validate/Apply chain.
    FullChain,
    /// An older successful Validate is an immutable no-successor tombstone.
    ReleasedTerminal(AuthenticatedRecoveredReleasedValidateNoSuccessorV1),
}
/// Exact lifecycle Apply completion projected by the installed registry carrier.
/// Finality remains bound to the exact lineage, Apply tag, and dispatch key.
#[must_use = "lifecycle Apply completion authority must enter the adapter preview"]
pub(in crate::sumeragi) struct LifecycleDecisionApplyAdapterCompletionAuthorityV1 {
    tag: reducer::EventTag,
    subject: wire::BlockSubject,
    dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
    validate_predecessor_ordinal: u128,
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
}
impl LifecycleDecisionApplyAdapterCompletionAuthorityV1 {
    /// Build recovered completion authority for queue-preflight regression tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn recovered_for_queue_preflight_test(
        tag: reducer::EventTag,
        subject: wire::BlockSubject,
        dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
        validate_predecessor_ordinal: u128,
        artifact: wire::finality::V2FinalityArtifact,
    ) -> Self {
        assert_ne!(validate_predecessor_ordinal, 0);
        assert!(validate_predecessor_ordinal < dispatch_key.lifecycle_ordinal());
        Self {
            tag,
            subject,
            dispatch_key: dispatch_key
                .with_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered),
            validate_predecessor_ordinal,
            receipt: KuraV2CommitReceipt::for_test(&artifact),
            artifact,
        }
    }

    /// Return the immutable registry lineage without exposing completion material.
    pub(in crate::sumeragi) const fn lineage(&self) -> LifecycleDecisionApplyLineageV1 {
        self.dispatch_key.lineage()
    }
    /// Return the complete immutable worker/registry ownership key.
    pub(in crate::sumeragi) const fn dispatch_key(&self) -> LifecycleDecisionApplyDispatchKeyV1 {
        self.dispatch_key
    }
    /// Return the exact durable Validate row which advanced into this Apply.
    pub(in crate::sumeragi) const fn validate_predecessor_ordinal(&self) -> u128 {
        self.validate_predecessor_ordinal
    }
    /// Return the reducer incarnation authorized by the installed carrier.
    pub(in crate::sumeragi) const fn tag(&self) -> reducer::EventTag {
        self.tag
    }
    /// Return the exact decided block subject.
    pub(in crate::sumeragi) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }
    /// Borrow the Kura receipt checked against the installed carrier.
    pub(in crate::sumeragi) const fn receipt(&self) -> &KuraV2CommitReceipt {
        &self.receipt
    }
    /// Borrow the finality artifact checked against the installed carrier.
    pub(in crate::sumeragi) const fn artifact(&self) -> &wire::finality::V2FinalityArtifact {
        &self.artifact
    }

    /// Replace only the dispatch lineage for closed executor non-substitution tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn substitute_dispatch_lineage_for_test(
        &mut self,
        lineage: LifecycleDecisionApplyLineageV1,
    ) {
        self.dispatch_key = self.dispatch_key.with_lineage_for_test(lineage);
    }
}
/// Adapter-private one-shot permit for unpacking a guarded recovered Sign result.
///
/// The worker owns the only constructor for the guarded material and this
/// module owns the only constructor for the permit. Consequently no sibling
/// can combine caller-supplied request/signature bytes into the reducer preview.
pub(in crate::sumeragi) struct RecoveredLifecycleSignAdapterCompletionPermitV1 {
    _linearity: RecoveredLifecycleSignAdapterCompletionPermitLinearityV1,
}
struct RecoveredLifecycleSignAdapterCompletionPermitLinearityV1;
impl Drop for RecoveredLifecycleSignAdapterCompletionPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleSignAdapterCompletionPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleSignAdapterCompletionPermitLinearityV1,
        }
    }
}
/// Adapter-private one-shot permit for sealing a follow-on recovered Vote Sign.
///
/// Replay evidence is structurally comparable, but only the adapter can mint
/// this permit after reproducing the exact `Signed` reducer transition and
/// authenticating the WAL frontier which owns the successor.
pub(in crate::sumeragi) struct RecoveredLifecycleNextWalVoteSealPermitV1 {
    _linearity: RecoveredLifecycleNextWalVoteSealPermitLinearityV1,
}
struct RecoveredLifecycleNextWalVoteSealPermitLinearityV1;
impl Drop for RecoveredLifecycleNextWalVoteSealPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextWalVoteSealPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleNextWalVoteSealPermitLinearityV1,
        }
    }
}
/// Opaque body lookup derived from the reducer's exact follow-on Vote Sign.
///
/// The lookup carries only the immutable Vote/body coordinates and the signed
/// Proposal manifest hash, when the parent Broadcast is a Proposal. It has no
/// receipt or coordinate accessors. Only the production service/executor join
/// or the cold body-store-private join may consume it to select one exact
/// validated body-store owner.
#[must_use = "a recovered next-Vote body lookup must enter the exact production service"]
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyLookupV1 {
    round: wire::ConsensusRound,
    proposal_round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
    expected_proposal_manifest_hash: Option<HashOf<wire::PayloadManifest>>,
}
impl RecoveredLifecycleNextVoteBodyLookupV1 {
    fn from_adapter_preview(
        next_sign: &AdapterEffect,
        expected_proposal_manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    ) -> Option<Self> {
        let AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        } = next_sign
        else {
            return None;
        };
        if !vote.signature.is_empty()
            || vote.round.context_id != vote.proposal_round.context_id
            || vote.round.height != vote.proposal_round.height
            || vote.execution_commitment.validate().is_err()
        {
            return None;
        }
        Some(Self {
            round: vote.round,
            proposal_round: vote.proposal_round,
            subject: vote.subject,
            execution_commitment: vote.execution_commitment,
            expected_proposal_manifest_hash,
        })
    }
    /// Compare the lookup with one immutable height/service owner.
    pub(in crate::sumeragi) fn matches_height_context(
        &self,
        context: &wire::HeightContext,
    ) -> bool {
        self.round.context_id == context.id()
            && self.round.height == context.height
            && self.proposal_round.context_id == context.id()
            && self.proposal_round.height == context.height
    }
    /// Compare one executor-retained validated receipt without exposing its key.
    pub(in crate::sumeragi) fn matches_validated_body(
        &self,
        validated: &ValidatedBodyReceipt,
    ) -> bool {
        let durable = validated.durable();
        durable.context_id() == self.round.context_id
            && durable.round() == self.proposal_round
            && durable.subject() == self.subject
            && validated.execution_commitment() == self.execution_commitment
            && self
                .expected_proposal_manifest_hash
                .is_none_or(|expected| durable.manifest_hash() == expected)
    }
    /// Compare the recovered manifest and durable catalog entry as one owner.
    pub(in crate::sumeragi) fn matches_recovered_body(
        &self,
        manifest: &wire::PayloadManifest,
        durable: &DurableBodyReceipt,
    ) -> bool {
        manifest.round == self.proposal_round
            && manifest.subject == self.subject
            && durable.context_id() == self.round.context_id
            && durable.round() == self.proposal_round
            && durable.subject() == self.subject
            && durable.manifest_hash() == HashOf::new(manifest)
            && self
                .expected_proposal_manifest_hash
                .is_none_or(|expected| HashOf::new(manifest) == expected)
    }
    fn matches_adapter_successor(
        &self,
        next_sign: &AdapterEffect,
        expected_proposal_manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    ) -> bool {
        let AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        } = next_sign
        else {
            return false;
        };
        vote.signature.is_empty()
            && vote.round == self.round
            && vote.proposal_round == self.proposal_round
            && vote.subject == self.subject
            && vote.execution_commitment == self.execution_commitment
            && expected_proposal_manifest_hash == self.expected_proposal_manifest_hash
    }
    #[cfg(test)]
    /// Build one inert lookup from a test-only unsigned Vote projection.
    pub(in crate::sumeragi) fn for_test(
        vote: &wire::Vote,
        expected_proposal_manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    ) -> Option<Self> {
        Self::from_adapter_preview(
            &AdapterEffect::Sign {
                tag: reducer::EventTag::new(
                    vote.round.height,
                    vote.round.view,
                    reducer::Generation::new(vote.round.height),
                ),
                request: SignRequest::Vote(vote.clone()),
            },
            expected_proposal_manifest_hash,
        )
    }
}
/// Adapter-private permit for consuming one executor-authenticated body owner.
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyConsumePermitV1 {
    _linearity: RecoveredLifecycleNextVoteBodyConsumePermitLinearityV1,
}
struct RecoveredLifecycleNextVoteBodyConsumePermitLinearityV1;
impl Drop for RecoveredLifecycleNextVoteBodyConsumePermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextVoteBodyConsumePermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleNextVoteBodyConsumePermitLinearityV1,
        }
    }
}
/// Move-only exact validated-body owner authenticated by a production owner.
///
/// Construction requires either the executor-private live permit or the body
/// store's cold-recovery permit after the exact validated, durable, and
/// recovered manifest catalogs have rejoined one store instance. The adapter
/// can consume this value only while rejoining the same next Sign and signed
/// Proposal manifest; no raw receipt or parts API is exposed.
#[must_use = "an authenticated next-Vote body owner must enter the adapter preview"]
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyAuthorityV1 {
    lookup: RecoveredLifecycleNextVoteBodyLookupV1,
    validated: ValidatedBodyReceipt,
    body_store_identity: V2BodyStoreInstanceIdentity,
}
impl RecoveredLifecycleNextVoteBodyAuthorityV1 {
    /// Mint only after the exact production executor catalog join succeeds.
    pub(in crate::sumeragi) fn from_exact_executor(
        _permit: super::v2_effects::RecoveredLifecycleNextVoteBodyAuthorityMintPermitV1,
        lookup: RecoveredLifecycleNextVoteBodyLookupV1,
        validated: ValidatedBodyReceipt,
        body_store_identity: V2BodyStoreInstanceIdentity,
    ) -> Option<Self> {
        lookup.matches_validated_body(&validated).then_some(Self {
            lookup,
            validated,
            body_store_identity,
        })
    }
    /// Mint only after the cold body store rejoined its exact recovered catalogs.
    ///
    /// The non-clone permit is constructible only inside the body-store module;
    /// callers cannot promote a receipt or a comparison-only lookup directly.
    pub(in crate::sumeragi) fn from_exact_revalidated_body_store(
        _permit: RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitV1,
        lookup: RecoveredLifecycleNextVoteBodyLookupV1,
        validated: ValidatedBodyReceipt,
        body_store_identity: V2BodyStoreInstanceIdentity,
    ) -> Option<Self> {
        lookup.matches_validated_body(&validated).then_some(Self {
            lookup,
            validated,
            body_store_identity,
        })
    }
    fn consume_for_adapter(
        self,
        _permit: RecoveredLifecycleNextVoteBodyConsumePermitV1,
        next_sign: &AdapterEffect,
        expected_proposal_manifest_hash: Option<HashOf<wire::PayloadManifest>>,
        expected_body_store_identity: &V2BodyStoreInstanceIdentity,
    ) -> Result<ValidatedBodyReceipt, AdapterError> {
        let Self {
            lookup,
            validated,
            body_store_identity,
        } = self;
        if !body_store_identity.same_instance(expected_body_store_identity)
            || !lookup.matches_adapter_successor(next_sign, expected_proposal_manifest_hash)
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        Ok(validated)
    }
    #[cfg(test)]
    fn for_test(
        lookup: RecoveredLifecycleNextVoteBodyLookupV1,
        validated: ValidatedBodyReceipt,
        body_store_identity: V2BodyStoreInstanceIdentity,
    ) -> Option<Self> {
        lookup.matches_validated_body(&validated).then_some(Self {
            lookup,
            validated,
            body_store_identity,
        })
    }
    /// Compare the retained receipt/store owner without exposing either part.
    #[cfg(test)]
    pub(in crate::sumeragi) fn exactly_matches_for_test(
        &self,
        validated: &ValidatedBodyReceipt,
        body_store_identity: &V2BodyStoreInstanceIdentity,
    ) -> bool {
        self.validated == *validated && self.body_store_identity.same_instance(body_store_identity)
    }
}
/// Closed reducer successor shape produced by one exact recovered signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) enum RecoveredLifecycleSignAdapterSuccessorShapeV1 {
    /// The signed message emitted only its mandatory Broadcast.
    Broadcast,
    /// The Broadcast was followed by one already-WAL-authorized Sign.
    BroadcastAndSign,
    /// A signed local Proposal emitted Broadcast plus a new Prepare-intent WAL write.
    ProposalPrepareWal,
}

/// Closed structural family used to choose one recovered Sign settler.
///
/// This is a selection-only oracle over the publication-inert adapter preview.
/// In particular, it does not claim that the follow-on Vote has rejoined its
/// body-store, output, WAL, or registry authorities; the chosen settler must
/// still authenticate those affine owners before publication.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) enum RecoveredLifecycleSignAdapterSettlementFamilyV1 {
    /// The signed message emitted only its mandatory Broadcast.
    Broadcast,
    /// A local Proposal must first append its exact Prepare-intent WAL record.
    ProposalPrepareWal,
    /// A signed Prepare Vote emitted Broadcast followed by Commit Sign.
    VoteBroadcastAndSign,
    /// A WAL-ahead signed Proposal emitted Broadcast followed by Prepare Sign.
    ProposalBroadcastAndSign,
}
/// Adapter-authenticated authority for projecting one recovered Broadcast child.
///
/// Unlike the output projection, this value retains the adapter effect in its
/// lifecycle form so the original WAL carrier can derive the exact pending
/// binding and durable replay admission. Only the WAL-recovery module can
/// unpack it, and only after the adapter has cryptographically verified the
/// worker signature and reproduced the reducer's mandatory Broadcast.
#[must_use = "authenticated recovered Broadcast must rejoin its WAL carrier"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastProjectionAuthorityV1 {
    dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
    broadcast: AdapterEffect,
}
/// Adapter-authenticated signed Proposal plus its exact canonical payload.
///
/// The worker-restored payload cannot be exposed independently from the
/// cryptographically checked Proposal.  The production output service consumes
/// this move-only value to prepare the control and chunk fanouts under one
/// exact-output corridor lock; no message, payload, or parts accessor exists.
#[must_use = "recovered Proposal output must enter its atomic exact-output corridor"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputAuthorityV1 {
    dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
    tag: reducer::EventTag,
    proposal: wire::ConsensusMessageV2,
    payload: super::v2_chunks::EncodedV2Payload,
    body_store_identity: V2BodyStoreInstanceIdentity,
    output_guard: Arc<super::output_guard::ConsensusOutputGuard>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleProposalExactOutputAuthorityV1 {
    fn validated(
        context: &wire::HeightContext,
        dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        tag: reducer::EventTag,
        proposal: wire::ConsensusMessageV2,
        payload: super::v2_chunks::EncodedV2Payload,
        body_store_identity: V2BodyStoreInstanceIdentity,
        output_guard: Arc<super::output_guard::ConsensusOutputGuard>,
    ) -> Option<Self> {
        let wire::ConsensusMessageV2Payload::Proposal(signed) = &proposal.payload else {
            return None;
        };
        (dispatch_key.matches_height_context(context)
            && signed.round.context_id == context.id()
            && signed.round.height == context.height
            && tag.height() == signed.round.height
            && tag.view() == signed.round.view
            && !signed.signature.is_empty()
            && payload.manifest() == &signed.manifest)
            .then_some(Self {
                dispatch_key,
                tag,
                proposal,
                payload,
                body_store_identity,
                output_guard,
            })
    }
    /// Consume only through the production service's private permit.
    pub(in crate::sumeragi) fn consume_for_service(
        self,
        _permit: super::v2_worker::RecoveredLifecycleProposalExactOutputPermitV1,
    ) -> (
        super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        reducer::EventTag,
        wire::ConsensusMessageV2,
        super::v2_chunks::EncodedV2Payload,
        V2BodyStoreInstanceIdentity,
        Arc<super::output_guard::ConsensusOutputGuard>,
    ) {
        (
            self.dispatch_key,
            self.tag,
            self.proposal,
            self.payload,
            self.body_store_identity,
            self.output_guard,
        )
    }
    /// Reconstitute the same opaque authority after a capacity-only retry cut.
    ///
    /// Only the service-private permit can invoke this path. The immutable
    /// context, tag, signed Proposal, and manifest join are all rechecked, so
    /// the service cannot turn unpacked bytes into a different authority.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn from_service_retry(
        _permit: super::v2_worker::RecoveredLifecycleProposalExactOutputPermitV1,
        context: &wire::HeightContext,
        dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        tag: reducer::EventTag,
        proposal: wire::ConsensusMessageV2,
        payload: super::v2_chunks::EncodedV2Payload,
        body_store_identity: V2BodyStoreInstanceIdentity,
        output_guard: Arc<super::output_guard::ConsensusOutputGuard>,
    ) -> Option<Self> {
        Self::validated(
            context,
            dispatch_key,
            tag,
            proposal,
            payload,
            body_store_identity,
            output_guard,
        )
    }
    /// Build one exact authority for focused output-corridor behavior tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        context: &wire::HeightContext,
        dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        tag: reducer::EventTag,
        proposal: wire::ConsensusMessageV2,
        payload: super::v2_chunks::EncodedV2Payload,
        body_store_identity: V2BodyStoreInstanceIdentity,
        output_guard: Arc<super::output_guard::ConsensusOutputGuard>,
    ) -> Option<Self> {
        Self::validated(
            context,
            dispatch_key,
            tag,
            proposal,
            payload,
            body_store_identity,
            output_guard,
        )
    }
}
/// Adapter-authenticated combined successor of one recovered signature.
///
/// The mandatory signed Broadcast remains paired with the reducer's exact
/// follow-on Vote Sign. The latter is already sealed to its latest matching
/// authenticated WAL frame and exact validated body receipt. Only WAL recovery
/// can unpack this value, and no independent Broadcast or Sign accessor exists.
#[must_use = "combined recovered Broadcast and Sign authority must remain inseparable"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastAndSignAuthorityV1 {
    dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
    broadcast: AdapterEffect,
    next_sign: RecoveredLifecycleNextWalVoteSealV1,
}
/// WAL- and Ledger-authenticated input for replaying one fsynced `Signed` event.
///
/// This authority is intentionally narrower than a worker completion: it has
/// no dispatch key, output ownership, or acknowledgement. The recovered WAL
/// parent is the sole mint, after the signed child has rejoined the exact
/// durable continuation and verified-height roster. Proposal is excluded
/// because its reducer successor also requires body chunks and a Prepare WAL.
#[must_use = "cold signed Broadcast authority must advance its exact adapter startup"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignColdAdapterAuthorityV1 {
    tag: reducer::EventTag,
    request: SignRequest,
    signature: Vec<u8>,
    broadcast: AdapterEffect,
}
/// Comparison-only authority for advancing a cold adapter to the next Sign.
///
/// The complete executable Broadcast-and-Sign projection remains owned by WAL
/// recovery. This token carries only cloned semantic witnesses and cannot
/// install registry work or expose either effect to callers.
#[must_use = "cold Broadcast-and-Sign authority must advance the exact adapter startup"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1 {
    broadcast: AdapterEffect,
    next_sign: AdapterEffect,
}
/// WAL-authenticated historical signature input for a cold two-child preview.
///
/// This authority has no dispatch key or worker completion. WAL recovery is
/// the sole mint: its private permit binds the unsigned request to the exact
/// signed Proposal or Prepare-vote Broadcast already reconstructed from the
/// durable parent. The cold adapter consumes it without publishing either
/// reducer child.
#[must_use = "a cold signed-Broadcast preview authority must enter the adapter"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1 {
    tag: reducer::EventTag,
    request: SignRequest,
    signature: Vec<u8>,
    broadcast: AdapterEffect,
}
/// Unpublished cold replay of one historical signature's two exact children.
///
/// The original adapter startup remains unmodified and owned by this value.
/// The next Vote Sign can only project an opaque body lookup through the body
/// store's private permit, and the lookup is affine even when body
/// authentication fails. Dropping this preview publishes no effect, WAL,
/// registry, status, or output mutation.
#[must_use = "a cold Broadcast-and-Sign preview must authenticate its next body"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1 {
    startup: ProductionLifecycleAdapterStartupV1,
    broadcast: AdapterEffect,
    next_sign: AdapterEffect,
    expected_proposal_manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    body_store_identity: Option<V2BodyStoreInstanceIdentity>,
    cold_proposal_output: Option<RecoveredLifecycleColdProposalOutputV1>,
    body_lookup_minted: bool,
}
/// Canonical Proposal payload retained for cold exact-output reconstruction.
///
/// The value is minted only while the next Vote rejoins the same revalidated
/// body-store instance. It exposes neither chunks nor process identity; the
/// launched output service may consume it only through its private permit.
#[derive(Clone)]
#[must_use = "cold recovered Proposal output must remain with its durable Broadcast"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredLifecycleColdProposalOutputV1 {
    payload: super::v2_chunks::EncodedV2Payload,
    body_store_identity: V2BodyStoreInstanceIdentity,
}
/// Cold adapter/body seal awaiting the frame-bound WAL/Ledger join.
///
/// The adapter startup is still at the historical pre-signature state. The
/// Broadcast and follow-on Vote Sign remain inseparable, with the latter
/// sealed to its exact WAL frame and semantically revalidated body. Only WAL
/// recovery can unpack this value; there is no general parts API.
#[must_use = "a cold Broadcast-and-Sign seal must enter WAL/Ledger recovery"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredLifecycleSignedBroadcastAndSignColdSealV1 {
    startup: ProductionLifecycleAdapterStartupV1,
    broadcast: AdapterEffect,
    next_sign: RecoveredLifecycleNextWalVoteSealV1,
    cold_proposal_output: Option<RecoveredLifecycleColdProposalOutputV1>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1 {
    /// Seal one exact historical Proposal or Prepare-vote signature.
    ///
    /// The permit is constructible only by WAL recovery. Cryptographic and
    /// reducer-state verification is deliberately repeated by the consuming
    /// cold adapter preview.
    pub(in crate::sumeragi) fn from_recovered_wal(
        _permit: super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1,
        tag: reducer::EventTag,
        request: SignRequest,
        broadcast: AdapterEffect,
    ) -> Option<Self> {
        let AdapterEffect::Broadcast(message) = &broadcast else {
            return None;
        };
        let signature = match (&request, &message.payload) {
            (
                SignRequest::Proposal(unsigned),
                wire::ConsensusMessageV2Payload::Proposal(signed),
            ) => {
                let mut expected = unsigned.clone();
                if !expected.signature.is_empty()
                    || signed.signature.is_empty()
                    || tag.height() != unsigned.round.height
                    || tag.view() != unsigned.round.view
                {
                    return None;
                }
                expected.signature.clone_from(&signed.signature);
                (expected == *signed).then(|| signed.signature.clone())?
            }
            (SignRequest::Vote(unsigned), wire::ConsensusMessageV2Payload::Vote(signed)) => {
                let mut expected = unsigned.clone();
                if unsigned.phase != wire::GlobalPhase::Prepare
                    || !expected.signature.is_empty()
                    || signed.signature.is_empty()
                    || tag.height() != unsigned.round.height
                    || tag.view() != unsigned.round.view
                {
                    return None;
                }
                expected.signature.clone_from(&signed.signature);
                (expected == *signed).then(|| signed.signature.clone())?
            }
            (SignRequest::TimeoutVote(_), _)
            | (SignRequest::Proposal(_) | SignRequest::Vote(_), _) => return None,
        };
        Some(Self {
            tag,
            request,
            signature,
            broadcast,
        })
    }
}
impl RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1 {
    /// Seal the exact two-child relation under WAL recovery's private permit.
    pub(in crate::sumeragi) fn from_recovered_wal(
        _permit: super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1,
        broadcast: AdapterEffect,
        next_sign: AdapterEffect,
    ) -> Option<Self> {
        let AdapterEffect::Broadcast(message) = &broadcast else {
            return None;
        };
        let AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(next_vote),
        } = &next_sign
        else {
            return None;
        };
        let next_vote_tag_is_exact = match next_vote.phase {
            wire::GlobalPhase::Prepare => tag.view() == next_vote.round.view,
            wire::GlobalPhase::Commit => tag.view() >= next_vote.round.view,
        };
        if !next_vote.signature.is_empty()
            || tag.height() != next_vote.round.height
            || !next_vote_tag_is_exact
        {
            return None;
        }
        let relation_is_exact = match &message.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                !proposal.signature.is_empty()
                    && next_vote.phase == wire::GlobalPhase::Prepare
                    && next_vote.round == proposal.round
                    && next_vote.proposal_round == proposal.round
                    && next_vote.subject == proposal.subject
                    && next_vote.signer == proposal.proposer
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                !vote.signature.is_empty()
                    && vote.phase == wire::GlobalPhase::Prepare
                    && next_vote.phase == wire::GlobalPhase::Commit
                    && next_vote.round == vote.round
                    && next_vote.proposal_round == vote.proposal_round
                    && next_vote.subject == vote.subject
                    && next_vote.execution_commitment == vote.execution_commitment
                    && next_vote.signer == vote.signer
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(_)
            | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => false,
        };
        relation_is_exact.then_some(Self {
            broadcast,
            next_sign,
        })
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleColdProposalOutputV1 {
    /// Build canonical output for focused service routing tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        payload: super::v2_chunks::EncodedV2Payload,
        body_store_identity: V2BodyStoreInstanceIdentity,
    ) -> Self {
        Self {
            payload,
            body_store_identity,
        }
    }
    /// Compare two sealed payload owners without exposing either constituent.
    pub(in crate::sumeragi) fn exactly_matches(&self, other: &Self) -> bool {
        self.payload == other.payload
            && self
                .body_store_identity
                .same_instance(&other.body_store_identity)
    }
    /// Compare the retained payload with one exact signed Proposal Broadcast.
    pub(in crate::sumeragi) fn matches_broadcast(&self, broadcast: &AdapterEffect) -> bool {
        let AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
            ..
        }) = broadcast
        else {
            return false;
        };
        self.payload.manifest() == &proposal.manifest
    }
    /// Release canonical output only to the production service's private permit.
    pub(in crate::sumeragi) fn consume_for_service(
        self,
        _permit: super::v2_worker::RecoveredLifecycleProposalExactOutputPermitV1,
    ) -> (
        super::v2_chunks::EncodedV2Payload,
        V2BodyStoreInstanceIdentity,
    ) {
        (self.payload, self.body_store_identity)
    }
}
impl RecoveredLifecycleSignColdAdapterAuthorityV1 {
    /// Seal one exact Vote/Timeout signature under the WAL module's permit.
    pub(in crate::sumeragi) fn from_recovered_wal(
        _permit: super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1,
        tag: reducer::EventTag,
        request: SignRequest,
        broadcast: AdapterEffect,
    ) -> Option<Self> {
        let AdapterEffect::Broadcast(message) = &broadcast else {
            return None;
        };
        let signature = match (&request, &message.payload) {
            (SignRequest::Vote(unsigned), wire::ConsensusMessageV2Payload::Vote(signed)) => {
                let mut expected = unsigned.clone();
                if !expected.signature.is_empty() || signed.signature.is_empty() {
                    return None;
                }
                expected.signature.clone_from(&signed.signature);
                (expected == *signed).then(|| signed.signature.clone())?
            }
            (
                SignRequest::TimeoutVote(unsigned),
                wire::ConsensusMessageV2Payload::TimeoutVote(signed),
            ) => {
                let mut expected = unsigned.clone();
                if !expected.signature.is_empty() || signed.signature.is_empty() {
                    return None;
                }
                expected.signature.clone_from(&signed.signature);
                (expected == *signed).then(|| signed.signature.clone())?
            }
            (SignRequest::Proposal(_), _)
            | (SignRequest::Vote(_) | SignRequest::TimeoutVote(_), _) => return None,
        };
        Some(Self {
            tag,
            request,
            signature,
            broadcast,
        })
    }
}
impl RecoveredLifecycleSignBroadcastProjectionAuthorityV1 {
    /// Consume the sealed effect only through the WAL carrier's private permit.
    pub(in crate::sumeragi) fn consume_for_recovered_wal(
        self,
        _permit: super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1,
    ) -> (
        super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        AdapterEffect,
    ) {
        (self.dispatch_key, self.broadcast)
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1 {
    /// Project the reducer-derived next-body lookup only to the exact body store.
    ///
    /// The store supplies both the unforgeable bind permit and its process
    /// identity. A failed catalog join therefore consumes the sole lookup and
    /// leaves this preview inert rather than permitting fallback to another
    /// store instance.
    pub(in crate::sumeragi) fn project_next_vote_body_lookup(
        &mut self,
        _permit: RecoveredLifecycleNextVoteBodyColdPreviewBindPermitV1,
        body_store_identity: V2BodyStoreInstanceIdentity,
    ) -> Result<RecoveredLifecycleNextVoteBodyLookupV1, &'static str> {
        if self.body_lookup_minted || self.body_store_identity.is_some() {
            return Err("cold next-Vote body lookup was already projected");
        }
        let lookup = RecoveredLifecycleNextVoteBodyLookupV1::from_adapter_preview(
            &self.next_sign,
            self.expected_proposal_manifest_hash,
        )
        .ok_or("cold next-Vote body lookup changed its reducer successor")?;
        self.body_store_identity = Some(body_store_identity);
        self.body_lookup_minted = true;
        Ok(lookup)
    }
    /// Retain canonical Proposal output only from the exact body-store join.
    ///
    /// Prepare-vote parents have no payload fanout and therefore deliberately
    /// discard the reconstructed bytes after proving the same body catalog.
    pub(in crate::sumeragi) fn bind_cold_proposal_output(
        &mut self,
        _permit: RecoveredLifecycleColdProposalOutputMintPermitV1,
        payload: super::v2_chunks::EncodedV2Payload,
        body_store_identity: V2BodyStoreInstanceIdentity,
    ) -> Result<(), &'static str> {
        let retained_identity = self
            .body_store_identity
            .as_ref()
            .ok_or("cold Proposal output was bound before its next-body lookup")?;
        if !retained_identity.same_instance(&body_store_identity)
            || self.cold_proposal_output.is_some()
        {
            return Err("cold Proposal output changed its body-store owner");
        }
        let Some(expected_manifest_hash) = self.expected_proposal_manifest_hash else {
            return Ok(());
        };
        if HashOf::new(payload.manifest()) != expected_manifest_hash {
            return Err("cold Proposal output changed its signed manifest");
        }
        self.cold_proposal_output = Some(RecoveredLifecycleColdProposalOutputV1 {
            payload,
            body_store_identity,
        });
        Ok(())
    }
    /// Bind the previewed next Vote to its exact WAL frame and validated body.
    ///
    /// Success retains the still-unmodified adapter startup beside the sealed
    /// pair. Any failure is fail-stop: no adapter or storage mutation has been
    /// published, and no body authority can escape for a retry against another
    /// preview.
    pub(in crate::sumeragi) fn seal_recovered_lifecycle_next_wal_vote(
        self,
        body_authority: RecoveredLifecycleNextVoteBodyAuthorityV1,
    ) -> Result<RecoveredLifecycleSignedBroadcastAndSignColdSealV1, &'static str> {
        let Self {
            startup,
            broadcast,
            next_sign,
            expected_proposal_manifest_hash,
            body_store_identity,
            cold_proposal_output,
            body_lookup_minted,
        } = self;
        if !body_lookup_minted {
            return Err("cold next-Vote body lookup was not authenticated");
        }
        if cold_proposal_output.is_some() != expected_proposal_manifest_hash.is_some()
            || cold_proposal_output
                .as_ref()
                .is_some_and(|output| !output.matches_broadcast(&broadcast))
        {
            return Err("cold Proposal output did not rejoin its signed Broadcast");
        }
        let body_store_identity = body_store_identity
            .as_ref()
            .ok_or("cold next-Vote body store identity was not retained")?;
        let validated = body_authority
            .consume_for_adapter(
                RecoveredLifecycleNextVoteBodyConsumePermitV1::new(),
                &next_sign,
                expected_proposal_manifest_hash,
                body_store_identity,
            )
            .map_err(|_| "cold next-Vote body authority changed its exact owner")?;
        let ProductionLifecycleAdapterStartupStateV1::Recovered {
            adapter,
            effects,
            pending_kura_apply: None,
            local_proposal_attempt: _,
            leader_wire_launch_prepared: false,
        } = &startup.state
        else {
            return Err("cold next-Vote adapter startup changed after preview");
        };
        if !effects.is_empty() {
            return Err("cold next-Vote adapter startup retained residual effects");
        }
        let next_sign = adapter
            .authenticate_recovered_lifecycle_next_vote(
                &next_sign,
                &validated,
                expected_proposal_manifest_hash,
            )
            .map_err(|_| "cold next-Vote WAL/body seal is inconsistent")?;
        Ok(RecoveredLifecycleSignedBroadcastAndSignColdSealV1 {
            startup,
            broadcast,
            next_sign,
            cold_proposal_output,
        })
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleSignedBroadcastAndSignColdSealV1 {
    /// Release the sealed startup and pair only to WAL recovery's private permit.
    pub(in crate::sumeragi) fn consume_for_recovered_wal(
        self,
        _permit: super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1,
    ) -> (
        ProductionLifecycleAdapterStartupV1,
        AdapterEffect,
        RecoveredLifecycleNextWalVoteSealV1,
        Option<RecoveredLifecycleColdProposalOutputV1>,
    ) {
        (
            self.startup,
            self.broadcast,
            self.next_sign,
            self.cold_proposal_output,
        )
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleSignBroadcastAndSignAuthorityV1 {
    /// Consume the combined authority only through WAL recovery's one-shot permit.
    pub(in crate::sumeragi) fn consume_for_recovered_wal(
        self,
        _permit: super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1,
    ) -> (
        super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        AdapterEffect,
        RecoveredLifecycleNextWalVoteSealV1,
    ) {
        (self.dispatch_key, self.broadcast, self.next_sign)
    }
    /// Compare the complete combined projection in focused substitution tests.
    #[cfg(test)]
    fn exactly_matches_for_test(
        &self,
        dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        broadcast: &AdapterEffect,
        next_wal_identity: RecoveredWalFrameIdentity,
        next_sign: &AdapterEffect,
        validated: &ValidatedBodyReceipt,
    ) -> bool {
        self.dispatch_key == dispatch_key
            && self.broadcast == *broadcast
            && self
                .next_sign
                .exactly_matches(next_wal_identity, next_sign, validated)
    }
}
/// Borrow-bound preview of the recovered reducer's exact `Signed` transition.
///
/// The reducer and wire registry are cloned. No WAL append, output fanout,
/// service mutation, or live-adapter mutation occurs until a lifecycle
/// transaction has durably published the corresponding successor family.
/// Broadcast, already-WAL-ahead Broadcast-and-Sign, and the initial Proposal
/// Prepare-intent WAL cut all have sealed LedgerV1 consumers.  The initial
/// Proposal path reserves exact output before its WAL append and keeps that
/// fail-stop owner through the two-child Ledger publication.
#[must_use = "prepared recovered Sign completion has not crossed LedgerV1"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignAdapterCompletionV1<'a> {
    adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    core_effects: Vec<reducer::Effect>,
    broadcast: AdapterEffect,
    next_sign: Option<AdapterEffect>,
    combined_authority_minted: bool,
    proposal_output_authority_minted: bool,
    next_vote_body_store_identity: Option<V2BodyStoreInstanceIdentity>,
    next_vote_output_guard: Option<Arc<super::output_guard::ConsensusOutputGuard>>,
    pending_prepare: Option<(reducer::EventTag, reducer::WalEntry)>,
    prepared_prepare_wal: Option<PreparedRecoveredLifecycleProposalPrepareWalV1>,
    persisted_prepare_wal: Option<RecoveredLifecycleProposalPrepareWalContinuationV1>,
    outbound_payload: Option<super::v2_chunks::EncodedV2Payload>,
    next_fence_generation: u64,
    dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
}
/// Preflighted continuation of the initial local Proposal `PrepareIntent`.
///
/// This value is still pre-WAL: it owns only cloned reducer/registry-derived
/// state, the canonical encoded frame, and the exact future Prepare Sign.  It
/// cannot authorize either lifecycle child until the retained adapter appends
/// and reauthenticates that frame.
struct PreparedRecoveredLifecycleProposalPrepareWalV1 {
    next_reducer: reducer::Reducer,
    persisted_event: reducer::Event,
    sign_core_effect: reducer::Effect,
    sign_effect: AdapterEffect,
    expected_wal_sequence: u64,
    encoded_wal_payload: Vec<u8>,
    next_fence_generation: u64,
}
/// Exact post-append acknowledgement retained until LedgerV1 publication.
///
/// The adapter keeps its persistence identifier armed while this value exists.
/// The launched transaction must either publish the two children and consume
/// it in the assertion-only commit tail, or close admission for restart.
struct RecoveredLifecycleProposalPrepareWalContinuationV1 {
    persisted_event: reducer::Event,
    sign_core_effect: reducer::Effect,
    persistence_id: u64,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedRecoveredLifecycleSignAdapterCompletionV1<'_> {
    /// Return the closed reducer successor class for fixed lifecycle projection.
    pub(in crate::sumeragi) const fn shape(&self) -> RecoveredLifecycleSignAdapterSuccessorShapeV1 {
        if self.pending_prepare.is_some() {
            RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
        } else if self.next_sign.is_some() {
            RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
        } else {
            RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast
        }
    }

    /// Classify the sole legal settlement family without binding body authority.
    ///
    /// The returned family is suitable only for selecting one mutually
    /// exclusive lifecycle settler. The selected settler repeats the complete
    /// body/WAL/registry authentication before it may publish either child.
    pub(in crate::sumeragi) fn settlement_family(
        &self,
    ) -> Option<RecoveredLifecycleSignAdapterSettlementFamilyV1> {
        match self.shape() {
            RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast => {
                Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast)
            }
            RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal => {
                Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal)
            }
            RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign => {
                let AdapterEffect::Broadcast(message) = &self.broadcast else {
                    return None;
                };
                let Some(AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(next_vote),
                }) = &self.next_sign
                else {
                    return None;
                };
                let next_vote_tag_is_exact = match next_vote.phase {
                    wire::GlobalPhase::Prepare => tag.view() == next_vote.round.view,
                    wire::GlobalPhase::Commit => tag.view() >= next_vote.round.view,
                };
                if !next_vote.signature.is_empty()
                    || tag.height() != next_vote.round.height
                    || !next_vote_tag_is_exact
                {
                    return None;
                }
                match &message.payload {
                    wire::ConsensusMessageV2Payload::Proposal(proposal)
                        if !proposal.signature.is_empty()
                            && next_vote.phase == wire::GlobalPhase::Prepare
                            && next_vote.round == proposal.round
                            && next_vote.proposal_round == proposal.round
                            && next_vote.subject == proposal.subject
                            && next_vote.signer == proposal.proposer =>
                    {
                        Some(
                            RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign,
                        )
                    }
                    wire::ConsensusMessageV2Payload::Vote(vote)
                        if !vote.signature.is_empty()
                            && vote.phase == wire::GlobalPhase::Prepare
                            && next_vote.phase == wire::GlobalPhase::Commit
                            && next_vote.round == vote.round
                            && next_vote.proposal_round == vote.proposal_round
                            && next_vote.subject == vote.subject
                            && next_vote.execution_commitment == vote.execution_commitment
                            && next_vote.signer == vote.signer =>
                    {
                        Some(
                            RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign,
                        )
                    }
                    _ => None,
                }
            }
        }
    }
    /// Borrow only the exact signed Broadcast for focused adapter tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn broadcast_effect(&self) -> &AdapterEffect {
        &self.broadcast
    }
    /// Borrow the optional already-WAL-authorized follow-on Sign.
    pub(in crate::sumeragi) const fn next_sign_effect(&self) -> Option<&AdapterEffect> {
        self.next_sign.as_ref()
    }
    /// Return whether the unsealed successor is Prepare Broadcast then Commit Sign.
    ///
    /// This structural preflight deliberately does not require the affine
    /// combined WAL/body authority: the registry mints that authority only
    /// after it rejoins the exact claimed carrier. Post-seal publication must
    /// use [`Self::is_vote_broadcast_and_sign`].
    pub(in crate::sumeragi) fn is_vote_broadcast_and_sign_shape(&self) -> bool {
        if self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
            || self.next_vote_body_store_identity.is_none()
            || self.next_vote_output_guard.is_none()
            || self.outbound_payload.is_some()
            || self.pending_prepare.is_some()
        {
            return false;
        }
        let (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(broadcast),
                ..
            }),
            Some(AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(next),
            }),
        ) = (&self.broadcast, &self.next_sign)
        else {
            return false;
        };
        broadcast.phase == wire::GlobalPhase::Prepare
            && next.phase == wire::GlobalPhase::Commit
            && !broadcast.signature.is_empty()
            && next.signature.is_empty()
            && next.round == broadcast.round
            && next.proposal_round == broadcast.proposal_round
            && next.subject == broadcast.subject
            && next.execution_commitment == broadcast.execution_commitment
            && next.signer == broadcast.signer
            && tag.height() == next.round.height
            && tag.view() >= next.round.view
    }
    /// Return whether the exact Prepare-Broadcast/Commit-Sign authority is sealed.
    pub(in crate::sumeragi) fn is_vote_broadcast_and_sign(&self) -> bool {
        self.combined_authority_minted && self.is_vote_broadcast_and_sign_shape()
    }
    /// Return whether Proposal output and both child projections are sealed.
    pub(in crate::sumeragi) fn is_authorized_proposal_broadcast_and_sign(&self) -> bool {
        if self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
            || !self.combined_authority_minted
            || !self.proposal_output_authority_minted
            || self.next_vote_body_store_identity.is_none()
            || self.next_vote_output_guard.is_none()
            || self.pending_prepare.is_some()
        {
            return false;
        }
        let (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
                ..
            }),
            Some(payload),
        ) = (&self.broadcast, &self.outbound_payload)
        else {
            return false;
        };
        !proposal.signature.is_empty() && payload.manifest() == &proposal.manifest
    }
    /// Return the dedicated worker/registry identity retained by the preview.
    pub(in crate::sumeragi) const fn dispatch_key(
        &self,
    ) -> super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1 {
        self.dispatch_key
    }
    /// Mint the sole adapter-authenticated WAL/registry projection authority.
    pub(in crate::sumeragi) fn project_registry_broadcast_authority(
        &self,
    ) -> RecoveredLifecycleSignBroadcastProjectionAuthorityV1 {
        RecoveredLifecycleSignBroadcastProjectionAuthorityV1 {
            dispatch_key: self.dispatch_key,
            broadcast: self.broadcast.clone(),
        }
    }
    /// Seal the signed Proposal and worker-restored payload for atomic output.
    ///
    /// This projection is affine but does not consume the adapter preview: the
    /// same preview must remain borrowed through body/WAL authentication and
    /// the later LedgerV1 transaction.  Only Proposal shapes with the exact
    /// retained manifest may mint it.
    pub(in crate::sumeragi) fn project_proposal_exact_output_authority(
        &mut self,
    ) -> Result<RecoveredLifecycleProposalExactOutputAuthorityV1, AdapterError> {
        let shape = self.shape();
        if self.proposal_output_authority_minted
            || !matches!(
                shape,
                RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
                    | RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
            )
            || (shape == RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
                && self.prepared_prepare_wal.is_none())
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let AdapterEffect::Broadcast(proposal) = &self.broadcast else {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        };
        let wire::ConsensusMessageV2Payload::Proposal(signed) = &proposal.payload else {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        };
        let payload = self
            .outbound_payload
            .as_ref()
            .filter(|payload| payload.manifest() == &signed.manifest)
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let reducer::Event::Signed { tag, .. } = &self.event else {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        };
        if tag.height() != signed.round.height || tag.view() != signed.round.view {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let body_store_identity = self
            .next_vote_body_store_identity
            .as_ref()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let output_guard = self
            .next_vote_output_guard
            .as_ref()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        self.proposal_output_authority_minted = true;
        Ok(RecoveredLifecycleProposalExactOutputAuthorityV1 {
            dispatch_key: self.dispatch_key,
            tag: *tag,
            proposal: proposal.clone(),
            payload: payload.clone(),
            body_store_identity: body_store_identity.clone(),
            output_guard: Arc::clone(output_guard),
        })
    }
    fn broadcast_proposal_manifest_hash(
        &self,
    ) -> Result<Option<HashOf<wire::PayloadManifest>>, AdapterError> {
        match &self.broadcast {
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
                ..
            }) => Ok(Some(HashOf::new(&proposal.manifest))),
            AdapterEffect::Broadcast(_) => Ok(None),
            _ => Err(AdapterError::RecoveredLifecycleSignCompletionMismatch),
        }
    }
    /// Preflight the initial Proposal's exact `PrepareIntent -> Sign(Prepare)` cut.
    ///
    /// The WAL payload and acknowledgement continuation are derived entirely
    /// on cloned reducer state.  The returned body lookup is inert and remains
    /// bound to the same launched store/output owner; no WAL byte, adapter
    /// state, or lifecycle row changes until
    /// [`Self::append_recovered_lifecycle_proposal_prepare_wal`] succeeds.
    pub(in crate::sumeragi) fn prepare_proposal_prepare_wal_body_lookup(
        &mut self,
        _permit: super::v2_effects::RecoveredLifecycleNextVoteBodyPreviewBindPermitV1,
        body_store_identity: V2BodyStoreInstanceIdentity,
        output_guard: Arc<super::output_guard::ConsensusOutputGuard>,
    ) -> Result<RecoveredLifecycleNextVoteBodyLookupV1, AdapterError> {
        if self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
            || self.prepared_prepare_wal.is_some()
            || self.persisted_prepare_wal.is_some()
            || self.next_vote_body_store_identity.is_some()
            || self.next_vote_output_guard.is_some()
            || self.proposal_output_authority_minted
            || self.combined_authority_minted
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let (persist_tag, entry) = self
            .pending_prepare
            .as_ref()
            .cloned()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let expected_vote = match entry.record() {
            reducer::WalRecord::PrepareIntent(vote) if vote.phase() == reducer::Phase::Prepare => {
                *vote
            }
            _ => return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch),
        };
        if self.adapter.fail_closed
            || self.adapter.pending_persistence_id.is_some()
            || self.next_reducer.pending_persistence_record() != Some(entry.record())
            || self.next_reducer.awaiting_signature().is_some()
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let expected_wal_sequence = match self.adapter.wal.recovered_records().last() {
            Some(record) => record
                .sequence()
                .checked_add(1)
                .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?,
            None => 0,
        };
        if expected_wal_sequence.checked_add(1) != Some(entry.id().get()) {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let encoded_wal_payload = self
            .next_registry
            .encode_wal_entry(&entry, self.adapter.aggregator.as_ref())?;
        let pre_ack_fence = ReducerFenceProjection {
            pending_persistence: self.next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: self.next_reducer.awaiting_signature().cloned(),
            replay_complete: self.adapter.replay_complete,
        };
        let persisted_event = reducer::Event::Persisted {
            tag: persist_tag,
            id: entry.id(),
        };
        let mut next_reducer = self.next_reducer.clone();
        let continuation = next_reducer.step(persisted_event.clone())?;
        if continuation.disposition() != reducer::StepDisposition::Applied {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let mut continuation_effects = continuation.into_effects();
        if continuation_effects.len() != 1 {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let sign_core_effect = continuation_effects
            .pop()
            .expect("one checked Proposal persistence continuation remains");
        let sign_effect = match &sign_core_effect {
            reducer::Effect::Sign {
                tag,
                message: reducer::SignableMessage::Vote(vote),
            } if *tag == persist_tag && *vote == expected_vote => AdapterEffect::Sign {
                tag: *tag,
                request: SignRequest::Vote(self.next_registry.unsigned_vote_to_wire(*vote)?),
            },
            _ => return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch),
        };
        if next_reducer.pending_persistence_record().is_some()
            || next_reducer.awaiting_signature()
                != Some(&reducer::SignableMessage::Vote(expected_vote))
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let post_ack_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: self.adapter.replay_complete,
        };
        let next_fence_generation = if post_ack_fence == pre_ack_fence {
            self.next_fence_generation
        } else {
            self.next_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
        };
        let lookup = RecoveredLifecycleNextVoteBodyLookupV1::from_adapter_preview(
            &sign_effect,
            self.broadcast_proposal_manifest_hash()?,
        )
        .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        self.next_vote_body_store_identity = Some(body_store_identity);
        self.next_vote_output_guard = Some(output_guard);
        self.prepared_prepare_wal = Some(PreparedRecoveredLifecycleProposalPrepareWalV1 {
            next_reducer,
            persisted_event,
            sign_core_effect,
            sign_effect,
            expected_wal_sequence,
            encoded_wal_payload,
            next_fence_generation,
        });
        Ok(lookup)
    }
    /// Append and fsync the preflighted initial Proposal `PrepareIntent`.
    ///
    /// Success advances only this borrow-bound preview to its exact
    /// `Broadcast(Proposal) + Sign(Prepare)` successor.  The live adapter keeps
    /// the persistence identifier armed until LedgerV1 publishes both
    /// children.  Any append or receipt ambiguity fail-closes the adapter and
    /// requires cold recovery; it is never reported as a retryable error.
    pub(in crate::sumeragi) fn append_recovered_lifecycle_proposal_prepare_wal(
        &mut self,
        permit: super::v2_worker::RecoveredLifecycleProposalPrepareWalAppendPermitV1<'_>,
    ) -> Result<(), AdapterError> {
        if self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
            || self.prepared_prepare_wal.is_none()
            || self.persisted_prepare_wal.is_some()
            || !self.proposal_output_authority_minted
            || self.next_vote_body_store_identity.is_none()
            || self.next_vote_output_guard.is_none()
            || self.adapter.fail_closed
            || self.adapter.pending_persistence_id.is_some()
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        if !permit.authorizes(
            self.dispatch_key,
            self.next_vote_body_store_identity
                .as_ref()
                .expect("checked Proposal body-store identity remains retained"),
            self.next_vote_output_guard
                .as_ref()
                .expect("checked Proposal output guard remains retained"),
        ) {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let (persist_tag, entry) = self
            .pending_prepare
            .as_ref()
            .cloned()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let prepared = self
            .prepared_prepare_wal
            .as_ref()
            .expect("checked Proposal WAL preflight remains retained");
        if self
            .adapter
            .wal
            .recovered_records()
            .last()
            .map_or(prepared.expected_wal_sequence != 0, |record| {
                record.sequence().checked_add(1) != Some(prepared.expected_wal_sequence)
            })
            || prepared.expected_wal_sequence.checked_add(1) != Some(entry.id().get())
            || self.next_reducer.pending_persistence_record() != Some(entry.record())
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let PreparedRecoveredLifecycleProposalPrepareWalV1 {
            next_reducer,
            persisted_event,
            sign_core_effect,
            sign_effect,
            expected_wal_sequence,
            encoded_wal_payload,
            next_fence_generation,
        } = self
            .prepared_prepare_wal
            .take()
            .expect("checked Proposal WAL preflight remains retained");
        let persistence_id = entry.id().get();
        self.adapter.pending_persistence_id = Some(persistence_id);
        permit.cross_wal_attempt_boundary();
        let receipt = match self.adapter.wal.append(&encoded_wal_payload) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.adapter.fail_closed = true;
                return Err(error.into());
            }
        };
        let frame_sequence = receipt.sequence();
        let frame_hash = receipt.frame_hash();
        let post_wal: Result<SealedLiveWalPersistedEffectV1, AdapterError> = (|| {
            let frame = self.adapter.wal.recovered_records().last().ok_or(
                AdapterError::WalFrameIdentityMismatch {
                    frame_sequence,
                    persistence_id,
                    frame_hash,
                },
            )?;
            if frame_sequence != expected_wal_sequence
                || frame.payload() != encoded_wal_payload.as_slice()
            {
                return Err(AdapterError::WalFrameIdentityMismatch {
                    frame_sequence,
                    persistence_id,
                    frame_hash,
                });
            }
            let wal_identity =
                LiveWalFrameIdentity::from_append_receipt(frame, receipt, persistence_id).ok_or(
                    AdapterError::WalFrameIdentityMismatch {
                        frame_sequence,
                        persistence_id,
                        frame_hash,
                    },
                )?;
            let pending = PendingRuntimeEffectBinding::from_exact_live_wal_append(
                &wal_identity,
                &sign_effect,
            )
            .ok_or(AdapterError::LiveWalReplayCauseMismatch)?;
            SealedLiveWalPersistedEffectV1::from_exact_live_append(
                ExactLiveWalPersistedContinuationCause::PayloadFree {
                    wal_identity,
                    effect: sign_effect.clone(),
                    pending,
                },
            )
            .ok_or(AdapterError::LiveWalReplayCauseMismatch)
        })();
        let persisted_sign = match post_wal {
            Ok(persisted_sign) => persisted_sign,
            Err(error) => {
                self.adapter.fail_closed = true;
                return Err(error);
            }
        };
        drop(persisted_sign);
        self.next_reducer = next_reducer;
        self.next_sign = Some(sign_effect);
        self.pending_prepare = None;
        self.persisted_prepare_wal = Some(RecoveredLifecycleProposalPrepareWalContinuationV1 {
            persisted_event,
            sign_core_effect,
            persistence_id,
        });
        self.next_fence_generation = next_fence_generation;
        debug_assert!(matches!(
            &self.event,
            reducer::Event::Signed { tag, .. } if *tag == persist_tag
        ));
        Ok(())
    }
    /// Project an inert exact-body lookup for the reducer-produced next Vote.
    ///
    /// The production service must consume this lookup while exclusively
    /// borrowing the launched executor and rejoining the exact body-store
    /// instance. It contains no validated receipt and cannot authorize WAL or
    /// runtime work by itself.
    pub(in crate::sumeragi) fn project_broadcast_and_sign_body_lookup(
        &mut self,
        _permit: super::v2_effects::RecoveredLifecycleNextVoteBodyPreviewBindPermitV1,
        body_store_identity: V2BodyStoreInstanceIdentity,
        output_guard: Arc<super::output_guard::ConsensusOutputGuard>,
    ) -> Result<RecoveredLifecycleNextVoteBodyLookupV1, AdapterError> {
        if self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        if self.next_vote_body_store_identity.is_some() || self.next_vote_output_guard.is_some() {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let next_sign = self
            .next_sign
            .as_ref()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let lookup = RecoveredLifecycleNextVoteBodyLookupV1::from_adapter_preview(
            next_sign,
            self.broadcast_proposal_manifest_hash()?,
        )
        .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        self.next_vote_body_store_identity = Some(body_store_identity);
        self.next_vote_output_guard = Some(output_guard);
        Ok(lookup)
    }
    #[cfg(test)]
    fn project_broadcast_and_sign_body_lookup_for_test(
        &mut self,
        body_store_identity: V2BodyStoreInstanceIdentity,
        output_guard: Arc<super::output_guard::ConsensusOutputGuard>,
    ) -> Result<RecoveredLifecycleNextVoteBodyLookupV1, AdapterError> {
        if self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
            || self.next_vote_body_store_identity.is_some()
            || self.next_vote_output_guard.is_some()
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let next_sign = self
            .next_sign
            .as_ref()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let lookup = RecoveredLifecycleNextVoteBodyLookupV1::from_adapter_preview(
            next_sign,
            self.broadcast_proposal_manifest_hash()?,
        )
        .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        self.next_vote_body_store_identity = Some(body_store_identity);
        self.next_vote_output_guard = Some(output_guard);
        Ok(lookup)
    }
    /// Seal the exact Broadcast-and-Sign successor against WAL and body authority.
    ///
    /// Live settlement consumes this authority as one opaque two-child input;
    /// neither child is reconstructed from caller-supplied bytes. Cold owner
    /// assembly must independently rejoin the persisted pair before use.
    pub(in crate::sumeragi) fn project_broadcast_and_sign_authority(
        &mut self,
        body_authority: RecoveredLifecycleNextVoteBodyAuthorityV1,
    ) -> Result<RecoveredLifecycleSignBroadcastAndSignAuthorityV1, AdapterError> {
        if self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        if self.combined_authority_minted {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let next_sign = self
            .next_sign
            .as_ref()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let expected_manifest_hash = self.broadcast_proposal_manifest_hash()?;
        let expected_body_store_identity = self
            .next_vote_body_store_identity
            .as_ref()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let validated = body_authority.consume_for_adapter(
            RecoveredLifecycleNextVoteBodyConsumePermitV1::new(),
            next_sign,
            expected_manifest_hash,
            expected_body_store_identity,
        )?;
        let next_sign = if self.persisted_prepare_wal.is_some() {
            core::mem::swap(&mut self.adapter.reducer, &mut self.next_reducer);
            core::mem::swap(&mut self.adapter.registry, &mut self.next_registry);
            let authenticated = self.adapter.authenticate_recovered_lifecycle_next_vote(
                next_sign,
                &validated,
                expected_manifest_hash,
            );
            core::mem::swap(&mut self.adapter.registry, &mut self.next_registry);
            core::mem::swap(&mut self.adapter.reducer, &mut self.next_reducer);
            authenticated?
        } else {
            self.adapter.authenticate_recovered_lifecycle_next_vote(
                next_sign,
                &validated,
                expected_manifest_hash,
            )?
        };
        self.combined_authority_minted = true;
        Ok(RecoveredLifecycleSignBroadcastAndSignAuthorityV1 {
            dispatch_key: self.dispatch_key,
            broadcast: self.broadcast.clone(),
            next_sign,
        })
    }
    /// Exercise fail-closed next-Sign substitution without exposing production parts.
    #[cfg(test)]
    fn project_broadcast_and_substituted_sign_for_test(
        &mut self,
        next_sign: &AdapterEffect,
        body_authority: RecoveredLifecycleNextVoteBodyAuthorityV1,
    ) -> Result<RecoveredLifecycleSignBroadcastAndSignAuthorityV1, AdapterError> {
        if self.shape() != RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        if self.combined_authority_minted {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let expected_manifest_hash = self.broadcast_proposal_manifest_hash()?;
        let expected_body_store_identity = self
            .next_vote_body_store_identity
            .as_ref()
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let validated = body_authority.consume_for_adapter(
            RecoveredLifecycleNextVoteBodyConsumePermitV1::new(),
            next_sign,
            expected_manifest_hash,
            expected_body_store_identity,
        )?;
        let next_sign = self.adapter.authenticate_recovered_lifecycle_next_vote(
            next_sign,
            &validated,
            expected_manifest_hash,
        )?;
        self.combined_authority_minted = true;
        Ok(RecoveredLifecycleSignBroadcastAndSignAuthorityV1 {
            dispatch_key: self.dispatch_key,
            broadcast: self.broadcast.clone(),
            next_sign,
        })
    }
    /// Install an exact Broadcast-only reducer successor after LedgerV1 fsync.
    ///
    /// Follow-on Sign and Proposal-persistence shapes deliberately remain
    /// ineligible: their complete successor families require separate durable
    /// transactions. This tail performs only in-memory moves and accounting.
    pub(in crate::sumeragi) fn commit_after_durable_broadcast(self) {
        assert_eq!(
            self.shape(),
            RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast,
            "single-child Sign settlement cannot discard another reducer successor"
        );
        let Self {
            adapter,
            next_reducer,
            next_registry,
            event,
            core_effects,
            broadcast: _,
            next_sign: None,
            combined_authority_minted: _,
            proposal_output_authority_minted: _,
            next_vote_body_store_identity: _,
            next_vote_output_guard: _,
            pending_prepare: None,
            prepared_prepare_wal: None,
            persisted_prepare_wal: None,
            outbound_payload: None,
            next_fence_generation,
            dispatch_key: _,
        } = self
        else {
            unreachable!("Broadcast-only shape was asserted above")
        };
        let effect_count = core_effects.len();
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &core_effects);
        adapter.log_body_progress(&event, reducer::StepDisposition::Applied, effect_count);
    }
    /// Install an exact Proposal Broadcast-and-next-Sign reducer successor.
    ///
    /// Both affine projections must already have crossed their respective
    /// durable owners: LedgerV1 owns the Broadcast and WAL-backed Sign rows,
    /// while the exact-output reservation owns the Proposal control and chunk
    /// fanouts. This assertion-only tail cannot discard either prerequisite.
    pub(in crate::sumeragi) fn commit_after_durable_broadcast_and_sign(self) {
        assert_eq!(
            self.shape(),
            RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign,
            "combined Sign settlement requires exactly Broadcast and Sign"
        );
        let Self {
            adapter,
            next_reducer,
            next_registry,
            event,
            core_effects,
            broadcast: _,
            next_sign: Some(_),
            combined_authority_minted: true,
            proposal_output_authority_minted: true,
            next_vote_body_store_identity: Some(_),
            next_vote_output_guard: Some(_),
            pending_prepare: None,
            prepared_prepare_wal: None,
            persisted_prepare_wal,
            outbound_payload: Some(_),
            next_fence_generation,
            dispatch_key: _,
        } = self
        else {
            unreachable!("combined Proposal settlement retained every affine authority")
        };
        let effect_count = core_effects.len();
        match persisted_prepare_wal {
            None => assert_eq!(
                core_effects.len(),
                2,
                "WAL-ahead Proposal successor must contain Broadcast then Sign"
            ),
            Some(RecoveredLifecycleProposalPrepareWalContinuationV1 {
                persisted_event,
                sign_core_effect,
                persistence_id,
            }) => {
                assert_eq!(
                    core_effects.len(),
                    2,
                    "initial Proposal successor must contain Broadcast then PrepareIntent"
                );
                assert_eq!(
                    adapter.pending_persistence_id,
                    Some(persistence_id),
                    "initial Proposal WAL acknowledgement must remain armed through LedgerV1"
                );
                adapter.pending_persistence_id = None;
                adapter.reducer = next_reducer;
                adapter.registry = next_registry;
                adapter.reducer_fence_generation = next_fence_generation;
                adapter.record_reducer_outcome(
                    &event,
                    reducer::StepDisposition::Applied,
                    &core_effects,
                );
                adapter.log_body_progress(&event, reducer::StepDisposition::Applied, effect_count);
                adapter.record_reducer_outcome(
                    &persisted_event,
                    reducer::StepDisposition::Applied,
                    core::slice::from_ref(&sign_core_effect),
                );
                return;
            }
        }
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &core_effects);
        adapter.log_body_progress(&event, reducer::StepDisposition::Applied, effect_count);
    }
    /// Install an exact Prepare Broadcast-and-Commit-Sign reducer successor.
    ///
    /// Vote output remains owned by the durable Broadcast row and is refanned
    /// by the typed post-publication driver. Unlike Proposal, this mode has no
    /// canonical chunk payload or pre-fsync exact-output reservation.
    pub(in crate::sumeragi) fn commit_after_durable_vote_broadcast_and_sign(self) {
        assert!(
            self.is_vote_broadcast_and_sign(),
            "combined Vote settlement requires Prepare Broadcast then Commit Sign"
        );
        let Self {
            adapter,
            next_reducer,
            next_registry,
            event,
            core_effects,
            broadcast: _,
            next_sign: Some(_),
            combined_authority_minted: true,
            proposal_output_authority_minted: false,
            next_vote_body_store_identity: Some(_),
            next_vote_output_guard: Some(_),
            pending_prepare: None,
            prepared_prepare_wal: None,
            persisted_prepare_wal: None,
            outbound_payload: None,
            next_fence_generation,
            dispatch_key: _,
        } = self
        else {
            unreachable!("combined Vote settlement retained every affine authority")
        };
        assert_eq!(
            core_effects.len(),
            2,
            "combined Vote successor must contain Broadcast then Sign"
        );
        let effect_count = core_effects.len();
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &core_effects);
        adapter.log_body_progress(&event, reducer::StepDisposition::Applied, effect_count);
    }
}
/// Borrow-bound adapter successor for one registry-owned lifecycle Apply.
///
/// Preparation executes `ApplicationCompleted` only on cloned reducer and
/// registry state. The exact state and precomputed status remain inert until
/// LedgerV1 and the concrete registry terminal transition have committed.
#[must_use = "prepared lifecycle Apply completion has not crossed LedgerV1"]
pub(in crate::sumeragi) struct PreparedLifecycleDecisionApplyAdapterCompletionV1<'a> {
    adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    next_fence_generation: u64,
    dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
    validate_predecessor_ordinal: u128,
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
    committed_status: wire::SumeragiV2Status,
}
/// Post-Ledger finality values emitted only by the fixed adapter commit.
#[must_use = "lifecycle Apply finality must be installed in the exact executor"]
pub(in crate::sumeragi) struct LifecycleDecisionApplyAdapterFinalityV1 {
    dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
    validate_predecessor_ordinal: u128,
    tag: reducer::EventTag,
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
    committed_status: wire::SumeragiV2Status,
}
impl LifecycleDecisionApplyAdapterFinalityV1 {
    /// Consume this terminal only for the exact executor-owned finality install.
    pub(in crate::sumeragi) fn consume_for_executor(
        self,
        _permit: super::v2_effects::LifecycleDecisionApplyExecutorFinalityPermitV1,
    ) -> (
        LifecycleDecisionApplyDispatchKeyV1,
        u128,
        reducer::EventTag,
        KuraV2CommitReceipt,
        wire::finality::V2FinalityArtifact,
        wire::SumeragiV2Status,
    ) {
        (
            self.dispatch_key,
            self.validate_predecessor_ordinal,
            self.tag,
            self.receipt,
            self.artifact,
            self.committed_status,
        )
    }
}
/// Opaque projection failure retaining every staged input.
#[must_use = "failed recovered Decision storage projection requires restart"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyStorageProjectionErrorV1 {
    _staged: RecoveredDecisionApplyStagedAdapterV1,
    _fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
    _replay: super::v2_lifecycle_coordinator::RecoveredDecisionApplyReplayLineageV1,
}
/// One-shot proof that the fixed staged adapter still owns all body successors.
///
/// Replay projection consumes this permit while effects and predecessor-derived
/// bindings remain nested in [`RecoveredDecisionApplyStagedAdapterV1`].
pub(in crate::sumeragi) struct RecoveredDecisionApplyCandidateProjectionPermit {
    _linearity: RecoveredDecisionApplyCandidateProjectionLinearity,
}
struct RecoveredDecisionApplyCandidateProjectionLinearity;
impl Drop for RecoveredDecisionApplyCandidateProjectionLinearity {
    fn drop(&mut self) {}
}
/// Opaque failure retaining the original, unstaged cold adapter.
///
/// The caller receives only a stable diagnostic. A failed preview has no
/// fallback path; startup must reopen the WAL and body store from durability.
#[must_use = "a failed recovered Decision preview requires restart"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyAdapterStagingError {
    error: AdapterError,
    _adapter: SumeragiV2Adapter,
}
struct RecoveredDecisionApplyAdapterRollback {
    reducer: reducer::Reducer,
    registry: WireRegistry,
    reducer_fence_generation: u64,
    last_progress: Option<(
        reducer::Generation,
        reducer::Round,
        wire::SumeragiV2ProgressTransition,
    )>,
}
impl RecoveredDecisionApplyAdapterRollback {
    fn restore(self, adapter: &mut SumeragiV2Adapter) {
        adapter.reducer = self.reducer;
        adapter.registry = self.registry;
        adapter.reducer_fence_generation = self.reducer_fence_generation;
        adapter.last_progress = self.last_progress;
    }
}
impl RecoveredDecisionApplyAdapterStagingError {
    /// Retain the restored cold adapter without releasing a fallback surface.
    pub(in crate::sumeragi) fn retain(error: AdapterError, adapter: SumeragiV2Adapter) -> Self {
        Self {
            error,
            _adapter: adapter,
        }
    }
    /// Borrow the non-authorizing adapter diagnostic.
    pub(in crate::sumeragi) const fn error(&self) -> &AdapterError {
        &self.error
    }
}
impl RecoveredDecisionApplyStagedAdapterV1 {
    /// Recheck the fixed effect shapes, derived pending lineage, and final state.
    pub(in crate::sumeragi) fn validates(&self) -> bool {
        let (
            AdapterEffect::StoreBody {
                tag: store_tag,
                round: store_round,
                subject: store_subject,
            },
            AdapterEffect::ValidateBody {
                tag: validate_tag,
                round: validate_round,
                subject: validate_subject,
            },
            AdapterEffect::Apply {
                tag: apply_tag,
                subject: apply_subject,
                certificate,
            },
        ) = (
            &self.store_effect,
            &self.validate_effect,
            &self.apply_effect,
        )
        else {
            return false;
        };
        store_tag == validate_tag
            && validate_tag == apply_tag
            && store_round == validate_round
            && store_subject == validate_subject
            && validate_subject == apply_subject
            && certificate.phase == wire::GlobalPhase::Commit
            && certificate.proposal_round == *validate_round
            && certificate.subject == *apply_subject
            && self.pending.exactly_matches(
                &self.store_effect,
                &self.validate_effect,
                &self.apply_effect,
            )
            && self.adapter.reducer.durable_state().decision().is_some()
            && self.adapter.registry.execution_commitments.iter().any(
                |((round, subject), commitment)| {
                    self.adapter.registry.round_to_wire(*round) == *validate_round
                        && self.adapter.registry.subjects.get(subject) == Some(apply_subject)
                        && commitment == &certificate.execution_commitment
                },
            )
    }
    /// Consume the fixed preview into one closed durable lineage.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_storage_projection(
        self,
        verified: &VerifiedHeightContext,
        fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        replay: super::v2_lifecycle_coordinator::RecoveredDecisionApplyReplayLineageV1,
        durable: &DurableBodyReceipt,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<
        Box<RecoveredDecisionApplyStagedStorageV1>,
        RecoveredDecisionApplyStorageProjectionErrorV1,
    > {
        if !self.validates() || validated_receipt.durable() != durable {
            return Err(RecoveredDecisionApplyStorageProjectionErrorV1 {
                _staged: self,
                _fetch: fetch,
                _replay: replay,
            });
        }
        let Self {
            adapter,
            store_effect,
            validate_effect,
            apply_effect,
            pending,
        } = self;
        let projected = pending.project_candidate_lineage(
            RecoveredDecisionApplyCandidateProjectionPermit {
                _linearity: RecoveredDecisionApplyCandidateProjectionLinearity,
            },
            &replay,
            verified,
            durable,
            &store_effect,
            &validate_effect,
            &apply_effect,
            &fetch,
        );
        let (lineage, apply_pending) = match projected {
            Ok(projected) => projected,
            Err(pending) => {
                return Err(RecoveredDecisionApplyStorageProjectionErrorV1 {
                    _staged: RecoveredDecisionApplyStagedAdapterV1 {
                        adapter,
                        store_effect,
                        validate_effect,
                        apply_effect,
                        pending,
                    },
                    _fetch: fetch,
                    _replay: replay,
                });
            }
        };
        debug_assert!(fetch.owns_apply_lineage(verified, &lineage));
        drop(store_effect);
        drop(validate_effect);
        drop(replay);
        Ok(Box::new(RecoveredDecisionApplyStagedStorageV1 {
            adapter,
            fetch,
            lineage,
            apply_effect,
            apply_pending,
            validated_receipt: validated_receipt.clone(),
        }))
    }
}
impl RecoveredDecisionApplyStagedStorageV1 {
    /// Recheck the complete Fetch-to-Apply projection without releasing parts.
    pub(in crate::sumeragi) fn validates(&self, verified: &VerifiedHeightContext) -> bool {
        let mut context = [0_u8; 32];
        context.copy_from_slice(verified.context().id().0.as_ref());
        let context =
            LifecycleContext::new(LifecycleDigest::new(context), verified.context().height);
        self.fetch.owns_apply_lineage(verified, &self.lineage)
            && self
                .lineage
                .exactly_matches_validated_receipt(context, &self.validated_receipt)
            && self
                .apply_pending
                .exactly_binds_adapter_effect(&self.apply_effect)
            && matches!(
                &self.apply_effect,
                AdapterEffect::Apply { certificate, .. }
                    if certificate.execution_commitment
                        == self.validated_receipt.execution_commitment()
            )
    }
    /// Borrow the opaque logical lineage for fixed ledger and registry oracles.
    pub(in crate::sumeragi) const fn lineage(&self) -> &RecoveredDecisionApplyCandidateLineageV1 {
        &self.lineage
    }
    /// Borrow the original exact Fetch only for its fixed ledger-row oracle.
    pub(in crate::sumeragi) const fn fetch(
        &self,
    ) -> &AuthenticatedRecoveredWalDecisionFetchProjection {
        &self.fetch
    }
    /// Borrow the already-revalidated body receipt for exact cold ledger joins.
    pub(in crate::sumeragi) const fn validated_receipt(&self) -> &ValidatedBodyReceipt {
        &self.validated_receipt
    }
    /// Consume the staged adapter into the sole dedicated registry carrier.
    ///
    /// Only the concrete work registry can mint the projection permit. Failure
    /// returns the complete staged value and exposes no constituent authority.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_registry_carrier(
        self: Box<Self>,
        _permit: RecoveredDecisionApplyRegistryProjectionPermit,
        verified: &VerifiedHeightContext,
        effects: Vec<AdapterEffect>,
    ) -> Result<
        Box<(
            ProductionLifecycleAdapterStartupV1,
            RecoveredDecisionApplyRegistryCarrierV1,
        )>,
        (Box<Self>, Vec<AdapterEffect>),
    > {
        if !self.validates(verified) || !effects.is_empty() {
            return Err((self, effects));
        }
        let Self {
            adapter,
            fetch,
            lineage,
            apply_effect,
            apply_pending,
            validated_receipt,
        } = *self;
        let mut context = [0_u8; 32];
        context.copy_from_slice(verified.context().id().0.as_ref());
        Ok(Box::new((
            ProductionLifecycleAdapterStartupV1::recovered(adapter, effects),
            RecoveredDecisionApplyRegistryCarrierV1 {
                context: LifecycleContext::new(
                    LifecycleDigest::new(context),
                    verified.context().height,
                ),
                fetch,
                lineage,
                apply_effect,
                apply_pending,
                validated_receipt,
                source: RecoveredDecisionApplyLedgerSourceV1::FullChain,
            },
        )))
    }
    /// Consume the staged adapter beside one storage-authenticated released
    /// Validate tombstone.
    ///
    /// Failure returns every move-only authority to the caller. The current
    /// Decision retains its WAL Fetch only as body-lineage evidence; it does
    /// not authorize a fabricated Fetch/Store/Validate ledger chain.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_released_registry_carrier(
        self: Box<Self>,
        _permit: RecoveredDecisionApplyRegistryProjectionPermit,
        verified: &VerifiedHeightContext,
        effects: Vec<AdapterEffect>,
        released: AuthenticatedRecoveredReleasedValidateNoSuccessorV1,
    ) -> Result<
        Box<(
            ProductionLifecycleAdapterStartupV1,
            RecoveredDecisionApplyRegistryCarrierV1,
        )>,
        (
            Box<Self>,
            Vec<AdapterEffect>,
            AuthenticatedRecoveredReleasedValidateNoSuccessorV1,
        ),
    > {
        let mut context = [0_u8; 32];
        context.copy_from_slice(verified.context().id().0.as_ref());
        let context =
            LifecycleContext::new(LifecycleDigest::new(context), verified.context().height);
        if !self.validates(verified)
            || !effects.is_empty()
            || !released.exactly_matches_validated_receipt(context, &self.validated_receipt)
        {
            return Err((self, effects, released));
        }
        let Self {
            adapter,
            fetch,
            lineage,
            apply_effect,
            apply_pending,
            validated_receipt,
        } = *self;
        Ok(Box::new((
            ProductionLifecycleAdapterStartupV1::recovered(adapter, effects),
            RecoveredDecisionApplyRegistryCarrierV1 {
                context,
                fetch,
                lineage,
                apply_effect,
                apply_pending,
                validated_receipt,
                source: RecoveredDecisionApplyLedgerSourceV1::ReleasedTerminal(released),
            },
        )))
    }
}
impl RecoveredDecisionApplyRegistryCarrierV1 {
    fn exact_body_binding(&self) -> bool {
        self.lineage
            .exactly_matches_validated_receipt(self.context, &self.validated_receipt)
            && self
                .apply_pending
                .exactly_binds_adapter_effect(&self.apply_effect)
            && matches!(
                &self.apply_effect,
                AdapterEffect::Apply { certificate, .. }
                    if certificate.execution_commitment
                        == self.validated_receipt.execution_commitment()
            )
    }
    /// Recheck the immutable WAL/body lineage and final effect binding.
    pub(in crate::sumeragi) fn validates(&self, verified: &VerifiedHeightContext) -> bool {
        let mut context = [0_u8; 32];
        context.copy_from_slice(verified.context().id().0.as_ref());
        self.context
            == LifecycleContext::new(LifecycleDigest::new(context), verified.context().height)
            && self.fetch.owns_apply_lineage(verified, &self.lineage)
            && self.exact_body_binding()
            && match &self.source {
                RecoveredDecisionApplyLedgerSourceV1::FullChain => true,
                RecoveredDecisionApplyLedgerSourceV1::ReleasedTerminal(released) => released
                    .exactly_matches_validated_receipt(self.context, &self.validated_receipt),
            }
    }

    /// Rejoin this carrier to its exact full-chain or released-terminal ledger lineage.
    pub(in crate::sumeragi) fn validates_in_ledger(
        &self,
        verified: &VerifiedHeightContext,
        ledger: &LifecycleLedgerV1,
        installed_apply_ordinal: u128,
    ) -> bool {
        self.validates(verified)
            && match &self.source {
                RecoveredDecisionApplyLedgerSourceV1::FullChain => ledger
                    .exactly_matches_recovered_decision_apply_carrier(
                        &self.fetch,
                        &self.lineage,
                        installed_apply_ordinal,
                    ),
                RecoveredDecisionApplyLedgerSourceV1::ReleasedTerminal(released) => ledger
                    .exactly_matches_recovered_released_decision_apply_carrier(
                        &self.fetch,
                        &self.lineage,
                        released,
                        installed_apply_ordinal,
                    ),
            }
    }

    /// Rejoin a released source to the exact reconstructed terminal Validate.
    pub(in crate::sumeragi) fn validates_released_terminal_in_coordinator(
        &self,
        coordinator: &super::v2_lifecycle_coordinator::LifecycleCoordinator,
    ) -> bool {
        match &self.source {
            RecoveredDecisionApplyLedgerSourceV1::FullChain => true,
            RecoveredDecisionApplyLedgerSourceV1::ReleasedTerminal(released) => {
                released.matches_current_terminal_record(self.context, coordinator)
            }
        }
    }

    /// Authenticate the exact durable Validate row which advanced into this
    /// unchanged recovered Apply carrier.
    pub(in crate::sumeragi) fn validate_predecessor_ordinal_in_ledger(
        &self,
        ledger: &LifecycleLedgerV1,
        installed_apply_ordinal: u128,
    ) -> Option<u128> {
        self.exact_body_binding().then_some(())?;
        ledger.recovered_decision_apply_validate_predecessor_ordinal(
            &self.fetch,
            &self.lineage,
            installed_apply_ordinal,
        )
    }

    /// Return the attached physical digest.
    pub(in crate::sumeragi) fn installed_digest(&self) -> LifecycleDigest {
        LifecycleDigest::new(*self.apply_pending.exact_effect_identity().as_ref())
    }
    /// Return the immutable lifecycle context fixed by the verified projection.
    pub(in crate::sumeragi) const fn context(&self) -> LifecycleContext {
        self.context
    }
    /// Borrow the closed body lineage only for fixed recovery/record oracles.
    pub(in crate::sumeragi) const fn lineage(&self) -> &RecoveredDecisionApplyCandidateLineageV1 {
        &self.lineage
    }
    /// Compare a reconstructed candidate with the sole retained live Apply.
    pub(in crate::sumeragi) fn exactly_matches_candidate(
        &self,
        candidate: &CandidateAdmission,
    ) -> bool {
        self.exact_body_binding() && self.lineage.exactly_matches_apply_candidate(candidate)
    }

    /// Project a registry identity and exact Apply material into the worker task.
    pub(in crate::sumeragi) fn project_recovered_apply_task(
        &self,
        identity: LifecycleDecisionApplyDispatchIdentityV1,
        address: super::v2_lifecycle_coordinator::ConcreteWorkAddress,
    ) -> Option<super::v2_apply::LifecycleDecisionApplyTaskV1> {
        if !self.exact_body_binding()
            || !identity.matches_carrier(
                self.context,
                address,
                self.installed_digest(),
                LifecycleDecisionApplyLineageV1::Recovered,
            )
        {
            return None;
        }
        let AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        } = &self.apply_effect
        else {
            return None;
        };
        super::v2_apply::LifecycleDecisionApplyTaskV1::from_recovered_registry_projection(
            identity,
            *tag,
            *subject,
            certificate.clone(),
            self.validated_receipt.clone(),
        )
    }
    /// Bind one applied worker result back to this exact installed carrier.
    ///
    /// The registry-minted permit proves the active lease and in-flight key.
    /// This fixed projection independently rechecks the body receipt, CommitQC,
    /// Kura receipt, and finality artifact before any adapter state is staged.
    pub(in crate::sumeragi) fn project_recovered_apply_completion(
        &self,
        permit: LifecycleDecisionApplyCompletionProjectionPermitV1,
        address: super::v2_lifecycle_coordinator::ConcreteWorkAddress,
        validate_predecessor_ordinal: u128,
        completion: &super::v2_apply::LifecycleDecisionApplyCompletionV1,
    ) -> Option<LifecycleDecisionApplyAdapterCompletionAuthorityV1> {
        self.exact_body_binding().then_some(())?;
        project_lifecycle_decision_apply_completion(
            permit,
            LifecycleDecisionApplyLineageV1::Recovered,
            self.context,
            address,
            validate_predecessor_ordinal,
            self.installed_digest(),
            &self.apply_effect,
            &self.validated_receipt,
            completion,
        )
    }
}

/// Project a live Validate successor's guarded Apply result without exposing
/// the retained candidate, pending owner, or registry carrier.
pub(in crate::sumeragi) fn project_live_decision_apply_completion(
    permit: LifecycleDecisionApplyCompletionProjectionPermitV1,
    context: LifecycleContext,
    address: super::v2_lifecycle_coordinator::ConcreteWorkAddress,
    validate_predecessor_ordinal: u128,
    installed_digest: LifecycleDigest,
    effect: &AdapterEffect,
    validated_receipt: &ValidatedBodyReceipt,
    completion: &super::v2_apply::LifecycleDecisionApplyCompletionV1,
) -> Option<LifecycleDecisionApplyAdapterCompletionAuthorityV1> {
    project_lifecycle_decision_apply_completion(
        permit,
        LifecycleDecisionApplyLineageV1::Live,
        context,
        address,
        validate_predecessor_ordinal,
        installed_digest,
        effect,
        validated_receipt,
        completion,
    )
}

fn project_lifecycle_decision_apply_completion(
    _permit: LifecycleDecisionApplyCompletionProjectionPermitV1,
    lineage: LifecycleDecisionApplyLineageV1,
    context: LifecycleContext,
    address: super::v2_lifecycle_coordinator::ConcreteWorkAddress,
    validate_predecessor_ordinal: u128,
    installed_digest: LifecycleDigest,
    effect: &AdapterEffect,
    validated_receipt: &ValidatedBodyReceipt,
    completion: &super::v2_apply::LifecycleDecisionApplyCompletionV1,
) -> Option<LifecycleDecisionApplyAdapterCompletionAuthorityV1> {
    let AdapterEffect::Apply {
        tag,
        subject,
        certificate,
    } = effect
    else {
        return None;
    };
    let key = completion.dispatch_key();
    let artifact = completion.artifact();
    let receipt = completion.receipt();
    if !key.matches_carrier(context, address, installed_digest, lineage)
        || validate_predecessor_ordinal == 0
        || validate_predecessor_ordinal >= key.lifecycle_ordinal()
        || completion.subject() != *subject
        || completion.certificate() != certificate
        || completion.validated_receipt() != validated_receipt
        || certificate.execution_commitment != validated_receipt.execution_commitment()
        || artifact.validate().is_err()
        || !key.matches_height_context(&artifact.height_context)
        || artifact.subject != *subject
        || &artifact.commit_qc != certificate
        || receipt.height() != artifact.height_context.height
        || receipt.context_id() != artifact.height_context.id()
        || receipt.block_hash() != subject.block_hash
        || receipt.subject() != *subject
        || receipt.certificate() != certificate.as_ref()
        || receipt.artifact_hash() != HashOf::new(artifact)
    {
        return None;
    }
    Some(LifecycleDecisionApplyAdapterCompletionAuthorityV1 {
        tag: *tag,
        subject: *subject,
        dispatch_key: key,
        validate_predecessor_ordinal,
        receipt: receipt.clone(),
        artifact: artifact.clone(),
    })
}
impl PreparedLifecycleDecisionApplyAdapterCompletionV1<'_> {
    /// Install the already-checked reducer successor after durable settlement.
    ///
    /// This method contains only infallible moves and accounting. It returns a
    /// sealed finality value so the executor can record Kura ownership without
    /// synthesizing `RuntimeEffectOwnership` or an `EffectWorkId`.
    pub(in crate::sumeragi) fn commit_after_durable_settlement(
        self,
    ) -> LifecycleDecisionApplyAdapterFinalityV1 {
        let Self {
            adapter,
            next_reducer,
            next_registry,
            event,
            next_fence_generation,
            dispatch_key,
            validate_predecessor_ordinal,
            receipt,
            artifact,
            committed_status,
        } = self;
        let tag = match &event {
            reducer::Event::ApplicationCompleted { tag, .. } => *tag,
            _ => unreachable!("lifecycle Decision Apply preview retains ApplicationCompleted"),
        };
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &[]);
        adapter.log_body_progress(&event, reducer::StepDisposition::Applied, 0);
        LifecycleDecisionApplyAdapterFinalityV1 {
            dispatch_key,
            validate_predecessor_ordinal,
            tag,
            receipt,
            artifact,
            committed_status,
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectValidationSucceededApply<'_> {
    /// Borrow the exact canonical application effect derived from the Decision.
    const fn apply_effect(&self) -> &AdapterEffect {
        &self.apply_effect
    }
}
/// Borrow-bound applied validation which emits one exact safety-WAL request.
///
/// The complete core `Persist` effect stays sealed here. In particular this
/// preview exposes no encoded WAL bytes and cannot append or acknowledge the
/// record.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a direct validation Persist result still owns an unappended WAL request"]
struct PreparedDirectValidationSucceededPersist<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    persist_effect: reducer::Effect,
    next_fence_generation: u64,
}
/// Closed classification of one direct successful deterministic validation.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "the direct validation classification owns an exclusive adapter borrow"]
enum DirectValidationSucceededPreparation<'a> {
    /// A reducer persistence/signature fence blocked the exact completion.
    Busy(PreparedDirectValidationSucceededBusy<'a>),
    /// The exact event was ignored without emitting a child effect.
    Inactive(PreparedDirectValidationSucceededInactive<'a>),
    /// Validation applied and emitted no child effect.
    #[allow(dead_code)]
    NoEffect(PreparedDirectValidationSucceededNoEffect<'a>),
    /// Validation applied and emitted one exact application effect.
    Apply(PreparedDirectValidationSucceededApply<'a>),
    /// Validation applied and emitted one exact safety-WAL effect.
    Persist(PreparedDirectValidationSucceededPersist<'a>),
}
// READY_DURABLE_VALIDATE_ADAPTER_PREVIEW_BEGIN
include!("v2_ready_durable_validate_adapter_preview.rs");
// READY_DURABLE_VALIDATE_ADAPTER_PREVIEW_END
/// Closed height retaining safety stores through durable lane/output rollover.
#[must_use = "finalized safety stores must be retained through output handoff"]
pub(crate) struct FinalizedV2Height {
    wal: SafetyWal,
    serviced_candidate_store: ServicedCandidateStore,
    retirement: reducer::WalRetirementAuthorization,
}
impl FinalizedV2Height {
    /// Retire safety stores after rollover; earlier drops preserve restart state.
    pub(in crate::sumeragi) fn retire_after_output_handoff(self) -> Option<String> {
        let serviced_candidate_warning = self.serviced_candidate_store.retire().err();
        let safety_wal_warning = self
            .wal
            .retire(self.retirement)
            .err()
            .map(|e| e.to_string());
        match (safety_wal_warning, serviced_candidate_warning) {
            (None, None) => None,
            (Some(warning), None) | (None, Some(warning)) => Some(warning),
            (Some(wal), Some(candidates)) => {
                Some(format!("{wal}; serviced-candidate cleanup: {candidates}"))
            }
        }
    }
}
/// Canonical consensus input whose structure and cryptography were verified.
///
/// The tuple field is private so networking code cannot manufacture the token
/// without passing [`SumeragiV2Adapter::authenticate`].
#[derive(Clone)]
pub(crate) struct AuthenticatedConsensusMessage(wire::ConsensusMessageV2);
impl AuthenticatedConsensusMessage {
    /// Borrow the cryptographically authenticated consensus payload.
    pub(crate) const fn payload(&self) -> &wire::ConsensusMessageV2Payload {
        &self.0.payload
    }
    /// Borrow the complete authenticated envelope for exact process-local
    /// ownership association. The private constructor remains the only way to
    /// mint this token.
    pub(crate) const fn wire_envelope(&self) -> &wire::ConsensusMessageV2 {
        &self.0
    }
    /// Return whether two authenticated tokens contain the exact same
    /// deterministic wire envelope.
    ///
    /// The runtime uses this only after independently authenticating the
    /// arriving envelope. Coalescing therefore cannot turn equality with an
    /// already-authenticated value into an authentication bypass.
    pub(crate) fn same_wire_envelope(&self, other: &Self) -> bool {
        self.0 == other.0
    }
    /// Return whether this authenticated token contains the supplied exact
    /// deterministic wire envelope.
    ///
    /// Runtime backpressure may use this comparison only to decide whether an
    /// already-owned retransmission is worth authenticating.  Admission still
    /// receives a fresh [`AuthenticatedConsensusMessage`] before it coalesces
    /// the retransmission.
    pub(crate) fn matches_wire_envelope(&self, message: &wire::ConsensusMessageV2) -> bool {
        self.0 == *message
    }
    /// Canonical bytes of the exact authenticated envelope retained by this
    /// process-local token.
    pub(crate) fn canonical_wire_bytes(&self) -> Vec<u8> {
        self.0.encode()
    }
    /// Clone the exact authenticated envelope for fair-ingress unit fixtures.
    #[cfg(test)]
    pub(crate) fn wire_envelope_for_test(&self) -> wire::ConsensusMessageV2 {
        self.0.clone()
    }
    /// Construct an authenticated token for scheduling-boundary unit tests.
    #[cfg(test)]
    pub(crate) fn for_test(message: wire::ConsensusMessageV2) -> Self {
        Self(message)
    }
}
/// Full trusted evidence retained while a body-pipeline completion waits in
/// the adapter's Busy-deferred lane.
///
/// Reducer events intentionally contain only the consensus fields they
/// consume.  Queue ownership is stricter: an asynchronous retry may coalesce
/// only when every manifest and non-forgeable receipt is byte-for-byte equal
/// to the already-owned completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BodyPipelineCompletionEvidence {
    /// Locally assembled proposal bytes crossed both durable boundaries.
    LocalProposalReady {
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    },
    /// Canonical body reconstruction completed with this exact manifest.
    BodyAvailable { manifest: wire::PayloadManifest },
    /// Canonical body storage completed with this exact durable receipt.
    BodyStored {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    },
}
/// Decision-time disposition for one exact local-proposal completion owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecisionLocalProposalDisposition {
    /// The completion is fully bound to the installed Decision and current reducer tag.
    Retain,
    /// The evidence is valid but its stale tag requires ordinary body-pipeline recovery.
    RetireForRecovery,
    /// The queued trusted evidence conflicts with the installed Decision.
    Conflict,
}
/// Classify one queued local-proposal completion against a durable Decision.
///
/// `None` means the completion does not belong to the selected durable body.
/// An exact Decision owner is retainable only when both non-forgeable receipts
/// bind the full manifest and Decision execution commitment, and its tag is the
/// current reducer tag for the certificate's strict same round. The immutable
/// block header may have been constructed in an earlier view, but the manifest,
/// Vote, and CommitQC for this reproposal all name one exact round.
pub(crate) fn classify_decided_local_proposal(
    tag: reducer::EventTag,
    manifest: &wire::PayloadManifest,
    durable_receipt: &DurableBodyReceipt,
    validated_receipt: &ValidatedBodyReceipt,
    decision_tag: reducer::EventTag,
    decision_body_round: wire::ConsensusRound,
    decision_subject: wire::BlockSubject,
    decision_commitment: wire::ExecutionCommitment,
) -> Option<DecisionLocalProposalDisposition> {
    if manifest.round != decision_body_round || manifest.subject != decision_subject {
        return None;
    }
    if durable_receipt.context_id() != decision_body_round.context_id
        || durable_receipt.round() != manifest.round
        || durable_receipt.subject() != decision_subject
        || durable_receipt.manifest_hash() != HashOf::new(manifest)
        || validated_receipt.durable() != durable_receipt
        || validated_receipt.execution_commitment() != decision_commitment
    {
        return Some(DecisionLocalProposalDisposition::Conflict);
    }
    Some(
        if tag == decision_tag
            && tag.height() == decision_body_round.height
            && tag.view() == manifest.round.view
        {
            DecisionLocalProposalDisposition::Retain
        } else {
            DecisionLocalProposalDisposition::RetireForRecovery
        },
    )
}
#[derive(Clone, Debug)]
struct DeferredInput {
    admission_ordinal: u128,
    admission_capability: DeferredAdmissionCapability,
    event: reducer::Event,
    completion_evidence: Option<BodyPipelineCompletionEvidence>,
    retag_authenticated_ingress: bool,
    priority: DeferredPriority,
    protected_progress: bool,
    admission: Option<IngressAdmission>,
    authenticated_wire_identity: Option<Arc<[u8]>>,
    admitted_at: Instant,
    eligible_skips: u64,
}
struct DeferPolicyOutcome {
    outcome: AdapterOutcome,
}
impl PartialEq for DeferredInput {
    fn eq(&self, other: &Self) -> bool {
        self.event == other.event
            && self.completion_evidence == other.completion_evidence
            && self.retag_authenticated_ingress == other.retag_authenticated_ingress
            && self.priority == other.priority
            && self.protected_progress == other.protected_progress
            && self.authenticated_wire_identity == other.authenticated_wire_identity
    }
}
impl Eq for DeferredInput {}
/// Actor-owned source of process-local deferred admission ordinals.
///
/// The source is deliberately shared across height adapters. Replacing an
/// adapter therefore cannot alias a stale deferred capability by restarting
/// the sequence. Values are opaque, never serialized, and have no consensus
/// meaning.
#[derive(Clone, Debug)]
pub(crate) struct DeferredAdmissionOrdinalSource {
    state: Arc<Mutex<DeferredAdmissionOrdinalState>>,
    identity: Arc<()>,
}
#[derive(Debug)]
struct DeferredAdmissionOrdinalState {
    next: u128,
}
impl DeferredAdmissionOrdinalSource {
    /// Construct an actor-global source whose first successful admission uses
    /// `first`.
    ///
    /// Callers must retain and reuse this source across replacement height
    /// adapters. The runtime actor must inject the same source into every
    /// replacement adapter; there is deliberately no process-global fallback.
    pub(crate) fn new(first: u128) -> Self {
        Self {
            state: Arc::new(Mutex::new(DeferredAdmissionOrdinalState { next: first })),
            identity: Arc::new(()),
        }
    }
    fn mint(
        &self,
        origin: DeferredAdmissionOrigin,
    ) -> Result<DeferredAdmissionCapability, AdapterError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AdapterError::DeferredAdmissionOrdinalSourceUnavailable)?;
        // Reserve the next ordinal before returning the current one. `u128::MAX`
        // is never issued, so every successful capability has a distinct
        // representable next value and exhaustion cannot wrap to a stale owner.
        let next = state
            .next
            .checked_add(1)
            .ok_or(AdapterError::DeferredAdmissionOrdinalExhausted)?;
        let ordinal = state.next;
        state.next = next;
        Ok(DeferredAdmissionCapability {
            ordinal,
            origin,
            source_identity: Arc::clone(&self.identity),
            adapter_service_claimed: Arc::new(AtomicBool::new(false)),
            runtime_handoff_claimed: Arc::new(AtomicBool::new(false)),
            runtime_ownership: None,
            #[cfg(test)]
            unbound_fixture: false,
        })
    }
    #[cfg(test)]
    fn next_for_test(&self) -> u128 {
        self.state
            .lock()
            .expect("test deferred ordinal source remains available")
            .next
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredAdmissionOrigin {
    LocalOrCausal,
    DirectAuthenticated,
}
impl DeferredAdmissionOrigin {
    const fn code(self) -> u8 {
        match self {
            Self::LocalOrCausal => 0,
            Self::DirectAuthenticated => 1,
        }
    }
    const fn is_authenticated(self) -> bool {
        matches!(self, Self::DirectAuthenticated)
    }
}
/// Runtime-owned fields frozen when one Busy occurrence crosses the adapter
/// boundary.
///
/// This value remains private to the adapter module. The serialized runtime
/// supplies the already-validated fields exactly once; later scheduler
/// evidence can compare against the resulting opaque seal but cannot rewrite
/// it and recompute a public integrity hash.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DeferredRuntimeOwnershipBinding {
    causal_lifecycle_key: Hash,
    initial_lifecycle_ordinal: u128,
    authenticated_ingress: bool,
    source_physical_ordinal: Option<u64>,
    physical_cut: u128,
}
impl DeferredRuntimeOwnershipBinding {
    fn validate_exact(&self) -> bool {
        self.initial_lifecycle_ordinal != 0
            && self.physical_cut != 0
            && self
                .source_physical_ordinal
                .is_none_or(|source| u128::from(source) < self.physical_cut)
    }
}
#[derive(Clone, Debug)]
struct DeferredAdmissionCapability {
    ordinal: u128,
    origin: DeferredAdmissionOrigin,
    source_identity: Arc<()>,
    adapter_service_claimed: Arc<AtomicBool>,
    runtime_handoff_claimed: Arc<AtomicBool>,
    runtime_ownership: Option<DeferredRuntimeOwnershipBinding>,
    #[cfg(test)]
    unbound_fixture: bool,
}
impl PartialEq for DeferredAdmissionCapability {
    fn eq(&self, other: &Self) -> bool {
        self.ordinal == other.ordinal
            && self.origin == other.origin
            && Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && Arc::ptr_eq(
                &self.adapter_service_claimed,
                &other.adapter_service_claimed,
            )
            && Arc::ptr_eq(
                &self.runtime_handoff_claimed,
                &other.runtime_handoff_claimed,
            )
            && self.runtime_ownership == other.runtime_ownership
            && {
                #[cfg(test)]
                {
                    self.unbound_fixture == other.unbound_fixture
                }
                #[cfg(not(test))]
                {
                    true
                }
            }
    }
}
impl Eq for DeferredAdmissionCapability {}
impl DeferredAdmissionCapability {
    fn pending() -> Self {
        Self {
            ordinal: 0,
            origin: DeferredAdmissionOrigin::LocalOrCausal,
            source_identity: Arc::new(()),
            adapter_service_claimed: Arc::new(AtomicBool::new(false)),
            runtime_handoff_claimed: Arc::new(AtomicBool::new(false)),
            runtime_ownership: None,
            #[cfg(test)]
            unbound_fixture: false,
        }
    }
    #[cfg(test)]
    fn for_test(ordinal: u128) -> Self {
        Self {
            ordinal,
            origin: DeferredAdmissionOrigin::LocalOrCausal,
            source_identity: Arc::new(()),
            adapter_service_claimed: Arc::new(AtomicBool::new(false)),
            runtime_handoff_claimed: Arc::new(AtomicBool::new(false)),
            runtime_ownership: None,
            unbound_fixture: true,
        }
    }
    #[cfg(test)]
    fn for_authenticated_test(ordinal: u128) -> Self {
        let mut capability = Self::for_test(ordinal);
        capability.origin = DeferredAdmissionOrigin::DirectAuthenticated;
        capability
    }
    fn claim_adapter_service_once(&self) -> bool {
        self.adapter_service_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn adapter_service_is_claimed(&self) -> bool {
        self.adapter_service_claimed.load(Ordering::Acquire)
    }
    fn claim_runtime_handoff_once(&self) -> bool {
        self.runtime_handoff_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn runtime_handoff_is_claimed(&self) -> bool {
        self.runtime_handoff_claimed.load(Ordering::Acquire)
    }
    fn runtime_ownership_seal(&self) -> Option<DeferredRuntimeOwnershipSeal> {
        DeferredRuntimeOwnershipSeal::from_capability(self)
    }
}
/// Opaque adapter-issued seal for the runtime owner attached to one Busy
/// occurrence.
///
/// Claim state is deliberately not part of immutable validation: the runtime
/// validates an active owner while both claims are open, then validates the
/// same seal again after deferred service has atomically claimed them. Pointer
/// identity still prevents a same-number capability from being substituted.
#[derive(Clone, Debug)]
pub(crate) struct DeferredRuntimeOwnershipSeal {
    admission_ordinal: u128,
    origin: DeferredAdmissionOrigin,
    source_identity: Arc<()>,
    adapter_service_claimed: Arc<AtomicBool>,
    runtime_handoff_claimed: Arc<AtomicBool>,
    binding: DeferredRuntimeOwnershipBinding,
    projection_hash: Hash,
    #[cfg(test)]
    unbound_fixture: bool,
}
impl PartialEq for DeferredRuntimeOwnershipSeal {
    fn eq(&self, other: &Self) -> bool {
        self.admission_ordinal == other.admission_ordinal
            && self.origin == other.origin
            && Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && Arc::ptr_eq(
                &self.adapter_service_claimed,
                &other.adapter_service_claimed,
            )
            && Arc::ptr_eq(
                &self.runtime_handoff_claimed,
                &other.runtime_handoff_claimed,
            )
            && self.binding == other.binding
            && self.projection_hash == other.projection_hash
            && {
                #[cfg(test)]
                {
                    self.unbound_fixture == other.unbound_fixture
                }
                #[cfg(not(test))]
                {
                    true
                }
            }
    }
}
impl Eq for DeferredRuntimeOwnershipSeal {}
impl DeferredRuntimeOwnershipSeal {
    fn from_capability(capability: &DeferredAdmissionCapability) -> Option<Self> {
        let binding = capability.runtime_ownership.clone()?;
        let mut seal = Self {
            admission_ordinal: capability.ordinal,
            origin: capability.origin,
            source_identity: Arc::clone(&capability.source_identity),
            adapter_service_claimed: Arc::clone(&capability.adapter_service_claimed),
            runtime_handoff_claimed: Arc::clone(&capability.runtime_handoff_claimed),
            binding,
            projection_hash: Hash::new([]),
            #[cfg(test)]
            unbound_fixture: capability.unbound_fixture,
        };
        seal.projection_hash = deferred_runtime_ownership_seal_projection_hash(&seal);
        seal.validate_identity().then_some(seal)
    }
    fn matches_capability(&self, capability: &DeferredAdmissionCapability) -> bool {
        self.validate_identity()
            && self.admission_ordinal == capability.ordinal
            && self.origin == capability.origin
            && Arc::ptr_eq(&self.source_identity, &capability.source_identity)
            && Arc::ptr_eq(
                &self.adapter_service_claimed,
                &capability.adapter_service_claimed,
            )
            && Arc::ptr_eq(
                &self.runtime_handoff_claimed,
                &capability.runtime_handoff_claimed,
            )
            && capability.runtime_ownership.as_ref() == Some(&self.binding)
            && {
                #[cfg(test)]
                {
                    self.unbound_fixture == capability.unbound_fixture
                }
                #[cfg(not(test))]
                {
                    true
                }
            }
    }
    /// Validate immutable capability identity independently of mutable claim
    /// state.
    pub(crate) fn validate_identity(&self) -> bool {
        self.binding.validate_exact()
            && self.origin.is_authenticated() == self.binding.authenticated_ingress
            && self.projection_hash == deferred_runtime_ownership_seal_projection_hash(self)
    }
    /// Whether the exact Busy occurrence remains retained and unclaimed.
    pub(crate) fn still_retained(&self) -> bool {
        self.validate_identity()
            && !self.adapter_service_claimed.load(Ordering::Acquire)
            && !self.runtime_handoff_claimed.load(Ordering::Acquire)
    }
    /// Whether this seal came from the runtime actor's exact deferred ordinal
    /// source rather than a same-number foreign capability.
    pub(crate) fn belongs_to(&self, source: &DeferredAdmissionOrdinalSource) -> bool {
        let exact = Arc::ptr_eq(&self.source_identity, &source.identity);
        #[cfg(test)]
        {
            exact || self.unbound_fixture
        }
        #[cfg(not(test))]
        {
            exact
        }
    }
    /// Match the immutable runtime fields carried by a deferred wrapper.
    pub(crate) fn matches_runtime_owner(
        &self,
        causal_lifecycle_key: &Hash,
        lifecycle_ordinal: u128,
        authenticated_ingress: bool,
        source_physical_ordinal: Option<u64>,
        physical_cut: u128,
    ) -> bool {
        self.validate_identity()
            && self.binding.causal_lifecycle_key == *causal_lifecycle_key
            && self.binding.authenticated_ingress == authenticated_ingress
            && self.binding.source_physical_ordinal == source_physical_ordinal
            && self.binding.physical_cut == physical_cut
            && if authenticated_ingress {
                lifecycle_ordinal <= self.binding.initial_lifecycle_ordinal
            } else {
                lifecycle_ordinal == self.binding.initial_lifecycle_ordinal
            }
    }
    /// Actor-global adapter ordinal sealed into this exact capability.
    pub(crate) const fn admission_ordinal(&self) -> u128 {
        self.admission_ordinal
    }
    /// Logical owner at the instant the adapter admitted this Busy occurrence.
    pub(crate) const fn initial_lifecycle_ordinal(&self) -> u128 {
        self.binding.initial_lifecycle_ordinal
    }
    /// Process-local integrity projection used by enclosing scheduler evidence.
    pub(crate) const fn projection_hash(&self) -> &Hash {
        &self.projection_hash
    }
    /// Construct a capability-consistent seal for scheduler-shell tests which
    /// do not instantiate the production adapter.
    #[cfg(test)]
    pub(crate) fn for_test(
        admission_ordinal: u128,
        causal_lifecycle_key: Hash,
        initial_lifecycle_ordinal: u128,
        authenticated_ingress: bool,
        source_physical_ordinal: Option<u64>,
        physical_cut: u128,
    ) -> Self {
        let mut capability = if authenticated_ingress {
            DeferredAdmissionCapability::for_authenticated_test(admission_ordinal)
        } else {
            DeferredAdmissionCapability::for_test(admission_ordinal)
        };
        capability.runtime_ownership = Some(DeferredRuntimeOwnershipBinding {
            causal_lifecycle_key,
            initial_lifecycle_ordinal,
            authenticated_ingress,
            source_physical_ordinal,
            physical_cut,
        });
        capability
            .runtime_ownership_seal()
            .expect("test runtime ownership binding is exact")
    }
    /// Construct a seal from a real, independently owned ordinal source so
    /// runtime tests can distinguish same-number foreign capabilities from the
    /// deliberate unbound fake-driver fixture above.
    #[cfg(test)]
    pub(crate) fn for_source_test(
        source: &DeferredAdmissionOrdinalSource,
        causal_lifecycle_key: Hash,
        initial_lifecycle_ordinal: u128,
        authenticated_ingress: bool,
        source_physical_ordinal: Option<u64>,
        physical_cut: u128,
    ) -> Self {
        let origin = if authenticated_ingress {
            DeferredAdmissionOrigin::DirectAuthenticated
        } else {
            DeferredAdmissionOrigin::LocalOrCausal
        };
        let mut capability = source
            .mint(origin)
            .expect("test ordinal source remains exact");
        capability.runtime_ownership = Some(DeferredRuntimeOwnershipBinding {
            causal_lifecycle_key,
            initial_lifecycle_ordinal,
            authenticated_ingress,
            source_physical_ordinal,
            physical_cut,
        });
        capability
            .runtime_ownership_seal()
            .expect("foreign test runtime ownership binding is exact")
    }
}
fn deferred_runtime_ownership_seal_projection_hash(seal: &DeferredRuntimeOwnershipSeal) -> Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:deferred-runtime-owner:v1");
    projection.extend_from_slice(&seal.admission_ordinal.to_le_bytes());
    projection.push(seal.origin.code());
    projection.extend_from_slice(&(Arc::as_ptr(&seal.source_identity) as usize).to_le_bytes());
    projection
        .extend_from_slice(&(Arc::as_ptr(&seal.adapter_service_claimed) as usize).to_le_bytes());
    projection
        .extend_from_slice(&(Arc::as_ptr(&seal.runtime_handoff_claimed) as usize).to_le_bytes());
    projection.extend_from_slice(seal.binding.causal_lifecycle_key.as_ref());
    projection.extend_from_slice(&seal.binding.initial_lifecycle_ordinal.to_le_bytes());
    projection.push(u8::from(seal.binding.authenticated_ingress));
    match seal.binding.source_physical_ordinal {
        None => projection.push(0),
        Some(source) => {
            projection.push(1);
            projection.extend_from_slice(&source.to_le_bytes());
        }
    }
    projection.extend_from_slice(&seal.binding.physical_cut.to_le_bytes());
    #[cfg(test)]
    projection.push(u8::from(seal.unbound_fixture));
    Hash::new(projection)
}
/// Immutable adapter-issued identity of one still-retained Busy occurrence.
///
/// Unlike a service token, this snapshot does not claim or remove the owner.
/// Its private admission capability binds the ordinal and direct-network vs
/// local/causal provenance so the runtime cannot reclassify an authenticated
/// fence target after dropping its fair-ingress carrier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeferredOccurrenceOwnershipEvidence {
    admission_ordinal: u128,
    authenticated_ingress: bool,
    source_identity: Arc<()>,
    admission_capability: DeferredAdmissionCapability,
    projection_hash: Hash,
}
impl DeferredOccurrenceOwnershipEvidence {
    fn from_input(input: &DeferredInput, source: &DeferredAdmissionOrdinalSource) -> Option<Self> {
        let source_is_exact = Arc::ptr_eq(
            &input.admission_capability.source_identity,
            &source.identity,
        ) || {
            #[cfg(test)]
            {
                input.admission_capability.unbound_fixture
            }
            #[cfg(not(test))]
            {
                false
            }
        };
        let authenticated_ingress = input.retag_authenticated_ingress;
        if input.admission_capability.ordinal != input.admission_ordinal
            || input.admission_capability.origin.is_authenticated() != authenticated_ingress
            || authenticated_ingress != input.authenticated_wire_identity.is_some()
            || !source_is_exact
            || input.admission_capability.adapter_service_is_claimed()
            || input.admission_capability.runtime_handoff_is_claimed()
        {
            return None;
        }
        let mut evidence = Self {
            admission_ordinal: input.admission_ordinal,
            authenticated_ingress,
            source_identity: Arc::clone(&input.admission_capability.source_identity),
            admission_capability: input.admission_capability.clone(),
            projection_hash: Hash::new([]),
        };
        evidence.projection_hash = deferred_occurrence_ownership_projection_hash(&evidence);
        evidence.validate_exact().then_some(evidence)
    }
    /// Mint one exact local/causal Busy capability and its matching runtime
    /// seal for serialized-runtime boundary tests.
    #[cfg(test)]
    pub(crate) fn local_for_runtime_test(
        source: &DeferredAdmissionOrdinalSource,
        causal_lifecycle_key: Hash,
        initial_lifecycle_ordinal: u128,
        physical_cut: u128,
    ) -> (Self, DeferredRuntimeOwnershipSeal) {
        let mut admission_capability = source
            .mint(DeferredAdmissionOrigin::LocalOrCausal)
            .expect("test deferred ordinal source remains exact");
        admission_capability.runtime_ownership = Some(DeferredRuntimeOwnershipBinding {
            causal_lifecycle_key,
            initial_lifecycle_ordinal,
            authenticated_ingress: false,
            source_physical_ordinal: None,
            physical_cut,
        });
        let runtime_seal = admission_capability
            .runtime_ownership_seal()
            .expect("test deferred capability owns one exact runtime seal");
        let mut evidence = Self {
            admission_ordinal: admission_capability.ordinal,
            authenticated_ingress: false,
            source_identity: Arc::clone(&admission_capability.source_identity),
            admission_capability,
            projection_hash: Hash::new([]),
        };
        evidence.projection_hash = deferred_occurrence_ownership_projection_hash(&evidence);
        assert!(evidence.validate_exact());
        (evidence, runtime_seal)
    }
    /// Validate the private actor capability and immutable provenance bit.
    pub(crate) fn validate_exact(&self) -> bool {
        self.admission_capability.ordinal == self.admission_ordinal
            && self.admission_capability.origin.is_authenticated() == self.authenticated_ingress
            && self
                .admission_capability
                .runtime_ownership
                .as_ref()
                .is_none_or(DeferredRuntimeOwnershipBinding::validate_exact)
            && Arc::ptr_eq(
                &self.source_identity,
                &self.admission_capability.source_identity,
            )
            && self.projection_hash == deferred_occurrence_ownership_projection_hash(self)
    }
    /// Whether the underlying occurrence remains retained and neither service
    /// seam has claimed it yet.
    pub(crate) fn still_retained(&self) -> bool {
        self.validate_exact()
            && !self.admission_capability.adapter_service_is_claimed()
            && !self.admission_capability.runtime_handoff_is_claimed()
    }
    /// Adapter-global Busy admission ordinal owned by this snapshot.
    pub(crate) const fn admission_ordinal(&self) -> u128 {
        self.admission_ordinal
    }
    /// Whether the occurrence itself directly carried authenticated ingress.
    pub(crate) const fn is_authenticated_ingress(&self) -> bool {
        self.authenticated_ingress
    }
    /// Process-local integrity projection consumed by runtime evidence.
    pub(crate) const fn projection_hash(&self) -> &Hash {
        &self.projection_hash
    }
    /// Bind a live occurrence snapshot to a previously retained runtime seal.
    pub(crate) fn matches_runtime_ownership_seal(
        &self,
        seal: &DeferredRuntimeOwnershipSeal,
    ) -> bool {
        self.validate_exact() && seal.matches_capability(&self.admission_capability)
    }
    /// Live-map form of [`Self::matches_runtime_ownership_seal`].
    pub(crate) fn matches_retained_runtime_ownership_seal(
        &self,
        seal: &DeferredRuntimeOwnershipSeal,
    ) -> bool {
        self.still_retained() && seal.still_retained() && self.matches_runtime_ownership_seal(seal)
    }
    /// Whether this occurrence was minted by the supplied actor-owned source.
    pub(crate) fn belongs_to(&self, source: &DeferredAdmissionOrdinalSource) -> bool {
        self.validate_exact()
            && Arc::ptr_eq(&self.admission_capability.source_identity, &source.identity)
    }
}
fn deferred_occurrence_ownership_projection_hash(
    evidence: &DeferredOccurrenceOwnershipEvidence,
) -> Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:deferred-occurrence-owner:v1");
    projection.extend_from_slice(&evidence.admission_ordinal.to_le_bytes());
    projection.push(u8::from(evidence.authenticated_ingress));
    projection.push(evidence.admission_capability.origin.code());
    projection.extend_from_slice(&(Arc::as_ptr(&evidence.source_identity) as usize).to_le_bytes());
    projection.extend_from_slice(
        &(Arc::as_ptr(&evidence.admission_capability.adapter_service_claimed) as usize)
            .to_le_bytes(),
    );
    projection.extend_from_slice(
        &(Arc::as_ptr(&evidence.admission_capability.runtime_handoff_claimed) as usize)
            .to_le_bytes(),
    );
    match evidence.admission_capability.runtime_ownership_seal() {
        None => projection.push(0),
        Some(seal) => {
            projection.push(1);
            projection.extend_from_slice(seal.projection_hash().as_ref());
        }
    }
    Hash::new(projection)
}
/// Three bounded classes in the adapter-owned Busy-deferred lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredPriority {
    /// Trusted completions and local timer events which untrusted traffic
    /// must not displace.
    Completion,
    /// Validated QCs/TCs, TimeoutVote messages, and exact locked-round Commit
    /// reconstruction.
    Progress,
    /// Proposals and individual control votes.
    Normal,
}
impl DeferredPriority {
    const fn code(self) -> u8 {
        match self {
            Self::Completion => 1,
            Self::Progress => 2,
            Self::Normal => 3,
        }
    }
}
/// Typed reducer-event discriminant retained by a deferred service token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredEventKind {
    /// Complete safety-WAL replay.
    ResumeAfterReplay,
    /// Locally built body is durable and valid.
    LocalProposalReady,
    /// Authenticated proposal.
    ProposalReceived,
    /// Authenticated Prepare or Commit vote.
    VoteReceived,
    /// Verified PrepareQC or CommitQC.
    QuorumCertificateReceived,
    /// Authenticated timeout vote.
    TimeoutVoteReceived,
    /// Verified timeout certificate.
    TimeoutCertificateReceived,
    /// Absolute round timeout.
    TimeoutElapsed,
    /// Periodic retransmission timeout.
    RetransmitElapsed,
    /// Reconstructed body completion.
    BodyAvailable,
    /// Durable body-store completion.
    BodyStored,
    /// Deterministic validation completion.
    ValidationCompleted,
    /// Safety-WAL persistence acknowledgement.
    Persisted,
    /// Safety-WAL persistence failure.
    PersistenceFailed,
    /// Local signing completion.
    Signed,
    /// Local application completion.
    ApplicationCompleted,
}
impl DeferredEventKind {
    const fn code(self) -> u8 {
        match self {
            Self::LocalProposalReady => 0,
            Self::ProposalReceived => 1,
            Self::VoteReceived => 2,
            Self::QuorumCertificateReceived => 3,
            Self::TimeoutVoteReceived => 4,
            Self::TimeoutCertificateReceived => 5,
            Self::TimeoutElapsed => 6,
            Self::RetransmitElapsed => 7,
            Self::BodyAvailable => 8,
            Self::BodyStored => 9,
            Self::ValidationCompleted => 10,
            Self::Persisted => 11,
            Self::PersistenceFailed => reducer::EVENT_PERSISTENCE_FAILED,
            Self::Signed => 13,
            Self::ApplicationCompleted => 14,
            Self::ResumeAfterReplay => 15,
        }
    }
}
/// Exact local retagging relation for one selected deferred occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredRetagRelation {
    /// The asynchronous completion or local event retained its original tag.
    Unchanged,
    /// An authenticated network event was rebound to the current reducer tag.
    AuthenticatedIngress {
        /// Tag retained while the authenticated event waited.
        from: reducer::EventTag,
        /// Current reducer tag used for this retry.
        to: reducer::EventTag,
    },
}
/// Per-class deferred queue lengths around one exact service selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeferredQueueLengths {
    /// Completion-lane owners.
    pub(crate) completion: u64,
    /// Progress-lane owners.
    pub(crate) progress: u64,
    /// Normal-lane owners.
    pub(crate) normal: u64,
}
impl DeferredQueueLengths {
    fn total(self) -> u64 {
        self.checked_total()
            .expect("bounded deferred queue totals fit u64")
    }
    fn checked_total(self) -> Option<u64> {
        self.completion
            .checked_add(self.progress)?
            .checked_add(self.normal)
    }
    const fn for_priority(self, priority: DeferredPriority) -> u64 {
        match priority {
            DeferredPriority::Completion => self.completion,
            DeferredPriority::Progress => self.progress,
            DeferredPriority::Normal => self.normal,
        }
    }
}
/// Adapter-private authority for one exact Busy-deferred queue removal.
///
/// Public length/cursor projections are useful for rank checking, but a
/// caller can otherwise alter those fields and recompute their ordinary hash.
/// This capability is minted only beside the actual queue mutation, binds the
/// runtime-selected ordinal set and physical lane position, and may cross the
/// adapter service seam exactly once.
#[derive(Clone, Debug)]
struct DeferredQueueSelectionSeal {
    source_identity: Arc<()>,
    adapter_selection_claimed: Arc<AtomicBool>,
    eligible_admission_ordinals: Arc<[u128]>,
    queue_lengths_before: DeferredQueueLengths,
    eligible_queue_lengths_before: DeferredQueueLengths,
    queue_lengths_after: DeferredQueueLengths,
    service_cursor_before: DeferredPriority,
    service_cursor_after: DeferredPriority,
    selected_priority: DeferredPriority,
    selected_position: u64,
    selected_admission_ordinal: u128,
    selected_eligible_skips: u64,
    selected_evidence_hash: Hash,
    projection_hash: Hash,
}
impl PartialEq for DeferredQueueSelectionSeal {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && Arc::ptr_eq(
                &self.adapter_selection_claimed,
                &other.adapter_selection_claimed,
            )
            && self.eligible_admission_ordinals == other.eligible_admission_ordinals
            && self.queue_lengths_before == other.queue_lengths_before
            && self.eligible_queue_lengths_before == other.eligible_queue_lengths_before
            && self.queue_lengths_after == other.queue_lengths_after
            && self.service_cursor_before == other.service_cursor_before
            && self.service_cursor_after == other.service_cursor_after
            && self.selected_priority == other.selected_priority
            && self.selected_position == other.selected_position
            && self.selected_admission_ordinal == other.selected_admission_ordinal
            && self.selected_eligible_skips == other.selected_eligible_skips
            && self.selected_evidence_hash == other.selected_evidence_hash
            && self.projection_hash == other.projection_hash
    }
}
impl Eq for DeferredQueueSelectionSeal {}
fn deferred_queue_selection_projection_hash(seal: &DeferredQueueSelectionSeal) -> Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:deferred-queue-selection:v1");
    projection.extend_from_slice(&(Arc::as_ptr(&seal.source_identity) as usize).to_le_bytes());
    projection
        .extend_from_slice(&(Arc::as_ptr(&seal.adapter_selection_claimed) as usize).to_le_bytes());
    append_deferred_projection_u64(
        &mut projection,
        u64::try_from(seal.eligible_admission_ordinals.len())
            .expect("bounded eligible ordinal set fits u64"),
    );
    for ordinal in seal.eligible_admission_ordinals.iter() {
        append_deferred_projection_field(&mut projection, &ordinal.to_le_bytes());
    }
    for lengths in [
        seal.queue_lengths_before,
        seal.eligible_queue_lengths_before,
        seal.queue_lengths_after,
    ] {
        append_deferred_projection_u64(&mut projection, lengths.completion);
        append_deferred_projection_u64(&mut projection, lengths.progress);
        append_deferred_projection_u64(&mut projection, lengths.normal);
    }
    projection.push(seal.service_cursor_before.code());
    projection.push(seal.service_cursor_after.code());
    projection.push(seal.selected_priority.code());
    append_deferred_projection_u64(&mut projection, seal.selected_position);
    append_deferred_projection_field(
        &mut projection,
        &seal.selected_admission_ordinal.to_le_bytes(),
    );
    append_deferred_projection_u64(&mut projection, seal.selected_eligible_skips);
    append_deferred_projection_field(&mut projection, seal.selected_evidence_hash.as_ref());
    Hash::new(projection)
}
impl DeferredQueueSelectionSeal {
    #[allow(clippy::too_many_arguments)]
    fn mint(
        source: &DeferredAdmissionOrdinalSource,
        eligible: &BTreeSet<u128>,
        queue_lengths_before: DeferredQueueLengths,
        eligible_queue_lengths_before: DeferredQueueLengths,
        queue_lengths_after: DeferredQueueLengths,
        service_cursor_before: DeferredPriority,
        service_cursor_after: DeferredPriority,
        selected_priority: DeferredPriority,
        selected_position: u64,
        selected_admission_ordinal: u128,
        selected_eligible_skips: u64,
        selected_evidence_hash: Hash,
    ) -> Option<Self> {
        let mut seal = Self {
            source_identity: Arc::clone(&source.identity),
            adapter_selection_claimed: Arc::new(AtomicBool::new(false)),
            eligible_admission_ordinals: eligible.iter().copied().collect::<Vec<_>>().into(),
            queue_lengths_before,
            eligible_queue_lengths_before,
            queue_lengths_after,
            service_cursor_before,
            service_cursor_after,
            selected_priority,
            selected_position,
            selected_admission_ordinal,
            selected_eligible_skips,
            selected_evidence_hash,
            projection_hash: Hash::new([]),
        };
        seal.projection_hash = deferred_queue_selection_projection_hash(&seal);
        seal.validate_identity().then_some(seal)
    }
    fn validate_identity(&self) -> bool {
        let eligible_count = self
            .eligible_queue_lengths_before
            .checked_total()
            .and_then(|count| usize::try_from(count).ok());
        let lane_before = self
            .queue_lengths_before
            .for_priority(self.selected_priority);
        let lane_after = self
            .queue_lengths_after
            .for_priority(self.selected_priority);
        self.projection_hash == deferred_queue_selection_projection_hash(self)
            && !self.eligible_admission_ordinals.is_empty()
            && self
                .eligible_admission_ordinals
                .windows(2)
                .all(|window| window[0] < window[1])
            && self
                .eligible_admission_ordinals
                .binary_search(&self.selected_admission_ordinal)
                .is_ok()
            && eligible_count == Some(self.eligible_admission_ordinals.len())
            && self.selected_position < lane_before
            && lane_after.checked_add(1) == Some(lane_before)
            && [
                DeferredPriority::Completion,
                DeferredPriority::Progress,
                DeferredPriority::Normal,
            ]
            .into_iter()
            .filter(|priority| *priority != self.selected_priority)
            .all(|priority| {
                self.queue_lengths_before.for_priority(priority)
                    == self.queue_lengths_after.for_priority(priority)
            })
    }
    fn matches_evidence(&self, evidence: &DeferredServiceEvidence) -> bool {
        let source_is_exact = Arc::ptr_eq(
            &self.source_identity,
            &evidence.admission_capability.source_identity,
        ) || {
            #[cfg(test)]
            {
                evidence.admission_capability.unbound_fixture
            }
            #[cfg(not(test))]
            {
                false
            }
        };
        self.validate_identity()
            && source_is_exact
            && self.queue_lengths_before == evidence.queue_lengths_before
            && self.eligible_queue_lengths_before == evidence.eligible_queue_lengths_before
            && self.queue_lengths_after == evidence.queue_lengths_after
            && self.service_cursor_before == evidence.service_cursor_before
            && self.service_cursor_after == evidence.service_cursor_after
            && self.selected_priority == evidence.priority
            && self.selected_admission_ordinal == evidence.admission_ordinal
            && self.selected_eligible_skips == evidence.eligible_skips_before
            && self.selected_evidence_hash == evidence.projection_hash
            && self.selected_evidence_hash == deferred_service_projection_hash(evidence)
    }
    fn matches_eligible_admission_ordinals(&self, eligible: &BTreeSet<u128>) -> bool {
        self.validate_identity()
            && self.eligible_admission_ordinals.len() == eligible.len()
            && self
                .eligible_admission_ordinals
                .iter()
                .copied()
                .eq(eligible.iter().copied())
    }
    fn claim_adapter_selection_once(&self) -> bool {
        self.validate_identity()
            && self
                .adapter_selection_claimed
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }
    fn adapter_selection_is_claimed(&self) -> bool {
        self.adapter_selection_claimed.load(Ordering::Acquire)
    }
}
/// Exact process-local owner discharged by one Busy-deferred service turn.
///
/// The full typed events remain process-local and make semantic identity
/// lossless. `projection_hash` is a deterministic integrity projection over
/// every externally inspected field and every fixed event/evidence field; it
/// is not a wire capability and must never be serialized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeferredServiceEvidence {
    /// Actor-global ordinal minted only when this owner first entered a queue.
    pub(crate) admission_ordinal: u128,
    /// Exact selected queue class.
    pub(crate) priority: DeferredPriority,
    /// Typed reducer-event discriminant.
    pub(crate) event_kind: DeferredEventKind,
    /// Original tag retained at admission.
    pub(crate) original_tag: reducer::EventTag,
    /// Effective tag dispatched to the reducer.
    pub(crate) effective_tag: reducer::EventTag,
    /// Exact authenticated-retag relation.
    pub(crate) retag: DeferredRetagRelation,
    /// Whether the selected owner is protected locked-round progress.
    pub(crate) protected_progress: bool,
    /// Selected owner's accumulated eligible service debt.
    pub(crate) eligible_skips_before: u64,
    /// Service retires the selected owner's debt.
    pub(crate) eligible_skips_after: u64,
    /// All class lengths before selection.
    pub(crate) queue_lengths_before: DeferredQueueLengths,
    /// Per-class owners admitted by the runtime's frozen target-relative set
    /// before selection. Post-cut or nonminimal owners remain represented in
    /// `queue_lengths_before`, but cannot influence class rotation.
    pub(crate) eligible_queue_lengths_before: DeferredQueueLengths,
    /// All class lengths after selection.
    pub(crate) queue_lengths_after: DeferredQueueLengths,
    /// Redundant exact total before selection.
    pub(crate) total_len_before: u64,
    /// Redundant exact total after selection.
    pub(crate) total_len_after: u64,
    /// Three-class service cursor before selection.
    pub(crate) service_cursor_before: DeferredPriority,
    /// Three-class service cursor after selection.
    pub(crate) service_cursor_after: DeferredPriority,
    /// Hash over the complete immutable process-local projection.
    pub(crate) projection_hash: Hash,
    original_event: reducer::Event,
    effective_event: reducer::Event,
    completion_evidence: Option<BodyPipelineCompletionEvidence>,
    original_admission: Option<IngressAdmission>,
    effective_admission: Option<IngressAdmission>,
    authenticated_wire_identity: Option<Arc<[u8]>>,
    admission_capability: DeferredAdmissionCapability,
    selection_seal: Option<DeferredQueueSelectionSeal>,
}
impl DeferredServiceEvidence {
    /// Construct one internally consistent Completion-lane token for scheduler
    /// shell tests which use a fake driver rather than a real adapter.
    #[cfg(test)]
    pub(crate) fn completion_for_test(
        source: &DeferredAdmissionOrdinalSource,
        tag: reducer::EventTag,
        completion_len_before: u64,
        service_cursor_before: DeferredPriority,
    ) -> Self {
        assert!(completion_len_before != 0);
        let admission_capability = source
            .mint(DeferredAdmissionOrigin::LocalOrCausal)
            .expect("test deferred ordinal remains available");
        let admission_ordinal = admission_capability.ordinal;
        let event = reducer::Event::TimeoutElapsed { tag };
        let queue_lengths_before = DeferredQueueLengths {
            completion: completion_len_before,
            progress: 0,
            normal: 0,
        };
        let queue_lengths_after = DeferredQueueLengths {
            completion: completion_len_before - 1,
            progress: 0,
            normal: 0,
        };
        let eligible_queue_lengths_before = DeferredQueueLengths {
            completion: 1,
            progress: 0,
            normal: 0,
        };
        let mut cursor = service_cursor_before;
        for _ in 0..3 {
            let selected = cursor;
            cursor = cursor.next();
            if selected == DeferredPriority::Completion {
                break;
            }
        }
        let mut evidence = Self {
            admission_ordinal,
            priority: DeferredPriority::Completion,
            event_kind: DeferredEventKind::TimeoutElapsed,
            original_tag: tag,
            effective_tag: tag,
            retag: DeferredRetagRelation::Unchanged,
            protected_progress: false,
            eligible_skips_before: 0,
            eligible_skips_after: 0,
            queue_lengths_before,
            eligible_queue_lengths_before,
            queue_lengths_after,
            total_len_before: queue_lengths_before.total(),
            total_len_after: queue_lengths_after.total(),
            service_cursor_before,
            service_cursor_after: cursor,
            projection_hash: Hash::new([]),
            original_event: event.clone(),
            effective_event: event,
            completion_evidence: None,
            original_admission: None,
            effective_admission: None,
            authenticated_wire_identity: None,
            admission_capability,
            selection_seal: None,
        };
        evidence.projection_hash = deferred_service_projection_hash(&evidence);
        evidence.selection_seal = DeferredQueueSelectionSeal::mint(
            source,
            &BTreeSet::from([admission_ordinal]),
            queue_lengths_before,
            eligible_queue_lengths_before,
            queue_lengths_after,
            service_cursor_before,
            cursor,
            DeferredPriority::Completion,
            0,
            admission_ordinal,
            0,
            evidence.projection_hash,
        );
        assert!(evidence.validate_exact());
        evidence
    }
    /// Return whether every redundant field and rank transition still matches
    /// the exact selected occurrence.
    pub(crate) fn validate_exact(&self) -> bool {
        let admission_is_protected = |admission: Option<IngressAdmission>| {
            admission.is_some_and(|admission| {
                admission.locked_commit_progress || admission.locked_reproposal_prepare_progress
            })
        };
        let protected_admission_matches_event =
            |admission: Option<IngressAdmission>, event: &reducer::Event| {
                let Some(admission) = admission else {
                    return false;
                };
                match event {
                    reducer::Event::VoteReceived { vote, .. } => match vote.vote().phase() {
                        reducer::Phase::Prepare => {
                            !admission.locked_commit_progress
                                && admission.locked_reproposal_prepare_progress
                        }
                        reducer::Phase::Commit => {
                            admission.locked_commit_progress
                                && !admission.locked_reproposal_prepare_progress
                        }
                    },
                    _ => false,
                }
            };
        let protected_progress_is_exact = self.protected_progress
            == admission_is_protected(self.original_admission)
            && self.protected_progress == admission_is_protected(self.effective_admission)
            && (!self.protected_progress
                || (self.priority == DeferredPriority::Progress
                    && protected_admission_matches_event(
                        self.original_admission,
                        &self.original_event,
                    )
                    && protected_admission_matches_event(
                        self.effective_admission,
                        &self.effective_event,
                    )));
        if self.admission_capability.ordinal != self.admission_ordinal
            || self.admission_capability.origin.is_authenticated()
                != self.is_authenticated_ingress()
            || self
                .admission_capability
                .runtime_ownership
                .as_ref()
                .is_some_and(|binding| !binding.validate_exact())
            || self.event_kind != deferred_event_kind(&self.original_event)
            || self.event_kind != deferred_event_kind(&self.effective_event)
            || self.original_tag != deferred_event_tag(&self.original_event)
            || self.effective_tag != deferred_event_tag(&self.effective_event)
            || !protected_progress_is_exact
            || self.eligible_skips_after != 0
            || Some(self.total_len_before) != self.queue_lengths_before.checked_total()
            || Some(self.total_len_after) != self.queue_lengths_after.checked_total()
            || self.total_len_after.checked_add(1) != Some(self.total_len_before)
            || self.eligible_queue_lengths_before.completion > self.queue_lengths_before.completion
            || self.eligible_queue_lengths_before.progress > self.queue_lengths_before.progress
            || self.eligible_queue_lengths_before.normal > self.queue_lengths_before.normal
            || self
                .queue_lengths_before
                .for_priority(self.priority)
                .checked_sub(1)
                != Some(self.queue_lengths_after.for_priority(self.priority))
            || !self
                .selection_seal
                .as_ref()
                .is_some_and(|seal| seal.matches_evidence(self))
        {
            return false;
        }
        if self.is_authenticated_ingress() != self.authenticated_wire_identity.is_some() {
            return false;
        }
        if let Some(identity) = &self.authenticated_wire_identity {
            let mut cursor = identity.as_ref();
            let Ok(message) = wire::ConsensusMessageV2::decode(&mut cursor) else {
                return false;
            };
            if !cursor.is_empty()
                || !matches!(
                    (&message.payload, self.event_kind),
                    (
                        wire::ConsensusMessageV2Payload::Proposal(_),
                        DeferredEventKind::ProposalReceived
                    ) | (
                        wire::ConsensusMessageV2Payload::Vote(_),
                        DeferredEventKind::VoteReceived
                    ) | (
                        wire::ConsensusMessageV2Payload::QuorumCertificate(_),
                        DeferredEventKind::QuorumCertificateReceived
                    ) | (
                        wire::ConsensusMessageV2Payload::TimeoutVote(_),
                        DeferredEventKind::TimeoutVoteReceived
                    ) | (
                        wire::ConsensusMessageV2Payload::TimeoutCertificate(_),
                        DeferredEventKind::TimeoutCertificateReceived
                    )
                )
            {
                return false;
            }
        }
        for priority in [
            DeferredPriority::Completion,
            DeferredPriority::Progress,
            DeferredPriority::Normal,
        ] {
            if priority != self.priority
                && self.queue_lengths_before.for_priority(priority)
                    != self.queue_lengths_after.for_priority(priority)
            {
                return false;
            }
        }
        let mut cursor = self.service_cursor_before;
        let mut expected_selection = None;
        let mut expected_after = cursor;
        for _ in 0..3 {
            let candidate = cursor;
            cursor = cursor.next();
            if self.eligible_queue_lengths_before.for_priority(candidate) != 0 {
                expected_selection = Some(candidate);
                expected_after = cursor;
                break;
            }
        }
        if expected_selection != Some(self.priority) || expected_after != self.service_cursor_after
        {
            return false;
        }
        let retag_is_exact = match self.retag {
            DeferredRetagRelation::Unchanged => {
                self.original_tag == self.effective_tag
                    && self.original_event == self.effective_event
                    && self.original_admission == self.effective_admission
            }
            DeferredRetagRelation::AuthenticatedIngress { from, to } => {
                from == self.original_tag
                    && to == self.effective_tag
                    && self.original_event.clone().retag_authenticated_ingress(to)
                        == self.effective_event
                    && self.original_admission.map(|mut admission| {
                        admission.consumer_tag = to;
                        admission
                    }) == self.effective_admission
            }
        };
        retag_is_exact && self.projection_hash == deferred_service_projection_hash(self)
    }
    /// Return whether this token owns the supplied exact reducer event.
    pub(crate) fn matches_effective_event(&self, event: &reducer::Event) -> bool {
        self.validate_exact() && self.effective_event == *event
    }
    /// Return whether the adapter claimed this owner before reducer dispatch.
    pub(crate) fn adapter_service_is_claimed(&self) -> bool {
        self.admission_capability.adapter_service_is_claimed()
            && self
                .selection_seal
                .as_ref()
                .is_some_and(DeferredQueueSelectionSeal::adapter_selection_is_claimed)
    }
    /// Match the complete target-relative ordinal set supplied by the
    /// serialized runtime before accepting this adapter selection.
    pub(crate) fn matches_eligible_admission_ordinals(&self, eligible: &BTreeSet<u128>) -> bool {
        self.validate_exact()
            && self
                .selection_seal
                .as_ref()
                .is_some_and(|seal| seal.matches_eligible_admission_ordinals(eligible))
    }
    fn claim_adapter_service_once(&self) -> bool {
        self.validate_exact()
            && self
                .selection_seal
                .as_ref()
                .is_some_and(DeferredQueueSelectionSeal::claim_adapter_selection_once)
            && self.admission_capability.claim_adapter_service_once()
    }
    /// Atomically consume the adapter-to-runtime handoff once. Cloned or
    /// replayed tokens retain the same process-local capability and fail after
    /// the first successful claim.
    pub(crate) fn claim_runtime_handoff_once(&self) -> bool {
        self.validate_exact()
            && self.adapter_service_is_claimed()
            && self.admission_capability.claim_runtime_handoff_once()
    }
    /// Return whether both production seams consumed this exact occurrence.
    pub(crate) fn service_handoff_is_complete(&self) -> bool {
        self.adapter_service_is_claimed() && self.admission_capability.runtime_handoff_is_claimed()
    }
    /// Whether this deferred occurrence originated at authenticated network
    /// ingress and therefore requires the runtime's matching fair-ingress
    /// carrier until service completes.
    pub(crate) const fn is_authenticated_ingress(&self) -> bool {
        matches!(
            self.retag,
            DeferredRetagRelation::AuthenticatedIngress { .. }
        )
    }
    /// Whether this token retains the exact canonical authenticated envelope
    /// carried by the serialized runtime owner.
    pub(crate) fn matches_authenticated_runtime_bytes(&self, canonical_bytes: &[u8]) -> bool {
        self.validate_exact()
            && self
                .authenticated_wire_identity
                .as_deref()
                .is_some_and(|identity| identity == canonical_bytes)
    }
    #[cfg(test)]
    pub(crate) fn claim_adapter_service_for_test(&self) -> bool {
        self.claim_adapter_service_once()
    }
    /// Attach the fake runtime's exact wrapper fields to this test capability.
    #[cfg(test)]
    pub(crate) fn bind_runtime_ownership_for_test(
        &mut self,
        causal_lifecycle_key: Hash,
        initial_lifecycle_ordinal: u128,
        source_physical_ordinal: Option<u64>,
        physical_cut: u128,
    ) -> Option<DeferredRuntimeOwnershipSeal> {
        let binding = DeferredRuntimeOwnershipBinding {
            causal_lifecycle_key,
            initial_lifecycle_ordinal,
            authenticated_ingress: self.is_authenticated_ingress(),
            source_physical_ordinal,
            physical_cut,
        };
        if !binding.validate_exact()
            || self
                .admission_capability
                .runtime_ownership
                .as_ref()
                .is_some_and(|retained| retained != &binding)
        {
            return None;
        }
        self.admission_capability.runtime_ownership = Some(binding);
        self.projection_hash = deferred_service_projection_hash(self);
        self.validate_exact()
            .then(|| self.admission_capability.runtime_ownership_seal())
            .flatten()
    }
    /// Return whether this occurrence was minted by the supplied actor-owned
    /// source rather than another runtime actor with an overlapping ordinal.
    pub(crate) fn belongs_to(&self, source: &DeferredAdmissionOrdinalSource) -> bool {
        let exact = Arc::ptr_eq(&self.admission_capability.source_identity, &source.identity);
        #[cfg(test)]
        {
            exact || self.admission_capability.unbound_fixture
        }
        #[cfg(not(test))]
        {
            exact
        }
    }
    /// Verify that post-service evidence came from the same adapter capability
    /// whose immutable runtime seal was retained before selection.
    pub(crate) fn matches_runtime_ownership_seal(
        &self,
        seal: &DeferredRuntimeOwnershipSeal,
    ) -> bool {
        self.validate_exact()
            && self.service_handoff_is_complete()
            && seal.validate_identity()
            && seal.matches_capability(&self.admission_capability)
    }
}
struct DeferredServiceSelection {
    input: DeferredInput,
    evidence: DeferredServiceEvidence,
}
fn deferred_event_kind(event: &reducer::Event) -> DeferredEventKind {
    match event {
        reducer::Event::ResumeAfterReplay { .. } => DeferredEventKind::ResumeAfterReplay,
        reducer::Event::LocalProposalReady { .. } => DeferredEventKind::LocalProposalReady,
        reducer::Event::ProposalReceived { .. } => DeferredEventKind::ProposalReceived,
        reducer::Event::VoteReceived { .. } => DeferredEventKind::VoteReceived,
        reducer::Event::QuorumCertificateReceived { .. } => {
            DeferredEventKind::QuorumCertificateReceived
        }
        reducer::Event::TimeoutVoteReceived { .. } => DeferredEventKind::TimeoutVoteReceived,
        reducer::Event::TimeoutCertificateReceived { .. } => {
            DeferredEventKind::TimeoutCertificateReceived
        }
        reducer::Event::TimeoutElapsed { .. } => DeferredEventKind::TimeoutElapsed,
        reducer::Event::RetransmitElapsed { .. } => DeferredEventKind::RetransmitElapsed,
        reducer::Event::BodyAvailable { .. } => DeferredEventKind::BodyAvailable,
        reducer::Event::BodyStored { .. } => DeferredEventKind::BodyStored,
        reducer::Event::ValidationCompleted { .. } => DeferredEventKind::ValidationCompleted,
        reducer::Event::Persisted { .. } => DeferredEventKind::Persisted,
        reducer::Event::PersistenceFailed { .. } => DeferredEventKind::PersistenceFailed,
        reducer::Event::Signed { .. } => DeferredEventKind::Signed,
        reducer::Event::ApplicationCompleted { .. } => DeferredEventKind::ApplicationCompleted,
    }
}
fn deferred_event_tag(event: &reducer::Event) -> reducer::EventTag {
    match event {
        reducer::Event::ResumeAfterReplay { tag }
        | reducer::Event::LocalProposalReady { tag, .. }
        | reducer::Event::ProposalReceived { tag, .. }
        | reducer::Event::VoteReceived { tag, .. }
        | reducer::Event::QuorumCertificateReceived { tag, .. }
        | reducer::Event::TimeoutVoteReceived { tag, .. }
        | reducer::Event::TimeoutCertificateReceived { tag, .. }
        | reducer::Event::TimeoutElapsed { tag }
        | reducer::Event::RetransmitElapsed { tag }
        | reducer::Event::BodyAvailable { tag, .. }
        | reducer::Event::BodyStored { tag, .. }
        | reducer::Event::ValidationCompleted { tag, .. }
        | reducer::Event::Persisted { tag, .. }
        | reducer::Event::PersistenceFailed { tag, .. }
        | reducer::Event::Signed { tag, .. }
        | reducer::Event::ApplicationCompleted { tag, .. } => *tag,
    }
}
fn append_deferred_projection_field(projection: &mut Vec<u8>, field: &[u8]) {
    let len = u64::try_from(field.len()).expect("bounded deferred projection field fits u64");
    projection.extend_from_slice(&len.to_le_bytes());
    projection.extend_from_slice(field);
}
fn append_deferred_projection_u64(projection: &mut Vec<u8>, value: u64) {
    append_deferred_projection_field(projection, &value.to_le_bytes());
}
fn append_deferred_projection_tag(projection: &mut Vec<u8>, tag: reducer::EventTag) {
    append_deferred_projection_u64(projection, tag.height());
    append_deferred_projection_u64(projection, tag.view());
    append_deferred_projection_u64(projection, tag.generation().get());
}
fn append_deferred_projection_round(projection: &mut Vec<u8>, round: reducer::Round) {
    append_deferred_projection_u64(projection, round.height());
    append_deferred_projection_u64(projection, round.view());
}
fn append_deferred_projection_phase(projection: &mut Vec<u8>, phase: reducer::Phase) {
    projection.push(match phase {
        reducer::Phase::Prepare => 1,
        reducer::Phase::Commit => 2,
    });
}
fn append_deferred_projection_signature(
    projection: &mut Vec<u8>,
    signature: &reducer::OpaqueSignature,
) {
    append_deferred_projection_field(projection, signature.as_bytes());
}
fn append_deferred_projection_certificate(
    projection: &mut Vec<u8>,
    certificate: &reducer::QuorumCertificate,
) {
    let reference = certificate.reference();
    append_deferred_projection_field(projection, reference.context_id().as_bytes());
    append_deferred_projection_round(projection, reference.round());
    append_deferred_projection_round(projection, reference.proposal_round());
    append_deferred_projection_phase(projection, reference.phase());
    append_deferred_projection_field(projection, reference.subject().as_bytes());
    append_deferred_projection_u64(
        projection,
        u64::try_from(certificate.signatures().len())
            .expect("bounded certificate signer count fits u64"),
    );
    for share in certificate.signatures() {
        append_deferred_projection_field(projection, share.signer().as_bytes());
        append_deferred_projection_signature(projection, share.signature());
    }
}
fn append_deferred_projection_manifest(
    projection: &mut Vec<u8>,
    manifest: &reducer::PayloadManifest,
) {
    append_deferred_projection_field(projection, manifest.subject().as_bytes());
    append_deferred_projection_field(projection, manifest.payload_hash().as_bytes());
    append_deferred_projection_field(projection, manifest.chunk_root().as_bytes());
    append_deferred_projection_u64(projection, manifest.byte_len());
    append_deferred_projection_field(projection, &manifest.chunk_count().to_le_bytes());
}
fn append_deferred_projection_timeout_certificate(
    projection: &mut Vec<u8>,
    certificate: &reducer::TimeoutCertificate,
) {
    append_deferred_projection_field(projection, certificate.context_id().as_bytes());
    append_deferred_projection_round(projection, certificate.round());
    append_deferred_projection_u64(
        projection,
        u64::try_from(certificate.groups().len()).expect("bounded timeout group count fits u64"),
    );
    for group in certificate.groups() {
        match group.highest_prepare() {
            Some(highest_prepare) => {
                projection.push(1);
                append_deferred_projection_certificate(projection, highest_prepare);
            }
            None => projection.push(0),
        }
        append_deferred_projection_u64(
            projection,
            u64::try_from(group.signatures().len()).expect("bounded timeout signer count fits u64"),
        );
        for share in group.signatures() {
            append_deferred_projection_field(projection, share.signer().as_bytes());
            append_deferred_projection_signature(projection, share.signature());
        }
    }
}
/// Append the semantic identity of a certified occurrence without projecting
/// the replaceable quorum subset or aggregate-signature carrier.
fn append_serviced_candidate_certificate(
    projection: &mut Vec<u8>,
    certificate: &reducer::QuorumCertificate,
) {
    let reference = certificate.reference();
    append_deferred_projection_field(projection, reference.context_id().as_bytes());
    append_deferred_projection_round(projection, reference.round());
    append_deferred_projection_round(projection, reference.proposal_round());
    append_deferred_projection_phase(projection, reference.phase());
    append_deferred_projection_field(projection, reference.subject().as_bytes());
}
/// Append the semantic timeout occurrence selected by the certified round and
/// highest Prepare reference. Signer grouping and aggregate bytes are
/// authenticated carriers, not additional logical owners.
fn append_serviced_candidate_timeout_certificate(
    projection: &mut Vec<u8>,
    certificate: &reducer::TimeoutCertificate,
) {
    append_deferred_projection_field(projection, certificate.context_id().as_bytes());
    append_deferred_projection_round(projection, certificate.round());
    match certificate.highest_prepare() {
        Some(highest_prepare) => {
            projection.push(1);
            append_serviced_candidate_certificate(projection, highest_prepare);
        }
        None => projection.push(0),
    }
}
fn append_deferred_projection_event(projection: &mut Vec<u8>, event: &reducer::Event) {
    projection.push(deferred_event_kind(event).code());
    append_deferred_projection_tag(projection, deferred_event_tag(event));
    match event {
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. } => {}
        reducer::Event::LocalProposalReady { manifest, .. } => {
            append_deferred_projection_manifest(projection, manifest);
        }
        reducer::Event::ProposalReceived { proposal, .. } => {
            let body = proposal.proposal();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_field(projection, body.proposer().as_bytes());
            append_deferred_projection_manifest(projection, body.manifest());
            match body.justification() {
                reducer::ProposalJustification::ParentCommit(reference) => {
                    projection.push(1);
                    match reference {
                        Some(reference) => {
                            projection.push(1);
                            append_deferred_projection_field(
                                projection,
                                reference.context_id().as_bytes(),
                            );
                            append_deferred_projection_round(projection, reference.round());
                            append_deferred_projection_round(
                                projection,
                                reference.proposal_round(),
                            );
                            append_deferred_projection_phase(projection, reference.phase());
                            append_deferred_projection_field(
                                projection,
                                reference.subject().as_bytes(),
                            );
                        }
                        None => projection.push(0),
                    }
                }
                reducer::ProposalJustification::Timeout(certificate) => {
                    projection.push(2);
                    append_deferred_projection_timeout_certificate(projection, certificate);
                }
            }
            append_deferred_projection_signature(projection, proposal.signature());
        }
        reducer::Event::VoteReceived { vote, .. } => {
            let body = vote.vote();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_round(projection, body.proposal_round());
            append_deferred_projection_phase(projection, body.phase());
            append_deferred_projection_field(projection, body.subject().as_bytes());
            append_deferred_projection_field(projection, body.signer().as_bytes());
            append_deferred_projection_signature(projection, vote.signature());
        }
        reducer::Event::QuorumCertificateReceived { certificate, .. } => {
            append_deferred_projection_certificate(projection, certificate);
        }
        reducer::Event::TimeoutVoteReceived { vote, .. } => {
            let body = vote.vote();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_field(projection, body.signer().as_bytes());
            match body.highest_prepare() {
                Some(highest_prepare) => {
                    projection.push(1);
                    append_deferred_projection_certificate(projection, highest_prepare);
                }
                None => projection.push(0),
            }
            append_deferred_projection_signature(projection, vote.signature());
        }
        reducer::Event::TimeoutCertificateReceived { certificate, .. } => {
            append_deferred_projection_timeout_certificate(projection, certificate);
        }
        reducer::Event::BodyAvailable { round, subject, .. }
        | reducer::Event::BodyStored { round, subject, .. } => {
            append_deferred_projection_round(projection, *round);
            append_deferred_projection_field(projection, subject.as_bytes());
        }
        reducer::Event::ValidationCompleted {
            round,
            subject,
            valid,
            ..
        } => {
            append_deferred_projection_round(projection, *round);
            append_deferred_projection_field(projection, subject.as_bytes());
            projection.push(u8::from(*valid));
        }
        reducer::Event::Persisted { id, .. } | reducer::Event::PersistenceFailed { id, .. } => {
            append_deferred_projection_u64(projection, id.get());
        }
        reducer::Event::Signed { signature, .. } => {
            append_deferred_projection_signature(projection, signature);
        }
        reducer::Event::ApplicationCompleted { subject, .. } => {
            append_deferred_projection_field(projection, subject.as_bytes());
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServicedCandidatePolicy {
    /// An already durable occurrence is consumed without re-entering the reducer.
    Suppress,
}
/// Closed adapter-event projection used by the serviced-identity bound.
///
/// These are exactly the reducer input classes which may retain a transient
/// or durable-terminal service record. The formal model projects its more
/// detailed command stages onto this same eleven-class carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum ServicedCandidateStage {
    LocalProposalReady,
    ProposalReceived,
    VoteReceived,
    QuorumCertificateReceived,
    TimeoutVoteReceived,
    TimeoutCertificateReceived,
    TimeoutElapsed,
    BodyAvailable,
    BodyStored,
    ValidationCompleted,
    ApplicationCompleted,
}
impl ServicedCandidateStage {
    const ALL: [Self; 11] = [
        Self::LocalProposalReady,
        Self::ProposalReceived,
        Self::VoteReceived,
        Self::QuorumCertificateReceived,
        Self::TimeoutVoteReceived,
        Self::TimeoutCertificateReceived,
        Self::TimeoutElapsed,
        Self::BodyAvailable,
        Self::BodyStored,
        Self::ValidationCompleted,
        Self::ApplicationCompleted,
    ];
    const COUNT: usize = Self::ALL.len();
    const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::LocalProposalReady),
            1 => Some(Self::ProposalReceived),
            2 => Some(Self::VoteReceived),
            3 => Some(Self::QuorumCertificateReceived),
            4 => Some(Self::TimeoutVoteReceived),
            5 => Some(Self::TimeoutCertificateReceived),
            6 => Some(Self::TimeoutElapsed),
            7 => Some(Self::BodyAvailable),
            8 => Some(Self::BodyStored),
            9 => Some(Self::ValidationCompleted),
            10 => Some(Self::ApplicationCompleted),
            _ => None,
        }
    }
}
/// Physical source which makes a volatile producer parent replayable after a
/// same-height crash.
///
/// This is an internal proof/refinement classifier, not a wire field or
/// configuration knob. It classifies physical replay and capacity ownership,
/// not whether restart-stable logical lifecycle metadata exists. In
/// particular, `BodyAvailable` retains its logical producer lifecycle across
/// restart, but neither body bytes nor a latent FIFO slot: `FetchBody`
/// reacquires the bytes and the exact completion spends one fresh FIFO slot.
/// Only classes backed by an independently owned local durable source install
/// a dormant local FIFO reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProducerParentReplaySource {
    /// Authenticated ingress is useful only under the explicit responsive-peer
    /// retransmission assumption. A receiver cannot infer that assumption
    /// from a signature, so this class never authorizes a local continuation.
    ConditionalResponsiveTransport,
    /// A reconstructed body has not crossed the durable store boundary yet.
    /// Its manifest is not a reconstruction source for the body bytes.
    VolatileBodyReconstruction,
    /// Exact manifest and non-forgeable receipt retained by the body pipeline.
    DurableBodyPipeline,
    /// Durable reducer view/intent deterministically recreates this local root.
    SafetyWal,
    /// Durable Decision recreates Apply until the matching completion arrives.
    DurableDecision,
}
const fn producer_parent_replay_source_for_stage(
    stage: ServicedCandidateStage,
) -> ProducerParentReplaySource {
    match stage {
        ServicedCandidateStage::ProposalReceived
        | ServicedCandidateStage::VoteReceived
        | ServicedCandidateStage::QuorumCertificateReceived
        | ServicedCandidateStage::TimeoutVoteReceived
        | ServicedCandidateStage::TimeoutCertificateReceived => {
            ProducerParentReplaySource::ConditionalResponsiveTransport
        }
        ServicedCandidateStage::BodyAvailable => {
            ProducerParentReplaySource::VolatileBodyReconstruction
        }
        ServicedCandidateStage::LocalProposalReady
        | ServicedCandidateStage::BodyStored
        | ServicedCandidateStage::ValidationCompleted => {
            ProducerParentReplaySource::DurableBodyPipeline
        }
        ServicedCandidateStage::TimeoutElapsed => ProducerParentReplaySource::SafetyWal,
        ServicedCandidateStage::ApplicationCompleted => ProducerParentReplaySource::DurableDecision,
    }
}
const fn producer_parent_is_locally_reconstructible(stage: ServicedCandidateStage) -> bool {
    matches!(
        producer_parent_replay_source_for_stage(stage),
        ProducerParentReplaySource::DurableBodyPipeline
            | ProducerParentReplaySource::SafetyWal
            | ProducerParentReplaySource::DurableDecision
    )
}
fn producer_parent_has_exact_local_replay_binding(
    event: &reducer::Event,
    completion_evidence: Option<&BodyPipelineCompletionEvidence>,
    durable_decision: bool,
) -> bool {
    let Some(stage) = serviced_candidate_stage(event) else {
        return true;
    };
    match producer_parent_replay_source_for_stage(stage) {
        ProducerParentReplaySource::ConditionalResponsiveTransport => false,
        ProducerParentReplaySource::VolatileBodyReconstruction => false,
        ProducerParentReplaySource::DurableBodyPipeline => matches!(
            (event, completion_evidence),
            (
                reducer::Event::LocalProposalReady { .. },
                Some(BodyPipelineCompletionEvidence::LocalProposalReady { .. })
            ) | (
                reducer::Event::BodyStored { .. },
                Some(BodyPipelineCompletionEvidence::BodyStored { .. })
            )
        ),
        ProducerParentReplaySource::SafetyWal => {
            matches!(event, reducer::Event::TimeoutElapsed { .. })
        }
        ProducerParentReplaySource::DurableDecision => {
            durable_decision && matches!(event, reducer::Event::ApplicationCompleted { .. })
        }
    }
}
fn serviced_candidate_stage(event: &reducer::Event) -> Option<ServicedCandidateStage> {
    let stage = serviced_candidate_stage_for_kind_code(deferred_event_kind(event).code())?;
    ServicedCandidateStage::from_code(stage)
}
fn serviced_candidate_policy(event: &reducer::Event) -> Option<ServicedCandidatePolicy> {
    serviced_candidate_stage(event).map(|_| ServicedCandidatePolicy::Suppress)
}
fn is_authenticated_ingress_event(event: &reducer::Event) -> bool {
    matches!(
        event,
        reducer::Event::ProposalReceived { .. }
            | reducer::Event::VoteReceived { .. }
            | reducer::Event::QuorumCertificateReceived { .. }
            | reducer::Event::TimeoutVoteReceived { .. }
            | reducer::Event::TimeoutCertificateReceived { .. }
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServicedCandidateRecordKind {
    /// Same-process memory which prevents an applied identity from re-entering
    /// after an equal-rank owner temporarily covers the same reducer stage.
    Transient,
    /// Restart-stable memory for an exact internal lifecycle which drained
    /// after its asynchronous owner disappeared.
    DurableTerminal,
}
/// Exact process-local lifecycle owner supplied by the serialized runtime.
///
/// This carrier is deliberately not serialized. The causal key and immutable
/// admission ordinal are sufficient to coalesce retries while the process is
/// alive, but not to reconstruct a command after restart.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectedProducerLifecycle {
    causal_lifecycle_key: Hash,
    admission_ordinal: u128,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum ProducerReservationChange {
    Unchanged,
    Inserted,
    ClaimedDormant,
    ReplacedTerminal {
        process_previous: ProducerContinuationRecord,
        durable_previous: Option<ProducerContinuationRecord>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ProducerReservationToken {
    address: ProducerContinuationAddress,
    change: ProducerReservationChange,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingProducerHandoff {
    token: ProducerContinuationHandoffToken,
    service_view: wire::View,
    durable_store_terminal: bool,
    durable_terminal_evidence: bool,
    durable_previous: Option<ProducerContinuationRecord>,
}
/// Exact evidence consumed when a runtime-owned producer reservation retires.
///
/// A concrete successor is acknowledged only after the runtime has installed
/// the returned non-empty effect batch in its ownership sidecar. A durable
/// terminal is accepted only when the adapter retained exact terminal evidence
/// for the same opaque token before returning from the reducer transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProducerContinuationHandoffEvidence {
    /// An inherited or fresh causal successor is now physically owned.
    ConcreteSuccessor,
    /// The drained producer reached a process-local last consumer without an
    /// independently durable terminal. Same-height restart must reopen it.
    VolatileTerminal,
    /// Exact durable terminal evidence replaces the drained producer.
    DurableTerminal,
}
/// Classify the only dispositions which consume a serviced-identity slot.
///
/// Authenticated junk, stale/policy rejection, and ordinary duplicates remain
/// marker-free. An applied authenticated occurrence is not junk: retaining its
/// route-neutral identity for the rest of this process generation closes the
/// A -> B -> A replenishment episode while a same-height restart still clears
/// it and permits volatile quorum/pipeline reconstruction.
fn serviced_candidate_record_kind(
    event: &reducer::Event,
    disposition: reducer::StepDisposition,
) -> Option<ServicedCandidateRecordKind> {
    if disposition == reducer::StepDisposition::Applied {
        Some(ServicedCandidateRecordKind::Transient)
    } else if !is_authenticated_ingress_event(event)
        && disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    {
        Some(ServicedCandidateRecordKind::DurableTerminal)
    } else {
        None
    }
}
/// Append a route-neutral event projection which deliberately excludes the
/// process-local reducer generation and consumer-episode tag. The immutable
/// height context and semantic source view are projected by the caller.
fn append_serviced_candidate_event(projection: &mut Vec<u8>, event: &reducer::Event) {
    projection.push(deferred_event_kind(event).code());
    match event {
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. } => {}
        reducer::Event::LocalProposalReady { manifest, .. } => {
            append_deferred_projection_manifest(projection, manifest);
        }
        reducer::Event::ProposalReceived { proposal, .. } => {
            let body = proposal.proposal();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_field(projection, body.proposer().as_bytes());
            append_deferred_projection_manifest(projection, body.manifest());
            match body.justification() {
                reducer::ProposalJustification::ParentCommit(reference) => {
                    projection.push(1);
                    match reference {
                        Some(reference) => {
                            projection.push(1);
                            append_deferred_projection_field(
                                projection,
                                reference.context_id().as_bytes(),
                            );
                            append_deferred_projection_round(projection, reference.round());
                            append_deferred_projection_round(
                                projection,
                                reference.proposal_round(),
                            );
                            append_deferred_projection_phase(projection, reference.phase());
                            append_deferred_projection_field(
                                projection,
                                reference.subject().as_bytes(),
                            );
                        }
                        None => projection.push(0),
                    }
                }
                reducer::ProposalJustification::Timeout(certificate) => {
                    projection.push(2);
                    append_serviced_candidate_timeout_certificate(projection, certificate);
                }
            }
        }
        reducer::Event::VoteReceived { vote, .. } => {
            let body = vote.vote();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_round(projection, body.proposal_round());
            append_deferred_projection_phase(projection, body.phase());
            append_deferred_projection_field(projection, body.subject().as_bytes());
            append_deferred_projection_field(projection, body.signer().as_bytes());
        }
        reducer::Event::QuorumCertificateReceived { certificate, .. } => {
            append_serviced_candidate_certificate(projection, certificate);
        }
        reducer::Event::TimeoutVoteReceived { vote, .. } => {
            let body = vote.vote();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_field(projection, body.signer().as_bytes());
            match body.highest_prepare() {
                Some(highest_prepare) => {
                    projection.push(1);
                    append_serviced_candidate_certificate(projection, highest_prepare);
                }
                None => projection.push(0),
            }
        }
        reducer::Event::TimeoutCertificateReceived { certificate, .. } => {
            append_serviced_candidate_timeout_certificate(projection, certificate);
        }
        reducer::Event::BodyAvailable { round, subject, .. }
        | reducer::Event::BodyStored { round, subject, .. } => {
            append_deferred_projection_round(projection, *round);
            append_deferred_projection_field(projection, subject.as_bytes());
        }
        reducer::Event::ValidationCompleted {
            round,
            subject,
            valid,
            ..
        } => {
            append_deferred_projection_round(projection, *round);
            append_deferred_projection_field(projection, subject.as_bytes());
            projection.push(u8::from(*valid));
        }
        reducer::Event::Persisted { id, .. } | reducer::Event::PersistenceFailed { id, .. } => {
            append_deferred_projection_u64(projection, id.get());
        }
        reducer::Event::Signed { signature, .. } => {
            append_deferred_projection_signature(projection, signature);
        }
        reducer::Event::ApplicationCompleted { subject, .. } => {
            append_deferred_projection_field(projection, subject.as_bytes());
        }
    }
}
fn serviced_candidate_event_fields(event: &reducer::Event) -> (wire::View, Option<[u8; 32]>, u8) {
    let tag_view = deferred_event_tag(event).view();
    match event {
        reducer::Event::LocalProposalReady { manifest, .. } => {
            (tag_view, Some(*manifest.subject().as_bytes()), 0)
        }
        reducer::Event::ProposalReceived { proposal, .. } => {
            let proposal = proposal.proposal();
            (
                proposal.round().view(),
                Some(*proposal.manifest().subject().as_bytes()),
                0,
            )
        }
        reducer::Event::VoteReceived { vote, .. } => {
            let vote = vote.vote();
            (
                vote.round().view(),
                Some(*vote.subject().as_bytes()),
                match vote.phase() {
                    reducer::Phase::Prepare => 1,
                    reducer::Phase::Commit => 2,
                },
            )
        }
        reducer::Event::QuorumCertificateReceived { certificate, .. } => (
            certificate.round().view(),
            Some(*certificate.subject().as_bytes()),
            match certificate.phase() {
                reducer::Phase::Prepare => 1,
                reducer::Phase::Commit => 2,
            },
        ),
        reducer::Event::TimeoutVoteReceived { vote, .. } => {
            let vote = vote.vote();
            (
                vote.round().view(),
                vote.highest_prepare()
                    .map(|certificate| *certificate.subject().as_bytes()),
                3,
            )
        }
        reducer::Event::TimeoutCertificateReceived { certificate, .. } => (
            certificate.round().view(),
            certificate
                .highest_prepare()
                .map(|highest| *highest.subject().as_bytes()),
            3,
        ),
        reducer::Event::BodyAvailable { round, subject, .. }
        | reducer::Event::BodyStored { round, subject, .. }
        | reducer::Event::ValidationCompleted { round, subject, .. } => {
            (round.view(), Some(*subject.as_bytes()), 0)
        }
        reducer::Event::ApplicationCompleted { subject, .. } => {
            (tag_view, Some(*subject.as_bytes()), 2)
        }
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. }
        | reducer::Event::Persisted { .. }
        | reducer::Event::PersistenceFailed { .. }
        | reducer::Event::Signed { .. } => (tag_view, None, 0),
    }
}
fn append_deferred_projection_receipt(projection: &mut Vec<u8>, receipt: &DurableBodyReceipt) {
    append_deferred_projection_field(projection, &receipt.context_id().encode());
    append_deferred_projection_field(projection, &receipt.round().encode());
    append_deferred_projection_field(projection, &receipt.subject().encode());
    append_deferred_projection_field(projection, receipt.manifest_hash().as_ref());
    append_deferred_projection_field(projection, receipt.frame_hash().as_ref());
}
fn append_deferred_projection_completion_evidence(
    projection: &mut Vec<u8>,
    evidence: Option<&BodyPipelineCompletionEvidence>,
) {
    let Some(evidence) = evidence else {
        projection.push(0);
        return;
    };
    match evidence {
        BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest,
            durable_receipt,
            validated_receipt,
        } => {
            projection.push(1);
            append_deferred_projection_field(projection, &manifest.encode());
            append_deferred_projection_receipt(projection, durable_receipt);
            append_deferred_projection_field(
                projection,
                &validated_receipt.execution_commitment().encode(),
            );
        }
        BodyPipelineCompletionEvidence::BodyAvailable { manifest } => {
            projection.push(2);
            append_deferred_projection_field(projection, &manifest.encode());
        }
        BodyPipelineCompletionEvidence::BodyStored {
            round,
            subject,
            receipt,
        } => {
            projection.push(3);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &subject.encode());
            append_deferred_projection_receipt(projection, receipt);
        }
    }
}
fn append_deferred_projection_admission(
    projection: &mut Vec<u8>,
    admission: Option<IngressAdmission>,
) {
    let Some(admission) = admission else {
        projection.push(0);
        return;
    };
    projection.push(1);
    match admission.key {
        IngressSemanticKey::Proposal { round, proposer } => {
            projection.push(1);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &proposer.encode());
        }
        IngressSemanticKey::Vote {
            round,
            phase,
            signer,
        } => {
            projection.push(2);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &phase.encode());
            append_deferred_projection_field(projection, &signer.encode());
        }
        IngressSemanticKey::TimeoutVote { round, signer } => {
            projection.push(3);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &signer.encode());
        }
    }
    match admission.fingerprint {
        IngressFingerprint::Proposal(hash) => {
            projection.push(1);
            append_deferred_projection_field(projection, hash.as_ref());
        }
        IngressFingerprint::Vote(proposal_round, subject, commitment) => {
            projection.push(2);
            append_deferred_projection_field(projection, &proposal_round.encode());
            append_deferred_projection_field(projection, &subject.encode());
            append_deferred_projection_field(projection, &commitment.encode());
        }
        IngressFingerprint::TimeoutVote(reference) => {
            projection.push(3);
            append_deferred_projection_field(projection, &reference.encode());
        }
    }
    append_deferred_projection_tag(projection, admission.consumer_tag);
    projection.push(u8::from(admission.inserted_equivocation));
    projection.push(u8::from(admission.locked_commit_progress));
    projection.push(u8::from(admission.locked_reproposal_prepare_progress));
}
fn deferred_service_projection_hash(evidence: &DeferredServiceEvidence) -> Hash {
    let mut projection = Vec::new();
    append_deferred_projection_field(&mut projection, &evidence.admission_ordinal.to_le_bytes());
    projection.push(evidence.priority.code());
    projection.push(evidence.event_kind.code());
    append_deferred_projection_tag(&mut projection, evidence.original_tag);
    append_deferred_projection_tag(&mut projection, evidence.effective_tag);
    match evidence.retag {
        DeferredRetagRelation::Unchanged => projection.push(0),
        DeferredRetagRelation::AuthenticatedIngress { from, to } => {
            projection.push(1);
            append_deferred_projection_tag(&mut projection, from);
            append_deferred_projection_tag(&mut projection, to);
        }
    }
    projection.push(u8::from(evidence.protected_progress));
    append_deferred_projection_u64(&mut projection, evidence.eligible_skips_before);
    append_deferred_projection_u64(&mut projection, evidence.eligible_skips_after);
    for lengths in [evidence.queue_lengths_before, evidence.queue_lengths_after] {
        append_deferred_projection_u64(&mut projection, lengths.completion);
        append_deferred_projection_u64(&mut projection, lengths.progress);
        append_deferred_projection_u64(&mut projection, lengths.normal);
    }
    append_deferred_projection_u64(
        &mut projection,
        evidence.eligible_queue_lengths_before.completion,
    );
    append_deferred_projection_u64(
        &mut projection,
        evidence.eligible_queue_lengths_before.progress,
    );
    append_deferred_projection_u64(
        &mut projection,
        evidence.eligible_queue_lengths_before.normal,
    );
    append_deferred_projection_u64(&mut projection, evidence.total_len_before);
    append_deferred_projection_u64(&mut projection, evidence.total_len_after);
    projection.push(evidence.service_cursor_before.code());
    projection.push(evidence.service_cursor_after.code());
    append_deferred_projection_event(&mut projection, &evidence.original_event);
    append_deferred_projection_event(&mut projection, &evidence.effective_event);
    append_deferred_projection_completion_evidence(
        &mut projection,
        evidence.completion_evidence.as_ref(),
    );
    append_deferred_projection_admission(&mut projection, evidence.original_admission);
    append_deferred_projection_admission(&mut projection, evidence.effective_admission);
    match &evidence.authenticated_wire_identity {
        None => projection.push(0),
        Some(identity) => {
            projection.push(1);
            append_deferred_projection_field(&mut projection, identity);
        }
    }
    Hash::new(projection)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredProgressClass {
    LockedCommitVote,
    LockedReproposalPrepareVote,
    TimeoutVote,
    PrepareCertificate,
    CommitCertificate,
    TimeoutCertificate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredProgressOwner {
    LockedCommitVote(reducer::ValidatorId),
    LockedReproposalPrepareVote(reducer::ValidatorId),
    TimeoutVote(reducer::ValidatorId),
    PrepareCertificate,
    CommitCertificate,
    TimeoutCertificate,
}
impl DeferredProgressOwner {
    const fn class(self) -> DeferredProgressClass {
        match self {
            Self::LockedCommitVote(_) => DeferredProgressClass::LockedCommitVote,
            Self::LockedReproposalPrepareVote(_) => {
                DeferredProgressClass::LockedReproposalPrepareVote
            }
            Self::TimeoutVote(_) => DeferredProgressClass::TimeoutVote,
            Self::PrepareCertificate => DeferredProgressClass::PrepareCertificate,
            Self::CommitCertificate => DeferredProgressClass::CommitCertificate,
            Self::TimeoutCertificate => DeferredProgressClass::TimeoutCertificate,
        }
    }
}
fn deferred_progress_owner(input: &DeferredInput) -> Option<DeferredProgressOwner> {
    if input.protected_progress {
        return match &input.event {
            reducer::Event::VoteReceived { vote, .. }
                if vote.vote().phase() == reducer::Phase::Commit =>
            {
                Some(DeferredProgressOwner::LockedCommitVote(
                    vote.vote().signer(),
                ))
            }
            reducer::Event::VoteReceived { vote, .. }
                if vote.vote().phase() == reducer::Phase::Prepare =>
            {
                Some(DeferredProgressOwner::LockedReproposalPrepareVote(
                    vote.vote().signer(),
                ))
            }
            _ => None,
        };
    }
    match &input.event {
        reducer::Event::TimeoutVoteReceived { vote, .. } => {
            Some(DeferredProgressOwner::TimeoutVote(vote.vote().signer()))
        }
        reducer::Event::QuorumCertificateReceived { certificate, .. } => {
            Some(match certificate.phase() {
                reducer::Phase::Prepare => DeferredProgressOwner::PrepareCertificate,
                reducer::Phase::Commit => DeferredProgressOwner::CommitCertificate,
            })
        }
        reducer::Event::TimeoutCertificateReceived { .. } => {
            Some(DeferredProgressOwner::TimeoutCertificate)
        }
        _ => None,
    }
}
fn deferred_progress_class(input: &DeferredInput) -> Option<DeferredProgressClass> {
    deferred_progress_owner(input).map(DeferredProgressOwner::class)
}
const fn deferred_progress_capacity(roster_len: usize) -> usize {
    let required = roster_len.saturating_mul(3).saturating_add(3);
    if required < MAX_DEFERRED_PROGRESS_INPUTS {
        required
    } else {
        MAX_DEFERRED_PROGRESS_INPUTS
    }
}
const fn semantic_ingress_capacity(roster_len: usize) -> usize {
    // Exact locked Commit and current-view locked-reproposal Prepare sets plus
    // current and adjacent-future TimeoutVote sets bypass the ordinary table.
    MAX_INGRESS_SEMANTIC_KEYS.saturating_add(roster_len.saturating_mul(4))
}
/// Maximum distinct service stages which one immutable lifecycle can cross.
///
/// This is mechanically derived from the closed reducer-event projection
/// accepted by [`serviced_candidate_policy`], not duplicated as a magic
/// number. It is neither a wire field nor a deployment knob.
const _: () = assert!(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE == ServicedCandidateStage::COUNT);
const _: () = assert!(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE == 11);
/// Dormant restart/historical roots in the reviewed lifecycle geometry.
///
/// The formal model uses the same fixed `AsyncDormantDurableLifecycleCapacity`.
const CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY: usize = 8;
/// Existing runtime/effect capacities which bound active candidate roots.
///
/// Production passes the already validated Sumeragi v2 queue configuration
/// into adapter construction. This internal value is not serialized and does
/// not add a configuration surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ServicedCandidateCapacityGeometry {
    runtime_command_capacity: usize,
    effect_work_capacity: usize,
}
impl ServicedCandidateCapacityGeometry {
    /// Bind the existing runtime command and effect-work capacities.
    pub(crate) const fn new(runtime_command_capacity: usize, effect_work_capacity: usize) -> Self {
        Self {
            runtime_command_capacity,
            effect_work_capacity,
        }
    }
}
// Standalone adapter fixtures are paired with the existing 1024-command and
// 1024-effect test defaults. Production construction always supplies the
// validated height configuration explicitly through the runner.
#[cfg(test)]
const DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY: ServicedCandidateCapacityGeometry =
    ServicedCandidateCapacityGeometry::new(MAX_DEFERRED_INPUTS, MAX_DEFERRED_INPUTS);
const fn candidate_lifecycle_capacity(
    roster_len: usize,
    geometry: ServicedCandidateCapacityGeometry,
) -> usize {
    let serviced = semantic_ingress_capacity(roster_len)
        .saturating_add(MAX_DEFERRED_INPUTS)
        .saturating_add(MAX_DEFERRED_INPUTS)
        .saturating_add(deferred_progress_capacity(roster_len));
    let active = geometry
        .runtime_command_capacity
        // One root plus at most three causal continuations per runtime owner.
        .saturating_add(geometry.runtime_command_capacity.saturating_mul(3))
        .saturating_add(geometry.effect_work_capacity)
        .saturating_add(CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY);
    serviced
        .saturating_add(active)
        // The due timeout clock owns one disjoint lifecycle reservation.
        .saturating_add(1)
}
/// Maximum same-view serviced identities retained by one adapter generation.
///
/// This is the complete reviewed lifecycle geometry: service queues, active
/// runtime roots, their bounded three-child causal continuations, effect work,
/// dormant durable replay, and the disjoint timeout clock. Multiplying by the
/// exact eleven-class reducer-event projection also covers a retained service
/// marker while the same causal lifecycle remains active.
#[cfg(test)]
const fn serviced_candidate_capacity_with_geometry(
    roster_len: usize,
    geometry: ServicedCandidateCapacityGeometry,
) -> usize {
    candidate_lifecycle_capacity(roster_len, geometry)
        .saturating_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
}
#[cfg(test)]
const fn serviced_candidate_capacity(roster_len: usize) -> usize {
    serviced_candidate_capacity_with_geometry(
        roster_len,
        DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY,
    )
}
/// Completion variant staged directly in the Busy-deferred lane by seam tests.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredBodyPipelineStageForTest {
    /// Body reconstruction completed.
    BodyAvailable,
    /// Durable storage completed.
    BodyStored,
    /// Local proposal construction completed.
    LocalProposalReady,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredBodyPipelineCompletionStage {
    LocalProposalReady,
    BodyAvailable,
    BodyStored,
}
fn deferred_body_pipeline_completion_stage(
    input: &DeferredInput,
    tag: reducer::EventTag,
    round: reducer::Round,
    subject: reducer::Subject,
) -> Option<DeferredBodyPipelineCompletionStage> {
    match &input.event {
        reducer::Event::LocalProposalReady {
            tag: queued_tag,
            manifest,
        } if *queued_tag == tag
            && tag.height() == round.height()
            && tag.view() == round.view()
            && manifest.subject() == subject =>
        {
            Some(DeferredBodyPipelineCompletionStage::LocalProposalReady)
        }
        reducer::Event::BodyAvailable {
            tag: queued_tag,
            round: queued_round,
            subject: queued_subject,
        } if *queued_tag == tag && *queued_round == round && *queued_subject == subject => {
            Some(DeferredBodyPipelineCompletionStage::BodyAvailable)
        }
        reducer::Event::BodyStored {
            tag: queued_tag,
            round: queued_round,
            subject: queued_subject,
        } if *queued_tag == tag && *queued_round == round && *queued_subject == subject => {
            Some(DeferredBodyPipelineCompletionStage::BodyStored)
        }
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::LocalProposalReady { .. }
        | reducer::Event::ProposalReceived { .. }
        | reducer::Event::VoteReceived { .. }
        | reducer::Event::QuorumCertificateReceived { .. }
        | reducer::Event::TimeoutVoteReceived { .. }
        | reducer::Event::TimeoutCertificateReceived { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. }
        | reducer::Event::BodyAvailable { .. }
        | reducer::Event::BodyStored { .. }
        | reducer::Event::ValidationCompleted { .. }
        | reducer::Event::Persisted { .. }
        | reducer::Event::PersistenceFailed { .. }
        | reducer::Event::Signed { .. }
        | reducer::Event::ApplicationCompleted { .. } => None,
    }
}
fn classify_deferred_decided_local_proposal(
    input: &DeferredInput,
    decision_tag: reducer::EventTag,
    decision_round: wire::ConsensusRound,
    decision_subject: wire::BlockSubject,
    decision_commitment: wire::ExecutionCommitment,
) -> Option<DecisionLocalProposalDisposition> {
    let reducer::Event::LocalProposalReady {
        manifest: core_manifest,
        tag,
    } = &input.event
    else {
        return None;
    };
    let Some(BodyPipelineCompletionEvidence::LocalProposalReady {
        manifest,
        durable_receipt,
        validated_receipt,
    }) = input.completion_evidence.as_ref()
    else {
        return (core_manifest.subject()
            == reducer::Subject::new(Hash::new(decision_subject.encode()).into()))
        .then_some(DecisionLocalProposalDisposition::Conflict);
    };
    let disposition = classify_decided_local_proposal(
        *tag,
        manifest,
        durable_receipt,
        validated_receipt,
        decision_tag,
        decision_round,
        decision_subject,
        decision_commitment,
    );
    let core_matches_evidence = core_manifest.subject()
        == reducer::Subject::new(Hash::new(manifest.subject.encode()).into())
        && core_manifest.payload_hash()
            == reducer::Digest::new(*manifest.subject.payload_hash.as_ref())
        && core_manifest.chunk_root() == reducer::Digest::new(*manifest.chunk_root.as_ref())
        && core_manifest.byte_len() == manifest.payload_size_bytes
        && usize::try_from(core_manifest.chunk_count()).ok() == Some(manifest.chunk_hashes.len());
    if !core_matches_evidence {
        return disposition
            .is_some()
            .then_some(DecisionLocalProposalDisposition::Conflict);
    }
    disposition
}
impl DeferredPriority {
    const fn next(self) -> Self {
        match self {
            Self::Completion => Self::Progress,
            Self::Progress => Self::Normal,
            Self::Normal => Self::Completion,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum IngressSemanticKey {
    Proposal {
        round: wire::ConsensusRound,
        proposer: wire::ValidatorIndex,
    },
    Vote {
        round: wire::ConsensusRound,
        phase: wire::GlobalPhase,
        signer: wire::ValidatorIndex,
    },
    TimeoutVote {
        round: wire::ConsensusRound,
        signer: wire::ValidatorIndex,
    },
}
impl IngressSemanticKey {
    fn round(self) -> wire::ConsensusRound {
        match self {
            Self::Proposal { round, .. }
            | Self::Vote { round, .. }
            | Self::TimeoutVote { round, .. } => round,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IngressFingerprint {
    Proposal(Hash),
    Vote(
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    ),
    TimeoutVote(Option<wire::QuorumCertificateRef>),
}
fn ingress_equivocation_identity(
    payload: &wire::ConsensusMessageV2Payload,
) -> Option<(IngressSemanticKey, IngressFingerprint)> {
    match payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => Some((
            IngressSemanticKey::Proposal {
                round: proposal.round,
                proposer: proposal.proposer,
            },
            IngressFingerprint::Proposal(Hash::new(proposal.signature_preimage())),
        )),
        wire::ConsensusMessageV2Payload::Vote(vote) => Some((
            IngressSemanticKey::Vote {
                round: vote.round,
                phase: vote.phase,
                signer: vote.signer,
            },
            IngressFingerprint::Vote(vote.proposal_round, vote.subject, vote.execution_commitment),
        )),
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => Some((
            IngressSemanticKey::TimeoutVote {
                round: vote.round,
                signer: vote.signer,
            },
            IngressFingerprint::TimeoutVote(
                vote.highest_prepare_qc
                    .as_ref()
                    .map(wire::QuorumCertificate::as_ref),
            ),
        )),
        wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => None,
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum IngressEquivocationArtifact {
    Proposal(Arc<wire::Proposal>),
    Vote(Arc<wire::Vote>),
    TimeoutVote(Arc<wire::TimeoutVote>),
}
impl IngressEquivocationArtifact {
    fn from_payload(payload: &wire::ConsensusMessageV2Payload) -> Option<Self> {
        match payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                Some(Self::Proposal(Arc::new(proposal.clone())))
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => Some(Self::Vote(Arc::new(vote.clone()))),
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                Some(Self::TimeoutVote(Arc::new(vote.clone())))
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => None,
        }
    }
    fn conflict_with(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> Result<AdapterEquivocationEvidence, AdapterError> {
        match (self, payload) {
            (Self::Proposal(first), wire::ConsensusMessageV2Payload::Proposal(second)) => Ok(
                AdapterEquivocationEvidence::proposal(first.as_ref().clone(), second.clone()),
            ),
            (Self::Vote(first), wire::ConsensusMessageV2Payload::Vote(second)) => Ok(
                AdapterEquivocationEvidence::vote(first.as_ref().clone(), second.clone()),
            ),
            (Self::TimeoutVote(first), wire::ConsensusMessageV2Payload::TimeoutVote(second)) => Ok(
                AdapterEquivocationEvidence::timeout_vote(first.as_ref().clone(), second.clone()),
            ),
            _ => Err(AdapterError::EquivocationArtifactMismatch),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct IngressEquivocationRecord {
    fingerprint: IngressFingerprint,
    artifact: IngressEquivocationArtifact,
    equivocation_reported: bool,
    capacity_bypass: bool,
    admitted_at: Instant,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IngressDeliveryRecord {
    fingerprint: IngressFingerprint,
    consumer_tag: reducer::EventTag,
    locked_commit_progress: bool,
    locked_reproposal_prepare_progress: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IngressAdmission {
    key: IngressSemanticKey,
    fingerprint: IngressFingerprint,
    consumer_tag: reducer::EventTag,
    inserted_equivocation: bool,
    locked_commit_progress: bool,
    locked_reproposal_prepare_progress: bool,
}
impl AdapterOutcome {
    /// Return whether the reducer applied or deliberately ignored the input.
    pub(crate) const fn disposition(&self) -> reducer::StepDisposition {
        self.disposition
    }
    /// Borrow the effects now safe for asynchronous execution.
    #[cfg(test)]
    pub(crate) fn effects(&self) -> &[AdapterEffect] {
        &self.effects
    }
    /// Consume the outcome and return its asynchronous effects.
    pub(crate) fn into_effects(self) -> Vec<AdapterEffect> {
        self.effects
    }
    /// Exact producer reservation which the serialized runtime must
    /// acknowledge only after installing its replacement owner.
    pub(crate) const fn producer_handoff(&self) -> Option<ProducerContinuationHandoffToken> {
        self.producer_handoff
    }
    /// Actor-global owner retained when this exact input crossed into the
    /// adapter's Busy-deferred queue.
    pub(crate) const fn deferred_admission_ordinal(&self) -> Option<u128> {
        self.deferred_admission_ordinal
    }
    /// Whether Busy backpressure retained no adapter-owned occurrence and the
    /// serialized runtime must keep the exact physical command in its FIFO.
    pub(crate) const fn requires_runtime_retry(&self) -> bool {
        matches!(
            self.disposition,
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        ) && self.deferred_admission_ordinal.is_none()
    }
}
/// Signature aggregation boundary used when the reducer forms a local QC or TC.
pub(crate) trait SignatureAggregator: Send + Sync {
    /// Aggregate the canonical signer-ordered BLS signature shares.
    fn aggregate(&self, signatures: &[&[u8]]) -> Result<Vec<u8>, String>;
}
/// Encode the canonical auxiliary payload placed beside one Commit vote's BLS signature.
pub(in crate::sumeragi) fn encode_kagemusha_commit_vote_seal_share_v1(
    message: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalitySealMessageV1,
    seal: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityValidatorSealV1,
) -> Vec<u8> {
    iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalitySealShareV1 {
        version: iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
        message,
        seal,
    }
    .encode()
}

#[derive(Debug, Default)]
struct BlsNormalSignatureAggregator;
impl SignatureAggregator for BlsNormalSignatureAggregator {
    fn aggregate(&self, signatures: &[&[u8]]) -> Result<Vec<u8>, String> {
        #[cfg(feature = "bls")]
        {
            let mut bls_signatures = Vec::with_capacity(signatures.len());
            let mut kagemusha_shares = Vec::with_capacity(signatures.len());
            let mut saw_raw = false;
            let mut saw_kagemusha = false;
            for signature in signatures {
                match wire::decode_kagemusha_consensus_signature_envelope_v1(signature)
                    .map_err(|error| error.to_string())?
                {
                    Some(parts) => {
                        if parts.kind != wire::KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1 {
                            return Err(
                                "cannot aggregate an Kagemusha CommitQC envelope as a vote share"
                                    .to_owned(),
                            );
                        }
                        saw_kagemusha = true;
                        bls_signatures.push(parts.bls_signature);
                        kagemusha_shares.push(
                            crate::zk::kagemusha_v1_recursion::decode_kagemusha_mint_finality_seal_share_v1(
                                parts.auxiliary_payload,
                            )
                            .map_err(|error| error.to_string())?,
                        );
                    }
                    None => {
                        saw_raw = true;
                        bls_signatures.push(*signature);
                    }
                }
            }
            if saw_raw && saw_kagemusha {
                return Err(
                    "cannot aggregate mixed raw and Kagemusha V1 Commit-vote signatures".to_owned(),
                );
            }
            let aggregate = iroha_crypto::bls_normal_aggregate_signatures(&bls_signatures)
                .map_err(|error| error.to_string())?;
            if !saw_kagemusha {
                return Ok(aggregate);
            }
            let first = kagemusha_shares
                .first()
                .ok_or_else(|| "Kagemusha V1 seal aggregation received no shares".to_owned())?;
            if kagemusha_shares
                .iter()
                .any(|share| share.message != first.message)
            {
                return Err(
                    "Kagemusha V1 Commit-vote seal shares bind different messages".to_owned(),
                );
            }
            let bundle = iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalitySealBundleV1 {
                message: first.message,
                seals: kagemusha_shares
                    .into_iter()
                    .map(|share| share.seal)
                    .collect(),
            };
            bundle.validate().map_err(|error| error.to_string())?;
            wire::encode_kagemusha_consensus_signature_envelope_v1(
                wire::KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1,
                &aggregate,
                &bundle.encode(),
            )
            .map_err(|error| error.to_string())
        }
        #[cfg(not(feature = "bls"))]
        {
            let _ = signatures;
            Err("the iroha_core `bls` feature is required by Sumeragi v2".to_owned())
        }
    }
}
/// Fatal or structurally invalid adapter input.
#[derive(Debug, Error)]
pub(crate) enum AdapterError {
    /// Canonical wire validation rejected an input.
    #[error("invalid Sumeragi v2 wire value: {0}")]
    WireValidation(#[from] wire::ValidationError),
    /// The executable reducer rejected a transition.
    #[error("Sumeragi v2 reducer rejected a transition: {0}")]
    Reducer(#[from] reducer::ReducerError),
    /// Frozen context conversion failed.
    #[error("invalid executable Sumeragi v2 height context: {0}")]
    HeightContext(#[from] reducer::HeightContextError),
    /// Genesis verification was requested for a non-genesis context.
    #[error("Sumeragi v2 genesis context must be height 1 with no parent CommitQC")]
    InvalidGenesisContext,
    /// Snapshot bootstrap verification was requested for a normal genesis/successor context.
    #[error("Sumeragi v2 snapshot bootstrap context must be an anchored post-snapshot height")]
    InvalidSnapshotBootstrapContext,
    /// Successor context is not anchored to the supplied durable parent.
    #[error("Sumeragi v2 height context does not match its durable parent artifact")]
    ParentContextMismatch,
    /// Successor election inputs changed outside a certified epoch boundary or
    /// differ from the finalized next-epoch snapshot.
    #[error("Sumeragi v2 successor context violates the certified epoch transition")]
    EpochTransitionMismatch,
    /// Safety WAL I/O or integrity checking failed.
    #[error(transparent)]
    SafetyWal(#[from] SafetyWalError),
    /// The adjacent serviced-candidate snapshot failed validation or publication.
    #[error("Sumeragi v2 serviced-candidate store failed: {0}")]
    ServicedCandidateStore(String),
    /// Kura finality artifact failed structural validation.
    #[error("invalid Sumeragi v2 Kura finality artifact: {0}")]
    FinalityArtifact(#[from] wire::finality::V2FinalityValidationError),
    /// Kura's typed receipt or artifact differs from the reducer's exact decision.
    #[error("Sumeragi v2 Kura finality receipt does not match the applied reducer decision")]
    DurableCommitMismatch,
    /// Body-store receipt differs from the exact manifest, round, or subject.
    #[error("Sumeragi v2 durable body receipt does not match the reducer work item")]
    DurableBodyMismatch,
    /// The runner attempted to publish a successor before its live pacemaker
    /// clocks crossed the one-shot post-startup boundary.
    #[error("Sumeragi v2 successor activation requires armed live pacemaker clocks")]
    SuccessorClocksNotArmed,
    /// A complete WAL payload could not be decoded.
    #[error("invalid Sumeragi v2 safety WAL payload: {0}")]
    WalDecode(String),
    /// A verified WAL frame identity did not match the reducer persistence identifier.
    #[error(
        "Sumeragi v2 WAL/reducer identity mismatch: frame {frame_sequence}, persistence id {persistence_id}, frame hash {frame_hash:?}"
    )]
    WalFrameIdentityMismatch {
        /// Zero-based file frame sequence.
        frame_sequence: u64,
        /// One-based reducer persistence identifier.
        persistence_id: u64,
        /// Hash of the exact verified complete frame.
        frame_hash: [u8; 32],
    },
    /// A post-fsync continuation did not match the exact WAL record, action,
    /// tag, or live frame identity required for its replay seal.
    #[error("Sumeragi v2 live WAL continuation replay cause is missing or mismatched")]
    LiveWalReplayCauseMismatch,
    /// A signer index was outside the frozen roster.
    #[error("Sumeragi v2 validator index {0} is outside the frozen roster")]
    ValidatorIndexOutOfRange(u32),
    /// A valid TimeoutVote was relayed under a different semantic origin.
    #[error(
        "authenticated Sumeragi v2 TimeoutVote signer index {signer} does not match semantic origin {semantic_origin}"
    )]
    AuthenticatedTimeoutVoteOriginMismatch {
        /// Frozen-roster signer index carried by the authenticated vote.
        signer: u32,
        /// End-to-end authenticated protocol origin retained by fair ingress.
        semantic_origin: PeerId,
    },
    /// A reducer validator token was not present in the adapter mapping.
    #[error("unknown executable Sumeragi v2 validator token {0}")]
    UnknownValidator(reducer::ValidatorId),
    /// A digest could not be expanded to its canonical block subject.
    #[error("unknown executable Sumeragi v2 subject {0}")]
    UnknownSubject(reducer::Subject),
    /// Two different wire subjects produced the same adapter digest.
    #[error("Sumeragi v2 block-subject digest collision")]
    SubjectCollision,
    /// A reducer manifest had no canonical wire representation.
    #[error("missing canonical Sumeragi v2 payload manifest")]
    MissingManifest,
    /// One round and subject were associated with two different manifests.
    #[error("conflicting canonical Sumeragi v2 payload manifests for one round and subject")]
    ConflictingManifest,
    /// A certificate reference could not be expanded to the full canonical QC.
    #[error("missing canonical Sumeragi v2 quorum certificate")]
    MissingCertificate,
    /// No fsynced deterministic execution result exists for a signable vote or QC.
    #[error("missing validated Sumeragi v2 execution commitment")]
    MissingExecutionCommitment,
    /// WAL replay named more validation-marker identities than the reviewed
    /// startup frontier can contain.
    #[error("Sumeragi v2 recovered validation authority exceeded its bounded replay frontier")]
    RecoveredValidationCapacityExceeded,
    /// The startup batch did not carry the reducer's exact current WAL-owned vote.
    #[error("Sumeragi v2 recovered WAL vote does not match its startup sign effect")]
    RecoveredVoteSignMismatch,
    /// More than one startup phase-vote effect claimed the current WAL intent.
    #[error("Sumeragi v2 recovered WAL vote has ambiguous startup sign effects")]
    RecoveredVoteSignAmbiguous,
    /// No latest exact ProposalIntent/TimeoutIntent owned the current control Sign.
    #[error("Sumeragi v2 recovered WAL control intent does not match its startup Sign")]
    RecoveredControlSignMismatch,
    /// The recovered Decision did not own one exact certificate-backed Fetch.
    #[error("Sumeragi v2 recovered Decision Fetch does not match its WAL authority")]
    RecoveredDecisionFetchMismatch,
    /// The interrupted canonical Kura tip did not own the sole recovered Decision Fetch.
    #[error("Sumeragi v2 pending Kura tip does not match its recovered Decision Fetch")]
    RecoveredPendingKuraApplyMismatch,
    /// An ordinal-free lifecycle validation marker could not rejoin its exact
    /// live Decision-owned Apply continuation.
    #[error(
        "Sumeragi v2 released lifecycle validation does not match its live Decision Apply continuation"
    )]
    ReleasedLifecycleValidatedApplyMismatch,
    /// No-clock activation was requested before exact pending-tip completion.
    #[error("Sumeragi v2 pending Kura tip is not ready for no-clock activation")]
    PendingKuraActivationNotReady,
    /// A lifecycle-owned recovered Fetch body did not produce its exact Store successor.
    #[error("Sumeragi v2 recovered Decision Fetch Store successor violated its closed contract")]
    RecoveredDecisionFetchStoreMismatch,
    /// The private recovered-Decision body fast-forward did not emit exactly
    /// Store, Validate, and final Apply under one unchanged Decision owner.
    #[error(
        "Sumeragi v2 recovered Decision Apply fast-forward violated its closed reducer contract"
    )]
    RecoveredDecisionApplyFastForwardMismatch,
    /// A registry-owned lifecycle Decision Apply completion changed its exact durable
    /// body, CommitQC, Kura receipt, finality artifact, or reducer successor.
    #[error("Sumeragi v2 lifecycle Decision Apply completion violated its closed contract")]
    LifecycleDecisionApplyCompletionMismatch,
    /// A lifecycle-owned recovered Sign completion changed its exact worker
    /// request, signature, Proposal payload, reducer fence, or closed successor shape.
    #[error("Sumeragi v2 recovered Sign completion violated its closed contract")]
    RecoveredLifecycleSignCompletionMismatch,
    /// A valid lifecycle-owned recovered Sign completion belongs to an exact
    /// signer fence which authenticated certified progress has superseded.
    #[error("Sumeragi v2 recovered Sign completion was superseded by certified progress")]
    RecoveredLifecycleSignCompletionSuperseded,
    /// Active serialized-runtime mutation debt temporarily excludes recovered
    /// Sign completion preview without invalidating its guarded owner.
    #[error("Sumeragi v2 recovered Sign completion is blocked by transient runtime debt")]
    RecoveredLifecycleSignCompletionRuntimeDebt,
    /// The residual ResumeAfterReplay inventory was not one supported exact effect.
    #[error("Sumeragi v2 recovered startup effect inventory is inconsistent")]
    RecoveredStartupEffectMismatch,
    /// One immutable subject was bound to different execution results.
    #[error("conflicting Sumeragi v2 execution commitments for one immutable subject")]
    ConflictingExecutionCommitment,
    /// A proposal justification was structurally inconsistent.
    #[error("inconsistent Sumeragi v2 proposal justification")]
    InvalidProposalJustification,
    /// BLS aggregation failed for a locally formed certificate.
    #[error("failed to aggregate Sumeragi v2 signatures: {0}")]
    SignatureAggregation(String),
    /// Authenticated ingress rejected a signature, key, or proof of possession.
    #[error("Sumeragi v2 authenticated ingress rejected cryptography: {0}")]
    Cryptography(String),
    /// Proofs of possession were not aligned with the frozen voting roster.
    #[error(
        "Sumeragi v2 proof-of-possession count {actual} does not match roster length {expected}"
    )]
    ProofOfPossessionCount {
        /// Frozen voting-roster length.
        expected: usize,
        /// Supplied proof count.
        actual: usize,
    },
    /// A transport-only canonical payload was incorrectly routed to the reducer.
    #[error("Sumeragi v2 transport payload is not a reducer input")]
    TransportPayload,
    /// One semantic equivocation slot retained a different signed artifact class.
    #[error("Sumeragi v2 equivocation artifact does not match its semantic slot")]
    EquivocationArtifactMismatch,
    /// Trusted completion ownership exceeded the bounded deferred lane.
    #[error("Sumeragi v2 deferred completion lane exceeded its bounded capacity")]
    DeferredCompletionCapacityExceeded,
    /// The actor-global deferred admission ordinal cannot advance without
    /// wrapping and potentially aliasing a stale owner.
    #[error("Sumeragi v2 deferred admission ordinal space is exhausted")]
    DeferredAdmissionOrdinalExhausted,
    /// The actor-global deferred ordinal source was poisoned by a failed local
    /// owner and can no longer mint trustworthy capabilities.
    #[error("Sumeragi v2 deferred admission ordinal source is unavailable")]
    DeferredAdmissionOrdinalSourceUnavailable,
    /// Exact deferred service debt could not advance without wrapping.
    #[error("Sumeragi v2 deferred service debt overflowed")]
    DeferredServiceDebtOverflow,
    /// One adapter invocation violated the reviewed reducer/continuation
    /// composition contract. This is an internal source-refinement failure,
    /// never recoverable input backpressure.
    #[error(
        "Sumeragi v2 adapter macro-step exceeded its reviewed shape: initial {initial_effects}/{maximum_initial_effects}, Persist {persist_effects}/1, continuation {continuation_effects}/{maximum_continuation_effects}, flattened maximum {maximum_flattened_effects}, nested Persist {continuation_contains_persist}"
    )]
    AdapterMacroStepBoundExceeded {
        /// Effects emitted by the source reducer transition.
        initial_effects: usize,
        /// Record-specific maximum source-transition effects.
        maximum_initial_effects: usize,
        /// Number of `Persist` effects in the source transition.
        persist_effects: usize,
        /// Effects emitted by the synchronous `Persisted` continuation.
        continuation_effects: usize,
        /// Record-specific maximum continuation effects.
        maximum_continuation_effects: usize,
        /// Record-specific maximum flattened effects.
        maximum_flattened_effects: usize,
        /// Whether the acknowledgement attempted a second persistence hop.
        continuation_contains_persist: bool,
    },
    /// The adapter reported deferred work as serviceable, but the reducer
    /// still rejected that exact transition as Busy. Requeueing here would
    /// create a non-decreasing serialized-runtime spin.
    #[error("Sumeragi v2 deferred service violated its open-fence contract and is fail-closed")]
    DeferredServiceContractViolation,
    /// The selected deferred occurrence did not retain the exact actor source,
    /// semantic projection, or single-use capability.
    #[error("Sumeragi v2 deferred service ownership token is invalid or already consumed")]
    DeferredServiceOwnershipViolation,
    /// The serialized runtime lost, altered, or misattached the fair-ingress
    /// carrier for an authenticated adapter command.
    #[error("Sumeragi v2 authenticated runtime ingress ownership is invalid")]
    RuntimeIngressOwnershipViolation,
    /// The direct lifecycle completion preview emitted any shape other than
    /// one exact `BodyAvailable -> StoreBody` transition.
    #[error("Sumeragi v2 direct certified-body completion violated its reducer contract")]
    DirectCertifiedBodyAvailableContractViolation,
    /// The direct durable-body completion preview emitted any shape other than
    /// one exact `BodyStored -> ValidateBody` transition.
    #[error("Sumeragi v2 direct durable-body completion violated its reducer contract")]
    DirectBodyStoredContractViolation,
    /// The direct successful-validation preview emitted a reducer shape outside
    /// its closed Busy, ignored, no-effect, Apply, or Persist inventory.
    #[error("Sumeragi v2 direct successful validation violated its reducer contract")]
    DirectValidationSucceededContractViolation,
    /// The sealed Ready Validate publication preflight did not reduce a
    /// validation-origin Persist into one exact Sign continuation.
    #[error("Sumeragi v2 Ready Validate publication violated its closed adapter contract")]
    ReadyDurableValidatePublicationContractViolation,
    /// The direct failed-validation preview emitted a reducer shape outside
    /// its closed Busy, ignored, no-effect, or PrepareQC-report inventory.
    #[error("Sumeragi v2 direct failed validation violated its reducer contract")]
    DirectValidationFailedContractViolation,
    /// The process-local reducer-fence generation cannot advance without
    /// aliasing a previously observed external wait.
    #[error("Sumeragi v2 reducer-fence generation space is exhausted")]
    ReducerFenceGenerationExhausted,
    /// The reducer is permanently closed after a durability failure.
    #[error("Sumeragi v2 adapter is fail-closed after a durability failure")]
    FailClosed,
    /// The caller attempted network ingress before recovery completed.
    #[error("Sumeragi v2 network ingress is closed until WAL replay completes")]
    ReplayNotComplete,
}
/// Production wrapper around the sole executable Sumeragi v2 reducer.
pub(crate) struct SumeragiV2Adapter {
    wire_context: wire::HeightContext,
    proofs_of_possession: Vec<Vec<u8>>,
    parent_verification: Option<ParentVerificationContext>,
    reducer: reducer::Reducer,
    wal: SafetyWal,
    serviced_candidate_store: ServicedCandidateStore,
    /// Process-generation coalescing markers. This superset includes durable
    /// terminal retirements restored from the adjacent snapshot and volatile
    /// successful services whose reducer state must be rebuilt after restart.
    serviced_candidates: BTreeMap<ServicedCandidateKey, wire::View>,
    /// Restart-stable subset of `serviced_candidates`.
    ///
    /// Only a drained, terminally discarded lifecycle enters this map.
    /// Ordinary successful proposal/vote/body service can leave quorum or
    /// pipeline state only in memory, so persisting its marker would suppress
    /// the retransmission needed to reconstruct that state after a crash.
    durable_serviced_candidates: BTreeMap<ServicedCandidateKey, wire::View>,
    /// Source-derived bound frozen with the adjacent durable store.
    serviced_candidate_capacity: usize,
    /// Process-local exact producer ownership, including live reservations.
    producer_continuations: BTreeMap<ProducerContinuationAddress, ProducerContinuationRecord>,
    /// Restart-safe producer lifecycle metadata published in the same atomic
    /// snapshot. Active records preserve their exact slot and ordinal, then
    /// reopen as `Reserved`; only terminal records suppress replay.
    durable_producer_continuations:
        BTreeMap<ProducerContinuationAddress, ProducerContinuationRecord>,
    /// Active records restored before this process reconstructed their exact
    /// runtime owner. The first matching retry must reuse the persisted causal
    /// key and first-admission ordinal; this set is only the process-local
    /// unclaimed marker and never authorizes identity replacement.
    restored_dormant_producer_continuations: BTreeSet<ProducerContinuationAddress>,
    /// Largest validated admission ordinal present in the snapshot as opened.
    ///
    /// Reclamation may immediately remove an older terminal record, so the
    /// runner must seed its actor-global source from this immutable opening
    /// watermark rather than recomputing it from the post-replay table.
    restored_producer_continuation_ordinal_high_watermark: Option<u128>,
    /// Number of bounded lifecycle slots frozen from runtime capacity geometry.
    producer_continuation_lifecycle_capacity: u64,
    /// Runtime-selected lifecycle being serviced by the next adapter step.
    selected_producer_lifecycle: Option<SelectedProducerLifecycle>,
    /// Busy-deferred adapter ordinal to its complete speculative reservation.
    deferred_producer_continuations: BTreeMap<u128, ProducerReservationToken>,
    /// Exact reservations returned across the runtime ownership cut but not
    /// yet acknowledged by a concrete successor or durable terminal.
    pending_producer_handoffs: BTreeMap<ProducerContinuationAddress, PendingProducerHandoff>,
    serviced_candidates_decision_reclaimed: bool,
    registry: WireRegistry,
    fingerprints: AdapterFingerprints,
    aggregator: Box<dyn SignatureAggregator>,
    active_subject: Option<(reducer::Round, reducer::Subject)>,
    pending_persistence_id: Option<u64>,
    /// Exact live ProposalIntent frame awaiting its positional Sign handoff.
    ///
    /// The sealed continuation retains the WAL-derived standalone owner. The
    /// earlier local-body owner rejoins only at lifecycle admission and never
    /// replaces this pending binding.
    pending_live_proposal_intent_sign: Option<Box<LiveProposalIntentWalSignHandoffV1>>,
    /// Source-only replay seal for the exact live Decision WAL frame whose
    /// Apply child waits for a durable Validate body frame.
    pending_live_decision_apply: Option<SealedLiveWalPersistedEffectV1>,
    ingress_equivocations: BTreeMap<IngressSemanticKey, IngressEquivocationRecord>,
    ingress_deliveries: BTreeMap<IngressSemanticKey, IngressDeliveryRecord>,
    deferred_completions: VecDeque<DeferredInput>,
    deferred_progress_inputs: VecDeque<DeferredInput>,
    deferred_inputs: VecDeque<DeferredInput>,
    deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    next_deferred_priority: DeferredPriority,
    ignore_counts: BTreeMap<reducer::IgnoreReason, u64>,
    last_progress: Option<(
        reducer::Generation,
        reducer::Round,
        wire::SumeragiV2ProgressTransition,
    )>,
    /// Process-local generation of the exact persistence/signature/replay
    /// fence projection. Lifecycle `Busy` outcomes wait on this monotone value
    /// instead of re-entering an adapter-owned FIFO.
    reducer_fence_generation: u64,
    replay_complete: bool,
    /// Whether reducer transitions may publish current-height global status;
    /// recovered startup keeps this closed until lifecycle activation.
    status_publication_enabled: bool,
    #[cfg(test)]
    status_publication_attempts: usize,
    fail_closed: bool,
}
enum SafetyWalOpenTarget<'kura> {
    Kura {
        kura: &'kura Kura,
        authority: KuraSafetyWalDirectoryAuthority,
    },
    #[cfg(test)]
    FixturePath(PathBuf),
}
fn commit_qc_status(
    certificate: &wire::QuorumCertificate,
    context: &wire::HeightContext,
) -> Result<wire::SumeragiV2CommitQcStatus, AdapterError> {
    if certificate.phase != wire::GlobalPhase::Commit
        || certificate.round.context_id != context.id()
        || certificate.round.height != context.height
    {
        return Err(AdapterError::DurableCommitMismatch);
    }
    certificate.validate(context)?;
    let signer_count = u32::try_from(certificate.signers.len())
        .map_err(|_| wire::ValidationError::TooManySigners)?;
    let signed_power = u64::from(signer_count);
    let validator_count =
        u32::try_from(context.roster.len()).map_err(|_| wire::ValidationError::RosterTooLarge)?;
    Ok(wire::SumeragiV2CommitQcStatus {
        certificate: certificate.as_ref(),
        validator_count,
        signer_count,
        min_signers: context.quorum.min_signers,
        signed_power,
        total_power: context.quorum.total_power,
    })
}
impl SumeragiV2Adapter {
    /// Return whether the exact live Decision WAL source still awaits its
    /// Validate-to-Apply body-frame join. This borrows the affine seal only;
    /// lifecycle publication remains its sole consuming path.
    pub(crate) fn has_exact_pending_live_decision_apply(
        &self,
        tag: reducer::EventTag,
        decision_round: wire::ConsensusRound,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        execution_commitment: wire::ExecutionCommitment,
    ) -> bool {
        self.pending_live_decision_apply
            .as_ref()
            .is_some_and(|sealed| {
                sealed.exactly_binds_pending_apply_decision(
                    tag,
                    decision_round,
                    proposal_round,
                    subject,
                    execution_commitment,
                )
            })
    }

    /// Open the safety WAL, replay every complete frame, and resume durable work.
    ///
    /// Network ingress is never exposed before replay has completed.  The
    /// returned startup effects may re-sign an already durable intent or fetch
    /// and apply an already durable decision.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            deferred_admission_ordinals,
        )
    }
    /// Open with validated ownership geometry and configured queue capacity.
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn open_with_capacity_geometry(
        kura: &Kura,
        wal_authority: KuraSafetyWalDirectoryAuthority,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        capacity_geometry: ServicedCandidateCapacityGeometry,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication_with_capacity(
            SafetyWalOpenTarget::Kura {
                kura,
                authority: wal_authority,
            },
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            true,
            capacity_geometry,
            deferred_admission_ordinals,
        )
    }
    /// Open behind the WAL-recovery seal without publishing or exposing replay.
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn open_recovered_startup_with_capacity_geometry(
        kura: &Kura,
        wal_authority: KuraSafetyWalDirectoryAuthority,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        capacity_geometry: ServicedCandidateCapacityGeometry,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<RecoveredAdapterStartup, AdapterError> {
        let (adapter, effects) = Self::open_with_aggregator_and_publication_with_capacity(
            SafetyWalOpenTarget::Kura {
                kura,
                authority: wal_authority,
            },
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            false,
            capacity_geometry,
            deferred_admission_ordinals,
        )?;
        Ok(RecoveredAdapterStartup { adapter, effects })
    }
    /// Open and replay the adapter without publishing its initial reducer
    /// status.
    ///
    /// The serialized runner uses this only while a finalized predecessor owns
    /// a `Running` successor handoff. It must publish a status snapshot after
    /// every remaining startup constructor succeeds, live clocks are armed,
    /// and authenticated ingress is open. All ordinary callers use [`Self::open`].
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_deferred_status(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            false,
            deferred_admission_ordinals,
        )
    }
    /// Open with deferred status publication and the validated queue geometry.
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn open_deferred_status_with_capacity_geometry(
        kura: &Kura,
        wal_authority: KuraSafetyWalDirectoryAuthority,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        capacity_geometry: ServicedCandidateCapacityGeometry,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication_with_capacity(
            SafetyWalOpenTarget::Kura {
                kura,
                authority: wal_authority,
            },
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            false,
            capacity_geometry,
            deferred_admission_ordinals,
        )
    }
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn open_with_aggregator(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        aggregator: Box<dyn SignatureAggregator>,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            aggregator,
            true,
            deferred_admission_ordinals,
        )
    }
    /// Test constructor for the same sealed recovery startup cut with a custom aggregator.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn open_recovered_startup_with_aggregator(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        aggregator: Box<dyn SignatureAggregator>,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<RecoveredAdapterStartup, AdapterError> {
        let (adapter, effects) = Self::open_with_aggregator_and_publication(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            aggregator,
            false,
            deferred_admission_ordinals,
        )?;
        Ok(RecoveredAdapterStartup { adapter, effects })
    }
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn open_with_aggregator_and_publication(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        aggregator: Box<dyn SignatureAggregator>,
        publish_initial_status: bool,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication_with_capacity(
            SafetyWalOpenTarget::FixturePath(wal_path.into()),
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            aggregator,
            publish_initial_status,
            DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY,
            deferred_admission_ordinals,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn open_with_aggregator_and_publication_with_capacity(
        wal_target: SafetyWalOpenTarget<'_>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        aggregator: Box<dyn SignatureAggregator>,
        publish_initial_status: bool,
        capacity_geometry: ServicedCandidateCapacityGeometry,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        let VerifiedHeightContext {
            context: wire_context,
            proofs_of_possession,
            parent_verification,
        } = verified_context;
        let mut registry = WireRegistry::new(&wire_context)?;
        let context = registry.core_context(&wire_context)?;
        let local_validator = local_validator
            .map(|index| registry.validator_id(index))
            .transpose()?;
        let network_id = *wire_context.network_id.as_bytes();
        let wal_identity = reducer::WalFileIdentity::new(
            wire::PROTOCOL_VERSION,
            network_id,
            context.id(),
            context.height(),
            consensus_key_hash,
        );
        let serviced_candidate_owner: [u8; 32] = fingerprints.node.into();
        let candidate_lifecycle_capacity =
            candidate_lifecycle_capacity(wire_context.roster.len(), capacity_geometry);
        let serviced_candidate_capacity = candidate_lifecycle_capacity
            .checked_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
            .ok_or_else(|| {
                AdapterError::ServicedCandidateStore(
                    "serviced-candidate lifecycle-stage capacity overflowed".to_owned(),
                )
            })?;
        let producer_continuation_lifecycle_capacity = u64::try_from(candidate_lifecycle_capacity)
            .map_err(|_| {
                AdapterError::ServicedCandidateStore(
                    "producer-continuation lifecycle capacity is not representable".to_owned(),
                )
            })?;
        let (wal_path, wal) = match wal_target {
            SafetyWalOpenTarget::Kura { kura, authority } => {
                let wal_name = format!("{:020}.wal", wire_context.height);
                let wal_path = kura.sumeragi_v2_storage_root().join("wal").join(&wal_name);
                let wal =
                    SafetyWal::open_with_kura_authority(kura, authority, wal_name, wal_identity)?;
                (wal_path, wal)
            }
            #[cfg(test)]
            SafetyWalOpenTarget::FixturePath(wal_path) => {
                let wal = SafetyWal::open(wal_path.clone(), wal_identity)?;
                (wal_path, wal)
            }
        };
        let serviced_candidate_storage = wal.mint_serviced_candidate_store_authority(&wal_path)?;
        let (serviced_candidate_store, restored_serviced_candidates) =
            ServicedCandidateStore::open_with_safety_wal_authority(
                serviced_candidate_storage,
                wire_context.id(),
                wire_context.height,
                serviced_candidate_owner,
                candidate_lifecycle_capacity,
            )
            .map_err(AdapterError::ServicedCandidateStore)?;
        let entries = wal
            .recovered_records()
            .iter()
            .map(|record| {
                registry.decode_wal_entry(
                    record,
                    parent_verification.as_ref(),
                    &proofs_of_possession,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let reducer = reducer::Reducer::recover(context, local_validator, generation, entries)?;
        if let Some(decision) = reducer.durable_state().decision() {
            // A stable certificate reference intentionally excludes the exact
            // signer quorum and aggregate. Reconstruct from the durable core
            // value rather than consulting the reference-resolution cache: a
            // later WAL record may carry another valid certificate for the
            // same reference, but it cannot authenticate the exact Decision
            // which replay retained.
            let certificate = registry.qc_to_wire(decision, aggregator.as_ref())?;
            // WAL framing detects torn or accidentally corrupted bytes, but it is not an
            // authority proof. Reauthenticate the exact replayed CommitQC before the reducer may
            // emit its recovery Apply effect. This also rejects a locally rewritten, perfectly
            // checksummed WAL whose QC was never signed by the frozen quorum.
            verify_quorum_certificate(&wire_context, &certificate, &proofs_of_possession)?;
        }
        // Recovery must expose the body/application pipeline in its very first
        // status snapshot. A durable decision owns the pipeline in preference
        // to the (possibly still retained) Prepare lock; otherwise the lock is
        // the active body which must remain recoverable while lifecycle views
        // advance before the exact body is re-proposed in a later round.
        let active_subject = reducer
            .durable_state()
            .decision()
            .or_else(|| reducer.durable_state().locked())
            .map(|certificate| (certificate.proposal_round(), certificate.subject()));
        let restored_records = restored_serviced_candidates.records;
        let restored_producer_continuations = restored_serviced_candidates.producer_continuations;
        let restored_dormant_producer_continuations = restored_producer_continuations
            .iter()
            .filter_map(|(address, record)| {
                (record.status() != ProducerContinuationStatus::Terminal).then_some(*address)
            })
            .collect();
        let restored_producer_continuation_ordinal_high_watermark = restored_producer_continuations
            .values()
            .map(|record| record.identity().admission_ordinal())
            .max();
        let mut adapter = Self {
            wire_context,
            proofs_of_possession,
            parent_verification,
            reducer,
            wal,
            serviced_candidate_store,
            serviced_candidates: restored_records.clone(),
            durable_serviced_candidates: restored_records,
            serviced_candidate_capacity,
            producer_continuations: restored_producer_continuations.clone(),
            durable_producer_continuations: restored_producer_continuations,
            restored_dormant_producer_continuations,
            restored_producer_continuation_ordinal_high_watermark,
            producer_continuation_lifecycle_capacity,
            selected_producer_lifecycle: None,
            deferred_producer_continuations: BTreeMap::new(),
            pending_producer_handoffs: BTreeMap::new(),
            serviced_candidates_decision_reclaimed: restored_serviced_candidates.decision_reclaimed,
            registry,
            fingerprints,
            aggregator,
            active_subject,
            pending_persistence_id: None,
            pending_live_proposal_intent_sign: None,
            pending_live_decision_apply: None,
            ingress_equivocations: BTreeMap::new(),
            ingress_deliveries: BTreeMap::new(),
            deferred_completions: VecDeque::new(),
            deferred_progress_inputs: VecDeque::new(),
            deferred_inputs: VecDeque::new(),
            deferred_admission_ordinals,
            next_deferred_priority: DeferredPriority::Completion,
            ignore_counts: BTreeMap::new(),
            last_progress: None,
            reducer_fence_generation: 0,
            replay_complete: false,
            status_publication_enabled: publish_initial_status,
            #[cfg(test)]
            status_publication_attempts: 0,
            fail_closed: false,
        };
        adapter.reconcile_restored_reserved_producer_frontier()?;
        adapter.reclaim_serviced_candidates()?;
        let replay_tag = adapter.reducer.current_tag();
        let replay_event = reducer::Event::ResumeAfterReplay { tag: replay_tag };
        let replay = adapter.step_reducer(replay_event.clone())?;
        adapter.record_reducer_outcome(&replay_event, replay.disposition(), replay.effects());
        let startup = replay.into_effects();
        let startup = adapter.drive_effects(startup)?;
        adapter.replay_complete = true;
        adapter.advance_reducer_fence_generation()?;
        if publish_initial_status {
            adapter.publish_status()?;
        }
        Ok((adapter, startup))
    }
    /// Return the tag which must accompany a new asynchronous operation.
    pub(crate) const fn current_tag(&self) -> reducer::EventTag {
        self.reducer.current_tag()
    }
    /// Consume the exact post-fsync ProposalIntent Sign sidecar for one batch.
    ///
    /// Recovered startup Proposal signs have no live append sidecar and return
    /// `None`. Once a live sidecar exists, any positional or semantic mismatch
    /// is restart-only: the adapter keeps the seal and fails closed rather than
    /// allowing another reducer turn to overtake its WAL-owned Sign.
    pub(in crate::sumeragi) fn take_live_proposal_intent_wal_sign(
        &mut self,
        effects: &[AdapterEffect],
    ) -> Result<Option<LiveProposalIntentWalSignHandoffV1>, AdapterError> {
        let Some(pending) = self.pending_live_proposal_intent_sign.as_ref() else {
            return Ok(None);
        };
        if !pending.exactly_matches_effects(effects) {
            self.fail_closed = true;
            return Err(AdapterError::LiveWalReplayCauseMismatch);
        }
        Ok(self
            .pending_live_proposal_intent_sign
            .take()
            .map(|pending| *pending))
    }
    /// Borrow the immutable wire context authenticated by this height's
    /// reducer adapter.
    pub(crate) const fn wire_context(&self) -> &wire::HeightContext {
        &self.wire_context
    }
    /// Return the reducer body state for one wire identity in seam tests.
    #[cfg(test)]
    pub(crate) fn body_state_for_test(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> reducer::BodyState {
        let round = reducer::Round::new(round.height, round.view);
        let subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        self.reducer.body_state(round, subject)
    }
    /// Return whether the live wire registry contains this exact manifest key.
    #[cfg(all(test, feature = "bls"))]
    pub(crate) fn has_registered_manifest_for_test(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        if round.context_id != self.wire_context.id() {
            return false;
        }
        let round = reducer::Round::new(round.height, round.view);
        let subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        self.registry.manifests.contains_key(&(round, subject))
    }
    /// Actor-global ordinal source shared with every replacement height
    /// adapter owned by this runtime actor.
    pub(crate) const fn deferred_admission_ordinal_source(
        &self,
    ) -> &DeferredAdmissionOrdinalSource {
        &self.deferred_admission_ordinals
    }
    /// Largest producer lifecycle ordinal validated while opening this height.
    ///
    /// This remains the opening value even if strict-view or Decision
    /// reclamation removes the corresponding tombstone before the serialized
    /// runtime is constructed.
    pub(crate) const fn restored_producer_continuation_ordinal_high_watermark(
        &self,
    ) -> Option<u128> {
        self.restored_producer_continuation_ordinal_high_watermark
    }
    /// Resolve one restart-dormant deterministic runtime root by its exact
    /// persisted causal key. Every stage in one lifecycle must agree on the
    /// immutable first-admission ordinal.
    pub(crate) fn dormant_producer_lifecycle(
        &self,
        causal_lifecycle_key: &Hash,
    ) -> super::v2_runtime::RuntimeDormantProducerLifecycle {
        use super::v2_runtime::RuntimeDormantProducerLifecycle as Dormant;
        let mut admission_ordinal = None;
        for address in &self.restored_dormant_producer_continuations {
            let Some(record) = self.producer_continuations.get(address) else {
                return Dormant::Conflict;
            };
            if record.identity().causal_lifecycle_key() != *causal_lifecycle_key {
                continue;
            }
            if record.status() != ProducerContinuationStatus::Reserved
                || self.durable_producer_continuations.get(address) != Some(record)
            {
                return Dormant::Conflict;
            }
            let candidate = record.identity().admission_ordinal();
            match admission_ordinal {
                Some(existing) if existing != candidate => return Dormant::Conflict,
                Some(_) => {}
                None => admission_ordinal = Some(candidate),
            }
        }
        admission_ordinal.map_or(Dormant::Absent, |admission_ordinal| Dormant::Exact {
            admission_ordinal,
        })
    }
    /// Retire one exact restart-dormant producer before its volatile runtime
    /// replacement is released without reducer service.
    ///
    /// Stage-7 `BodyAvailable` reconstruction deliberately reacquires a fresh
    /// physical FIFO slot after restart while retaining this durable logical
    /// producer reservation. A certified view change, Decision, or durable
    /// signing preemption can make that fetch unnecessary before the
    /// completion reaches the reducer. Persist the exact durable removal
    /// first so a crash cannot reopen the already-retired stage again.
    pub(crate) fn retire_restored_producer_continuation(
        &mut self,
        causal_lifecycle_key: Hash,
        admission_ordinal: u128,
        producer_stage: u8,
    ) -> Result<bool, AdapterError> {
        self.ensure_ingress()?;
        if admission_ordinal == 0
            || producer_stage != ServicedCandidateStage::BodyAvailable as u8
            || self.selected_producer_lifecycle.is_some()
        {
            return Err(self.fail_serviced_candidate_store(
                "restored producer retirement carried an invalid stage, ordinal, or active selection"
                    .to_owned(),
            ));
        }
        let matches = self
            .producer_continuations
            .iter()
            .filter_map(|(address, record)| {
                let identity = record.identity();
                (identity.causal_lifecycle_key() == causal_lifecycle_key
                    && identity.admission_ordinal() == admission_ordinal
                    && identity.stage() == producer_stage)
                    .then_some((*address, record.clone()))
            })
            .collect::<Vec<_>>();
        let [(address, record)] = matches.as_slice() else {
            return match matches.len() {
                0 => Ok(false),
                _ => Err(self.fail_serviced_candidate_store(
                    "restored producer retirement matched multiple bounded addresses".to_owned(),
                )),
            };
        };
        self.persist_restored_body_producer_retirement(*address, record)?;
        Ok(true)
    }
    /// Retire the exact restart-dormant stage-7 parent named by a terminal
    /// reconstructed-body fetch.
    ///
    /// A restart intentionally gives `FetchBody` a fresh physical runtime
    /// lifecycle, so that volatile owner cannot carry the old producer key.
    /// Recover the authority from the adapter's persisted route-neutral body
    /// coordinates instead. A supplied manifest must reproduce the complete
    /// serviced-candidate identity; a manifest-less fetch is accepted only
    /// when those coordinates select one unique dormant stage-7 record.
    pub(crate) fn retire_restored_body_fetch_parent(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        manifest: Option<&wire::PayloadManifest>,
    ) -> Result<bool, AdapterError> {
        self.ensure_ingress()?;
        if self.selected_producer_lifecycle.is_some()
            || round.context_id != self.wire_context.id()
            || round.height != self.wire_context.height
        {
            return Err(self.fail_serviced_candidate_store(
                "restored body-fetch retirement crossed its durable height geometry".to_owned(),
            ));
        }
        let core_round = reducer::Round::new(round.height, round.view);
        let core_subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        let expected_candidate = manifest
            .map(|manifest| {
                manifest.validate(&self.wire_context)?;
                if manifest.round != round || manifest.subject != subject {
                    return Err(AdapterError::DurableBodyMismatch);
                }
                let event = reducer::Event::BodyAvailable {
                    tag: self.reducer.current_tag(),
                    round: core_round,
                    subject: core_subject,
                };
                let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
                    manifest: manifest.clone(),
                };
                self.serviced_candidate(&event, DeferredPriority::Completion, Some(&evidence), None)
                    .map(|(candidate, _, _)| candidate)
                    .ok_or_else(|| {
                        self.fail_serviced_candidate_store(
                            "restored body-fetch parent had no serviced-candidate stage".to_owned(),
                        )
                    })
            })
            .transpose()?;
        let coordinate_matches = self
            .producer_continuations
            .iter()
            .filter_map(|(address, record)| {
                let identity = record.identity();
                let candidate = identity.candidate();
                (identity.stage() == ServicedCandidateStage::BodyAvailable as u8
                    && candidate.source_view() == round.view
                    && candidate.target() == Some(*core_subject.as_bytes())
                    && record.status() == ProducerContinuationStatus::Reserved
                    && record.source_class() == ProducerContinuationSourceClass::VolatileBody
                    && self.durable_producer_continuations.get(address) == Some(record)
                    && self
                        .restored_dormant_producer_continuations
                        .contains(address))
                .then_some((*address, record.clone()))
            })
            .collect::<Vec<_>>();
        let [(address, record)] = coordinate_matches.as_slice() else {
            return match coordinate_matches.len() {
                0 => Ok(false),
                _ => Err(self.fail_serviced_candidate_store(
                    "restored body-fetch coordinates matched multiple dormant producers".to_owned(),
                )),
            };
        };
        if expected_candidate.is_some_and(|expected| record.identity().candidate() != expected) {
            return Err(self.fail_serviced_candidate_store(
                "restored body-fetch manifest changed its persisted producer identity".to_owned(),
            ));
        }
        self.persist_restored_body_producer_retirement(*address, record)?;
        Ok(true)
    }
    /// Persist the removal of one preflighted restart-dormant body producer.
    fn persist_restored_body_producer_retirement(
        &mut self,
        address: ProducerContinuationAddress,
        record: &ProducerContinuationRecord,
    ) -> Result<(), AdapterError> {
        if record.status() != ProducerContinuationStatus::Reserved
            || record.source_class() != ProducerContinuationSourceClass::VolatileBody
            || record.identity().address() != address
            || record.identity().stage() != ServicedCandidateStage::BodyAvailable as u8
            || self.durable_producer_continuations.get(&address) != Some(record)
            || !self
                .restored_dormant_producer_continuations
                .contains(&address)
            || self
                .deferred_producer_continuations
                .values()
                .any(|reservation| reservation.address == address)
            || self.pending_producer_handoffs.contains_key(&address)
        {
            return Err(self.fail_serviced_candidate_store(
                "restored producer retirement did not own one exact dormant durable record"
                    .to_owned(),
            ));
        }
        let process_previous = self
            .producer_continuations
            .remove(&address)
            .expect("matched process producer remains present");
        let durable_previous = self
            .durable_producer_continuations
            .remove(&address)
            .expect("matched durable producer remains present");
        let dormant_removed = self
            .restored_dormant_producer_continuations
            .remove(&address);
        debug_assert!(dormant_removed);
        if let Err(reason) = self
            .serviced_candidate_store
            .persist_with_producer_continuations(
                &self.durable_serviced_candidates,
                &self.durable_producer_continuations,
                self.serviced_candidates_decision_reclaimed,
            )
        {
            self.producer_continuations
                .insert(address, process_previous);
            self.durable_producer_continuations
                .insert(address, durable_previous);
            if dormant_removed {
                self.restored_dormant_producer_continuations.insert(address);
            }
            return Err(self.fail_serviced_candidate_store(reason));
        }
        Ok(())
    }
    /// Return the restart-dormant Local stages which already reserve a
    /// completion-FIFO position.
    ///
    /// Timeout replay remains a non-FIFO clock root. Authenticated transport
    /// and pre-store reconstructed-body work retain their separate physical
    /// owners, so neither class consumes a local FIFO reservation here.
    pub(crate) fn dormant_local_fifo_reservations(
        &self,
    ) -> Result<Vec<super::v2_runtime::RuntimeDormantLocalFifoReservation>, String> {
        let expected_dormant = self
            .producer_continuations
            .iter()
            .filter_map(|(address, record)| {
                (record.status() != ProducerContinuationStatus::Terminal).then_some(*address)
            })
            .collect::<BTreeSet<_>>();
        if expected_dormant != self.restored_dormant_producer_continuations {
            return Err(
                "restart-dormant producer index disagreed with active snapshot records".to_owned(),
            );
        }
        let mut lifecycle_ordinals = BTreeMap::<Hash, u128>::new();
        let mut reservations = BTreeSet::new();
        for address in &self.restored_dormant_producer_continuations {
            let record = self
                .producer_continuations
                .get(address)
                .ok_or_else(|| "restart-dormant producer record was missing".to_owned())?;
            if record.status() != ProducerContinuationStatus::Reserved
                || record.identity().address() != *address
                || self.durable_producer_continuations.get(address) != Some(record)
            {
                return Err(
                    "restart-dormant producer record was not exact durable Reserved metadata"
                        .to_owned(),
                );
            }
            let identity = record.identity();
            let lifecycle_key = identity.causal_lifecycle_key();
            let admission_ordinal = identity.admission_ordinal();
            match lifecycle_ordinals.insert(lifecycle_key, admission_ordinal) {
                Some(existing) if existing != admission_ordinal => {
                    return Err(
                        "restart-dormant producer lifecycle changed its immutable ordinal"
                            .to_owned(),
                    );
                }
                Some(_) | None => {}
            }
            let stage = ServicedCandidateStage::from_code(identity.stage()).ok_or_else(|| {
                "restart-dormant producer carried an unknown service stage".to_owned()
            })?;
            let expected_source = producer_parent_replay_source_for_stage(stage);
            let source_exact = matches!(
                (expected_source, record.source_class()),
                (
                    ProducerParentReplaySource::ConditionalResponsiveTransport,
                    ProducerContinuationSourceClass::ConditionalTransport
                ) | (
                    ProducerParentReplaySource::VolatileBodyReconstruction,
                    ProducerContinuationSourceClass::VolatileBody
                ) | (
                    ProducerParentReplaySource::DurableBodyPipeline
                        | ProducerParentReplaySource::SafetyWal
                        | ProducerParentReplaySource::DurableDecision,
                    ProducerContinuationSourceClass::Local
                )
            );
            if !source_exact {
                return Err("restart-dormant producer changed its physical replay class".to_owned());
            }
            if matches!(
                stage,
                ServicedCandidateStage::LocalProposalReady
                    | ServicedCandidateStage::BodyStored
                    | ServicedCandidateStage::ValidationCompleted
                    | ServicedCandidateStage::ApplicationCompleted
            ) && !reservations.insert(
                super::v2_runtime::RuntimeDormantLocalFifoReservation::completion(
                    lifecycle_key,
                    admission_ordinal,
                    identity.stage(),
                ),
            ) {
                return Err(
                    "restart-dormant Local producer duplicated one FIFO reservation".to_owned(),
                );
            }
        }
        Ok(reservations.into_iter().collect())
    }
    /// Snapshot the exact reducer-owned facts which constrain local proposal
    /// construction. Proposal justification remains internal to the reducer.
    pub(crate) fn local_proposal_directive(&self) -> Result<LocalProposalDirective, AdapterError> {
        let durable = self.reducer.durable_state();
        let view = durable.current_view();
        let leader = self
            .registry
            .validator_index(self.reducer.context().leader(view))?;
        let locked = durable.locked();
        let locked_round =
            locked.map(|certificate| self.registry.round_to_wire(certificate.round()));
        let locked_subject = locked
            .map(|certificate| self.registry.subject(certificate.subject()))
            .transpose()?;
        let decided_subject = durable
            .decision()
            .map(|certificate| self.registry.subject(certificate.subject()))
            .transpose()?;
        Ok(LocalProposalDirective {
            tag: self.reducer.current_tag(),
            leader,
            locked_round,
            locked_subject,
            decided_subject,
        })
    }
    /// Return whether replay-authenticated safety authority closes local
    /// production for the reducer's exact current round.
    ///
    /// This is a defense-in-depth admission fact beneath runner-local attempt
    /// bookkeeping. A recovered phase vote, timeout, Decision, or empty
    /// residual effect set must not reopen the active-view producer after the
    /// safety WAL has fixed or closed that round.
    pub(crate) fn durable_current_round_local_proposal_is_closed(&self) -> bool {
        let tag = self.reducer.current_tag();
        let round = reducer::Round::new(tag.height(), tag.view());
        let durable = self.reducer.durable_state();
        durable.proposal_intent(round).is_some()
            || durable.timeout_intent(round).is_some()
            || durable.decision().is_some()
    }
    /// Mint the bounded validation-marker frontier authorized by WAL replay.
    ///
    /// Marker files from superseded views remain checksummed local data, not
    /// restart authority. Only the durable highest Prepare, active durable
    /// lock/decision, and exact body identities referenced by the first replay
    /// batch can be rebound before live ingress opens.
    pub(crate) fn recovered_validation_authority(
        &self,
        startup_effects: &[AdapterEffect],
    ) -> Result<RecoveredValidationAuthority, AdapterError> {
        self.ensure_ingress()?;
        if startup_effects.len() > MAX_ADAPTER_EFFECTS_PER_MACRO_STEP {
            return Err(AdapterError::RecoveredValidationCapacityExceeded);
        }
        let context_id = self.wire_context.id();
        let height = self.wire_context.height;
        let mut keys = BTreeSet::new();
        let mut retain = |round: wire::ConsensusRound,
                          subject: wire::BlockSubject|
         -> Result<(), AdapterError> {
            if round.context_id != context_id || round.height != height {
                return Err(AdapterError::DurableBodyMismatch);
            }
            keys.insert((round, subject));
            Ok(())
        };
        if let Some(certificate) = self.replayed_highest_prepare_certificate_ref()? {
            retain(certificate.proposal_round, certificate.subject)?;
        }
        if let Some(certificate) = self.reducer.durable_state().locked() {
            retain(
                self.registry.round_to_wire(certificate.proposal_round()),
                self.registry.subject(certificate.subject())?,
            )?;
        }
        if let Some((_, proposal_round, subject, _)) = self.replayed_decision_key()? {
            retain(proposal_round, subject)?;
        }
        for effect in startup_effects {
            match effect {
                AdapterEffect::Sign { request, .. } => {
                    if let Some((round, subject)) = request.body_round().zip(request.subject()) {
                        retain(round, subject)?;
                    }
                }
                AdapterEffect::FetchBody { round, subject, .. }
                | AdapterEffect::StoreBody { round, subject, .. }
                | AdapterEffect::ValidateBody { round, subject, .. } => {
                    retain(*round, *subject)?;
                }
                AdapterEffect::Apply {
                    subject,
                    certificate,
                    ..
                } => {
                    retain(certificate.proposal_round, *subject)?;
                }
                AdapterEffect::EnterView {
                    protected_lock: Some(certificate),
                    ..
                } => {
                    retain(certificate.proposal_round, certificate.subject)?;
                }
                AdapterEffect::Broadcast(_)
                | AdapterEffect::EnterView {
                    protected_lock: None,
                    ..
                }
                | AdapterEffect::ReportEquivocation { .. }
                | AdapterEffect::ReportInvalidCertifiedBody { .. } => {}
            }
        }
        if keys.len() > MAX_RECOVERED_VALIDATION_AUTHORITIES {
            return Err(AdapterError::RecoveredValidationCapacityExceeded);
        }
        Ok(RecoveredValidationAuthority {
            context_id,
            height,
            keys,
        })
    }
    /// Reauthenticate one complete recovered WAL frame and seal its exact identity.
    fn authenticate_recovered_wal_frame(
        &self,
        frame: &RecoveredRecord,
    ) -> Result<(RecoveredWalFrameIdentity, WalEnvelopeV2), AdapterError> {
        let mut payload = frame.payload();
        let envelope = WalEnvelopeV2::decode(&mut payload)
            .map_err(|error| AdapterError::WalDecode(error.to_string()))?;
        if !payload.is_empty() {
            return Err(AdapterError::WalDecode(
                "trailing bytes after complete record".to_owned(),
            ));
        }
        if envelope.protocol_version != wire::PROTOCOL_VERSION {
            return Err(AdapterError::WalDecode(format!(
                "unsupported protocol version {}",
                envelope.protocol_version
            )));
        }
        let Some(identity) =
            RecoveredWalFrameIdentity::from_recovered_record(frame, envelope.persistence_id)
        else {
            return Err(AdapterError::WalFrameIdentityMismatch {
                frame_sequence: frame.sequence(),
                persistence_id: envelope.persistence_id,
                frame_hash: frame.frame_hash(),
            });
        };
        verify_wal_record_authority(
            &self.wire_context,
            self.parent_verification.as_ref(),
            &envelope.record,
            &self.proofs_of_possession,
        )?;
        Ok((identity, envelope))
    }
    /// Authenticate terminal WAL continuity independently of the current Sign owner.
    fn authenticate_recovered_wal_frontier(&self) -> Result<(), AdapterError> {
        let durable_last_id = self.reducer.durable_state().last_id().get();
        let Some(frame) = self.wal.recovered_records().last() else {
            return (durable_last_id == 0)
                .then_some(())
                .ok_or(AdapterError::RecoveredStartupEffectMismatch);
        };
        let (_, envelope) = self.authenticate_recovered_wal_frame(frame)?;
        if envelope.persistence_id != durable_last_id {
            return Err(AdapterError::RecoveredStartupEffectMismatch);
        }
        Ok(())
    }
    // RECOVERED_WAL_VOTE_SIGN_MINT_BEGIN
    /// Extract the current startup phase vote from its latest exact WAL owner.
    ///
    /// Terminal WAL continuity is authenticated separately. This mint binds the
    /// reducer's sole `awaiting_signature` and sole residual Sign to the latest
    /// authenticated matching `PrepareIntent` or `LockAndCommit`, so legal
    /// byte-identical repeated intents deterministically select the later frame.
    /// A recovered Commit may carry the current owner tag into a later view only
    /// while the full selected PrepareQC is still the reducer's exact active lock.
    #[cfg_attr(not(test), allow(dead_code))]
    fn authenticate_recovered_wal_vote_sign(
        &self,
        startup_effects: &mut Vec<AdapterEffect>,
    ) -> Result<Option<RecoveredWalVoteSign>, AdapterError> {
        self.ensure_ingress()?;
        if startup_effects.len() > MAX_ADAPTER_EFFECTS_PER_MACRO_STEP {
            return Err(AdapterError::RecoveredVoteSignAmbiguous);
        }
        let Some(reducer::SignableMessage::Vote(awaiting_vote)) = self.reducer.awaiting_signature()
        else {
            if startup_effects.iter().any(|effect| {
                matches!(
                    effect,
                    AdapterEffect::Sign {
                        request: SignRequest::Vote(_),
                        ..
                    }
                )
            }) {
                return Err(AdapterError::RecoveredVoteSignMismatch);
            }
            return Ok(None);
        };
        let vote = self.registry.unsigned_vote_to_wire(*awaiting_vote)?;
        if startup_effects.len() != 1 {
            return Err(AdapterError::RecoveredVoteSignAmbiguous);
        }
        let signer_in_roster =
            usize::try_from(vote.signer).is_ok_and(|index| index < self.wire_context.roster.len());
        let Some(local_validator) = self.reducer.local_validator() else {
            return Err(AdapterError::RecoveredVoteSignMismatch);
        };
        let local_signer = self.registry.validator_index(local_validator)?;
        if !vote.signature.is_empty()
            || vote.round != vote.proposal_round
            || vote.round.context_id != self.wire_context.id()
            || vote.round.height != self.wire_context.height
            || vote.proposal_round.context_id != self.wire_context.id()
            || vote.proposal_round.height != self.wire_context.height
            || vote.execution_commitment.validate().is_err()
            || !signer_in_roster
            || vote.signer != local_signer
        {
            return Err(AdapterError::RecoveredVoteSignMismatch);
        }
        let expected_tag = self.current_tag();
        let tag_is_exact = expected_tag.height() == vote.round.height
            && match vote.phase {
                wire::GlobalPhase::Prepare => expected_tag.view() == vote.round.view,
                wire::GlobalPhase::Commit => expected_tag.view() >= vote.round.view,
            };
        if !tag_is_exact {
            return Err(AdapterError::RecoveredVoteSignMismatch);
        }
        let active_commit_lock = match vote.phase {
            wire::GlobalPhase::Prepare => None,
            wire::GlobalPhase::Commit => {
                let Some(locked) = self.reducer.durable_state().locked() else {
                    return Err(AdapterError::RecoveredVoteSignMismatch);
                };
                if self.reducer.durable_state().commit_intent_for_lock(locked)
                    != Some(*awaiting_vote)
                {
                    return Err(AdapterError::RecoveredVoteSignMismatch);
                }
                Some(locked)
            }
        };
        let mut owner = None;
        for frame in self.wal.recovered_records().iter().rev() {
            let (wal_identity, envelope) = self.authenticate_recovered_wal_frame(frame)?;
            match envelope.record {
                WalRecordV2::PrepareIntent(candidate)
                    if vote.phase == wire::GlobalPhase::Prepare && candidate == vote =>
                {
                    owner = Some((wal_identity, None));
                    break;
                }
                WalRecordV2::LockAndCommit {
                    prepare,
                    vote: candidate,
                } if vote.phase == wire::GlobalPhase::Commit
                    && candidate == vote
                    && prepare.phase == wire::GlobalPhase::Prepare
                    && prepare.round == prepare.proposal_round
                    && prepare.round == vote.round
                    && prepare.proposal_round == vote.proposal_round
                    && prepare.subject == vote.subject
                    && prepare.execution_commitment == vote.execution_commitment
                    && active_commit_lock.is_some_and(|locked| {
                        self.registry.reducer_qc_matches_wire(locked, &prepare)
                    }) =>
                {
                    // The frame authentication above verifies the full PrepareQC.
                    // Retain its complete signer set and aggregate rather than a
                    // stable reference, because it is the active lock lineage.
                    owner = Some((wal_identity, Some(prepare)));
                    break;
                }
                WalRecordV2::ProposalIntent(_)
                | WalRecordV2::PrepareIntent(_)
                | WalRecordV2::ObservePrepare(_)
                | WalRecordV2::LockAndCommit { .. }
                | WalRecordV2::TimeoutIntent(_)
                | WalRecordV2::InstallTimeout(_)
                | WalRecordV2::Decision(_) => {}
            }
        }
        let Some((wal_identity, prepare_certificate)) = owner else {
            return Err(AdapterError::RecoveredVoteSignMismatch);
        };
        let mut vote_effects = startup_effects
            .iter()
            .enumerate()
            .filter_map(|(index, effect)| match effect {
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(startup_vote),
                } => Some((index, *tag, startup_vote)),
                _ => None,
            });
        let Some((effect_index, tag, startup_vote)) = vote_effects.next() else {
            return Err(AdapterError::RecoveredVoteSignMismatch);
        };
        if vote_effects.next().is_some() {
            return Err(AdapterError::RecoveredVoteSignAmbiguous);
        }
        if tag != expected_tag || startup_vote != &vote {
            return Err(AdapterError::RecoveredVoteSignMismatch);
        }
        let Some(replay_evidence) =
            RecoveredWalVoteReplayEvidenceV1::from_sealed_recovered_vote(wal_identity, tag, &vote)
        else {
            return Err(AdapterError::RecoveredVoteSignMismatch);
        };
        let removed = startup_effects.remove(effect_index);
        debug_assert!(matches!(
            removed,
            AdapterEffect::Sign {
                tag: removed_tag,
                request: SignRequest::Vote(ref removed_vote),
            } if removed_tag == tag && removed_vote == &vote
        ));
        Ok(Some(RecoveredWalVoteSign {
            wal_identity,
            replay_evidence,
            tag,
            vote,
            prepare_certificate,
        }))
    }
    // RECOVERED_WAL_VOTE_SIGN_MINT_END
    /// Bind one reducer-produced follow-on Vote Sign to its durable WAL and body.
    ///
    /// The reverse scan deliberately selects the latest exact matching owner.
    /// Authenticated unrelated later records neither redirect nor invalidate the
    /// vote lineage. This helper mints the inert authority consumed by the
    /// atomic combined Ledger/registry publication or exact cold recovery join.
    #[cfg_attr(not(test), allow(dead_code))]
    fn authenticate_recovered_lifecycle_next_vote(
        &self,
        effect: &AdapterEffect,
        validated: &ValidatedBodyReceipt,
        expected_manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    ) -> Result<RecoveredLifecycleNextWalVoteSealV1, AdapterError> {
        self.ensure_ingress()?;
        self.authenticate_recovered_wal_frontier()?;
        let AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(vote),
        } = effect
        else {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        };
        let local_signer = self
            .reducer
            .local_validator()
            .map(|validator| self.registry.validator_index(validator))
            .transpose()?
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let signer_in_roster =
            usize::try_from(vote.signer).is_ok_and(|index| index < self.wire_context.roster.len());
        let tag_is_exact = tag.height() == vote.round.height
            && match vote.phase {
                wire::GlobalPhase::Prepare => tag.view() == vote.round.view,
                wire::GlobalPhase::Commit => tag.view() >= vote.round.view,
            };
        let durable = validated.durable();
        if !vote.signature.is_empty()
            || vote.round != vote.proposal_round
            || vote.round.context_id != self.wire_context.id()
            || vote.round.height != self.wire_context.height
            || vote.execution_commitment.validate().is_err()
            || !signer_in_roster
            || vote.signer != local_signer
            || *tag != self.current_tag()
            || !tag_is_exact
            || durable.context_id() != vote.round.context_id
            || durable.round() != vote.proposal_round
            || durable.subject() != vote.subject
            || expected_manifest_hash.is_some_and(|expected| durable.manifest_hash() != expected)
            || validated.execution_commitment() != vote.execution_commitment
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let active_commit_lock = match vote.phase {
            wire::GlobalPhase::Prepare => None,
            wire::GlobalPhase::Commit => self.reducer.durable_state().locked(),
        };
        if vote.phase == wire::GlobalPhase::Commit && active_commit_lock.is_none() {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let mut owner = None;
        for frame in self.wal.recovered_records().iter().rev() {
            let (wal_identity, envelope) = self.authenticate_recovered_wal_frame(frame)?;
            match envelope.record {
                WalRecordV2::PrepareIntent(candidate)
                    if vote.phase == wire::GlobalPhase::Prepare && candidate == *vote =>
                {
                    owner = Some(wal_identity);
                    break;
                }
                WalRecordV2::LockAndCommit {
                    prepare,
                    vote: candidate,
                } if vote.phase == wire::GlobalPhase::Commit
                    && candidate == *vote
                    && prepare.phase == wire::GlobalPhase::Prepare
                    && prepare.round == prepare.proposal_round
                    && prepare.round == vote.round
                    && prepare.proposal_round == vote.proposal_round
                    && prepare.subject == vote.subject
                    && prepare.execution_commitment == vote.execution_commitment
                    && active_commit_lock.is_some_and(|locked| {
                        self.registry.reducer_qc_matches_wire(locked, &prepare)
                    }) =>
                {
                    owner = Some(wal_identity);
                    break;
                }
                WalRecordV2::ProposalIntent(_)
                | WalRecordV2::PrepareIntent(_)
                | WalRecordV2::ObservePrepare(_)
                | WalRecordV2::LockAndCommit { .. }
                | WalRecordV2::TimeoutIntent(_)
                | WalRecordV2::InstallTimeout(_)
                | WalRecordV2::Decision(_) => {}
            }
        }
        let wal_identity = owner.ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        let replay_evidence =
            RecoveredWalVoteReplayEvidenceV1::from_sealed_recovered_vote(wal_identity, *tag, vote)
                .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        RecoveredLifecycleNextWalVoteSealV1::from_authenticated_adapter(
            RecoveredLifecycleNextWalVoteSealPermitV1::new(),
            wal_identity,
            replay_evidence,
            effect.clone(),
            validated.clone(),
        )
        .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)
    }
    /// Consume the current ProposalIntent/TimeoutIntent control Sign from replay.
    ///
    /// The reducer awaiting-signature state, tag, local role, unsigned action,
    /// and complete one-effect inventory must agree. The latest exact matching
    /// authenticated WAL frame owns the control Sign even when later frames own
    /// queued phase signatures.
    fn authenticate_recovered_wal_control_sign(
        &mut self,
        startup_effects: &mut Vec<AdapterEffect>,
    ) -> Result<Option<RecoveredWalControlSign>, AdapterError> {
        self.ensure_ingress()?;
        let expected_tag = self.current_tag();
        let (wal_identity, expected) = match self.reducer.awaiting_signature() {
            Some(reducer::SignableMessage::Proposal(awaiting)) => {
                let proposal = self
                    .registry
                    .unsigned_proposal_to_wire(awaiting, self.aggregator.as_ref())?;
                if !proposal.signature.is_empty()
                    || proposal.round.context_id != self.wire_context.id()
                    || proposal.round.height != self.wire_context.height
                    || expected_tag.height() != proposal.round.height
                    || expected_tag.view() != proposal.round.view
                {
                    return Err(AdapterError::RecoveredControlSignMismatch);
                }
                let mut owner = None;
                for frame in self.wal.recovered_records().iter().rev() {
                    let (identity, envelope) = self.authenticate_recovered_wal_frame(frame)?;
                    if matches!(
                        envelope.record,
                        WalRecordV2::ProposalIntent(candidate) if candidate == proposal
                    ) {
                        owner = Some(identity);
                        break;
                    }
                }
                let Some(owner) = owner else {
                    return Err(AdapterError::RecoveredControlSignMismatch);
                };
                (
                    owner,
                    AdapterEffect::Sign {
                        tag: expected_tag,
                        request: SignRequest::Proposal(proposal),
                    },
                )
            }
            Some(reducer::SignableMessage::TimeoutVote(awaiting)) => {
                let vote = self
                    .registry
                    .unsigned_timeout_vote_to_wire(awaiting, self.aggregator.as_ref())?;
                let local = self
                    .reducer
                    .local_validator()
                    .and_then(|validator| self.registry.validator_index(validator).ok());
                if !vote.signature.is_empty()
                    || local != Some(vote.signer)
                    || vote.round.context_id != self.wire_context.id()
                    || vote.round.height != self.wire_context.height
                    || expected_tag.height() != vote.round.height
                    || expected_tag.view() != vote.round.view
                {
                    return Err(AdapterError::RecoveredControlSignMismatch);
                }
                let mut owner = None;
                for frame in self.wal.recovered_records().iter().rev() {
                    let (identity, envelope) = self.authenticate_recovered_wal_frame(frame)?;
                    if matches!(
                        envelope.record,
                        WalRecordV2::TimeoutIntent(candidate) if candidate == vote
                    ) {
                        owner = Some(identity);
                        break;
                    }
                }
                let Some(owner) = owner else {
                    return Err(AdapterError::RecoveredControlSignMismatch);
                };
                (
                    owner,
                    AdapterEffect::Sign {
                        tag: expected_tag,
                        request: SignRequest::TimeoutVote(vote),
                    },
                )
            }
            Some(reducer::SignableMessage::Vote(_)) | None => {
                if startup_effects.iter().any(|effect| {
                    matches!(
                        effect,
                        AdapterEffect::Sign {
                            request: SignRequest::Proposal(_) | SignRequest::TimeoutVote(_),
                            ..
                        }
                    )
                }) {
                    return Err(AdapterError::RecoveredControlSignMismatch);
                }
                return Ok(None);
            }
        };
        if startup_effects.as_slice() != [expected.clone()] {
            return Err(AdapterError::RecoveredControlSignMismatch);
        }
        let Some(replay_evidence) =
            RecoveredWalControlReplayEvidenceV1::from_sealed_recovered_control(
                wal_identity,
                &expected,
            )
        else {
            return Err(AdapterError::RecoveredControlSignMismatch);
        };
        let effect = startup_effects
            .pop()
            .expect("one exact control Sign was compared above");
        Ok(Some(RecoveredWalControlSign {
            wal_identity,
            replay_evidence,
            effect,
        }))
    }
    /// Consume the exact certificate-backed Fetch owned by a durable Decision.
    ///
    /// The reducer Decision, current tag, complete one-effect inventory,
    /// manifest absence, frozen ordered archive roster, authenticated CommitQC,
    /// and latest exact Decision frame must all agree. The resulting token has
    /// no raw-effect or locator extraction surface.
    fn authenticate_recovered_wal_decision_fetch(
        &mut self,
        startup_effects: &mut Vec<AdapterEffect>,
    ) -> Result<Option<RecoveredWalDecisionFetch>, AdapterError> {
        self.ensure_ingress()?;
        let has_fetch = startup_effects
            .iter()
            .any(|effect| matches!(effect, AdapterEffect::FetchBody { .. }));
        let Some(decision) = self.reducer.durable_state().decision().cloned() else {
            return if has_fetch {
                Err(AdapterError::RecoveredDecisionFetchMismatch)
            } else {
                Ok(None)
            };
        };
        let certificate = self
            .registry
            .qc_to_wire(&decision, self.aggregator.as_ref())?;
        let expected_sources = self
            .wire_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let expected = AdapterEffect::FetchBody {
            tag: self.current_tag(),
            round: certificate.proposal_round,
            subject: certificate.subject,
            manifest: None,
            certified_sources: expected_sources,
            certificate: Some(certificate.clone()),
        };
        if startup_effects.as_slice() != [expected.clone()]
            || certificate.phase != wire::GlobalPhase::Commit
        {
            return if has_fetch {
                Err(AdapterError::RecoveredDecisionFetchMismatch)
            } else {
                Ok(None)
            };
        }
        let mut owner = None;
        for frame in self.wal.recovered_records().iter().rev() {
            let (identity, envelope) = self.authenticate_recovered_wal_frame(frame)?;
            if matches!(envelope.record, WalRecordV2::Decision(candidate) if candidate == certificate)
            {
                owner = Some(identity);
                break;
            }
        }
        let Some(wal_identity) = owner else {
            return Err(AdapterError::RecoveredDecisionFetchMismatch);
        };
        let verified = VerifiedHeightContext {
            context: self.wire_context.clone(),
            proofs_of_possession: self.proofs_of_possession.clone(),
            parent_verification: self.parent_verification.clone(),
        };
        let Some(replay_evidence) =
            RecoveredWalDecisionFetchReplayEvidenceV1::from_sealed_recovered_decision_fetch(
                &verified,
                wal_identity,
                &expected,
            )
        else {
            return Err(AdapterError::RecoveredDecisionFetchMismatch);
        };
        let effect = startup_effects
            .pop()
            .expect("one exact Decision Fetch was compared above");
        Ok(Some(RecoveredWalDecisionFetch {
            wal_identity,
            replay_evidence,
            effect,
        }))
    }
    /// Return the exact Decision key reconstructed from complete WAL frames.
    ///
    /// Startup uses this before ingress opens to bind an interrupted canonical
    /// Kura tip to the reducer Decision and the exact durable body marker. A
    /// missing value means WAL replay contains no durable CommitQC decision.
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
        self.reducer
            .durable_state()
            .decision()
            .map(|certificate| {
                Ok((
                    self.registry.round_to_wire(certificate.round()),
                    self.registry.round_to_wire(certificate.proposal_round()),
                    self.registry.subject(certificate.subject())?,
                    self.registry.execution_commitment(
                        certificate.proposal_round(),
                        certificate.subject(),
                    )?,
                ))
            })
            .transpose()
    }
    /// Return the exact highest Prepare certificate retained by safety-WAL replay.
    ///
    /// This frontier is distinct from the voting lock: a node may durably
    /// observe a PrepareQC before a TimeoutCertificate promotes any PrepareQC
    /// into the active lock.
    pub(crate) fn replayed_highest_prepare_certificate_ref(
        &self,
    ) -> Result<Option<wire::QuorumCertificateRef>, AdapterError> {
        self.reducer
            .durable_state()
            .highest_prepare()
            .map(|certificate| {
                Ok(wire::QuorumCertificateRef {
                    round: self.registry.round_to_wire(certificate.round()),
                    proposal_round: self.registry.round_to_wire(certificate.proposal_round()),
                    phase: WireRegistry::phase_to_wire(certificate.phase()),
                    subject: self.registry.subject(certificate.subject())?,
                    execution_commitment: self.registry.execution_commitment(
                        certificate.proposal_round(),
                        certificate.subject(),
                    )?,
                })
            })
            .transpose()
    }
    /// Return the strongest complete body certificate retained by durable reducer state.
    ///
    /// A body stage may be physically retained from an authenticated Proposal
    /// while the reducer monotonically refines its semantic owner to Prepare or
    /// Commit. Lifecycle replay must retain that complete QC, rather than only
    /// the route-neutral statement derived from it, before publishing the
    /// refined Store or Validate row.
    pub(crate) fn replayed_body_authority_certificate(
        &self,
    ) -> Result<Option<wire::QuorumCertificate>, AdapterError> {
        let durable = self.reducer.durable_state();
        let certificate = durable.decision().or_else(|| durable.locked());
        let Some(certificate) = certificate else {
            return Ok(None);
        };
        let mut registry = self.registry.clone();
        registry
            .qc_to_wire(certificate, self.aggregator.as_ref())
            .map(Some)
    }
    /// Return whether WAL replay completed and authenticated ingress may open.
    pub(crate) const fn ingress_ready(&self) -> bool {
        self.replay_complete && !self.fail_closed
    }
    /// Return whether application completed and no unfinished safety write,
    /// signature, or adapter-owned deferred input remains before height
    /// rollover.
    pub(crate) fn ready_to_finish(&self) -> bool {
        self.ingress_ready()
            && self.deferred_completions.is_empty()
            && self.deferred_progress_inputs.is_empty()
            && self.deferred_inputs.is_empty()
            && self.reducer.ready_to_finish()
    }
    /// Verify a canonical consensus message against this adapter's frozen
    /// roster and prevalidated proofs of possession.
    pub(crate) fn authenticate(
        &self,
        message: wire::ConsensusMessageV2,
    ) -> Result<AuthenticatedConsensusMessage, AdapterError> {
        self.ensure_ingress()?;
        verify_authenticated_message(
            &self.wire_context,
            self.parent_verification.as_ref(),
            &message,
            &self.proofs_of_possession,
        )?;
        let authenticated = AuthenticatedConsensusMessage(message);
        // A second, independently authenticated statement for an already
        // retained semantic slot must reach exact-evidence admission even when
        // its manifest or execution commitment deliberately conflicts with the
        // local registry. Ordinary traffic still fails those consistency
        // gates before it can mutate adapter state.
        if !self.retained_authenticated_equivocation(authenticated.payload()) {
            self.ensure_authenticated_manifest_compatible(&authenticated)?;
            self.ensure_authenticated_execution_commitments_compatible(&authenticated)?;
        }
        Ok(authenticated)
    }
    fn retained_authenticated_equivocation(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        let Some((key, fingerprint)) = ingress_equivocation_identity(payload) else {
            return false;
        };
        self.ingress_equivocations
            .get(&key)
            .is_some_and(|record| record.fingerprint != fingerprint)
    }
    /// Return whether authenticated ingress belongs to the active lock's
    /// reserved progress path.
    ///
    /// QCs and TCs have their own progress classification at the runtime
    /// boundary. This predicate is deliberately narrower: only an exact
    /// historical Commit vote or an exact current-view Prepare witness for an
    /// unchanged older lock may bypass normal ingress capacity.
    pub(crate) fn authenticated_ingress_is_progress(
        &self,
        message: &AuthenticatedConsensusMessage,
    ) -> bool {
        self.wire_ingress_may_use_progress(message.payload())
    }
    /// Return the tag of an exact Commit/Prepare QC already owned by the
    /// adapter's Busy-deferred progress lane.
    ///
    /// This comparison is intentionally exact, including canonical signer
    /// order and aggregate signature. Runtime admission may use the result as
    /// a capacity hint, but it must independently authenticate the arriving
    /// envelope before coalescing it with this owner.
    pub(crate) fn deferred_quorum_certificate_owner_tag(
        &self,
        candidate: &wire::QuorumCertificate,
    ) -> Option<reducer::EventTag> {
        self.deferred_quorum_certificate_owner(candidate)
            .map(|(tag, _)| tag)
    }
    /// Return the tag and actor-global admission ordinal of an exact
    /// Commit/Prepare QC already owned by the Busy-deferred progress lane.
    ///
    /// The ordinal is an opaque process-local association key. It lets the
    /// serialized runtime merge later authenticated-source routes into the
    /// exact deferred occurrence without exposing or reconstructing the
    /// adapter's reducer event.
    pub(crate) fn deferred_quorum_certificate_owner(
        &self,
        candidate: &wire::QuorumCertificate,
    ) -> Option<(reducer::EventTag, u128)> {
        self.deferred_progress_inputs.iter().find_map(|input| {
            let reducer::Event::QuorumCertificateReceived { tag, certificate } = &input.event
            else {
                return None;
            };
            self.registry
                .reducer_qc_matches_wire(certificate, candidate)
                .then_some((*tag, input.admission_ordinal))
        })
    }
    /// Return the tag and actor-global admission ordinal of an exact canonical
    /// authenticated envelope already owned by a Busy-deferred lane.
    ///
    /// The ordinal is an opaque process-local association key. It lets the
    /// serialized runtime merge later authenticated-source routes into the
    /// exact deferred occurrence without exposing or reconstructing the
    /// adapter's reducer event. This raw-byte comparison is only a capacity
    /// hint; runtime admission repeats it after authenticating the candidate.
    pub(crate) fn deferred_authenticated_message_owner(
        &self,
        candidate: &wire::ConsensusMessageV2,
    ) -> Option<(reducer::EventTag, u128)> {
        if let wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) = &candidate.payload
        {
            return self.deferred_quorum_certificate_owner(certificate);
        }
        let encoded = candidate.encode();
        self.deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .find_map(|input| {
                input
                    .authenticated_wire_identity
                    .as_deref()
                    .is_some_and(|owned| owned == encoded.as_slice())
                    .then_some((deferred_event_tag(&input.event), input.admission_ordinal))
            })
    }
    /// Exact actor-global ordinals currently retained by authenticated
    /// Busy-deferred inputs across all service classes.
    ///
    /// The serialized runtime uses this snapshot only to retire carriers for
    /// inputs which a legitimate adapter transition superseded and to reject
    /// any newly active deferred input that lacks its original carrier.
    pub(crate) fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
        self.deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .filter(|input| input.retag_authenticated_ingress)
            .map(|input| input.admission_ordinal)
            .collect()
    }
    /// Exact actor-global ordinals retained by every Busy-deferred input.
    /// Runtime lifecycle ownership uses this complete set; the authenticated
    /// subset above remains the separate fair-ingress carrier authority.
    pub(crate) fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
        self.deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .map(|input| input.admission_ordinal)
            .collect()
    }
    /// Snapshot the private actor capability of one exact retained Busy owner
    /// without claiming its service turn.
    pub(crate) fn deferred_occurrence_ownership(
        &self,
        admission_ordinal: u128,
    ) -> Option<DeferredOccurrenceOwnershipEvidence> {
        let mut matching = self
            .deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .filter(|input| input.admission_ordinal == admission_ordinal);
        let input = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        DeferredOccurrenceOwnershipEvidence::from_input(input, &self.deferred_admission_ordinals)
    }
    /// Atomically attach the runtime's immutable lifecycle/cut owner to one
    /// newly admitted Busy occurrence and return its opaque adapter seal.
    ///
    /// Exact repetition is idempotent. A different binding for the same
    /// adapter ordinal, a foreign capability, or an already claimed occurrence
    /// closes the adapter before scheduler state can be retained.
    pub(crate) fn bind_deferred_runtime_ownership(
        &mut self,
        admission_ordinal: u128,
        causal_lifecycle_key: Hash,
        initial_lifecycle_ordinal: u128,
        authenticated_ingress: bool,
        source_physical_ordinal: Option<u64>,
        physical_cut: u128,
    ) -> Result<DeferredRuntimeOwnershipSeal, AdapterError> {
        let binding = DeferredRuntimeOwnershipBinding {
            causal_lifecycle_key,
            initial_lifecycle_ordinal,
            authenticated_ingress,
            source_physical_ordinal,
            physical_cut,
        };
        let matching = self
            .deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .filter(|input| input.admission_ordinal == admission_ordinal)
            .count();
        if matching != 1 || !binding.validate_exact() {
            self.fail_closed = true;
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        let source = &self.deferred_admission_ordinals.identity;
        let input = self
            .deferred_completions
            .iter_mut()
            .chain(&mut self.deferred_progress_inputs)
            .chain(&mut self.deferred_inputs)
            .find(|input| input.admission_ordinal == admission_ordinal)
            .expect("the exact matching Busy occurrence was counted above");
        let capability = &mut input.admission_capability;
        let exact_input = capability.ordinal == admission_ordinal
            && capability.origin.is_authenticated() == authenticated_ingress
            && input.retag_authenticated_ingress == authenticated_ingress
            && input.authenticated_wire_identity.is_some() == authenticated_ingress
            && Arc::ptr_eq(&capability.source_identity, source)
            && !capability.adapter_service_is_claimed()
            && !capability.runtime_handoff_is_claimed();
        if !exact_input
            || capability
                .runtime_ownership
                .as_ref()
                .is_some_and(|retained| retained != &binding)
        {
            self.fail_closed = true;
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        if capability.runtime_ownership.is_none() {
            capability.runtime_ownership = Some(binding);
        }
        let Some(seal) = capability.runtime_ownership_seal() else {
            self.fail_closed = true;
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        };
        if !seal.still_retained() {
            self.fail_closed = true;
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        Ok(seal)
    }
    /// Return whether a wire payload may use the active lock's progress lane.
    ///
    /// This is only a pre-authentication capacity hint. A current Prepare must
    /// already match the locally bound current-round execution commitment;
    /// the incoming vote cannot create that binding. Callers must still
    /// authenticate the envelope and use [`Self::authenticated_ingress_is_progress`]
    /// as the security gate before enqueueing it as progress traffic.
    pub(crate) fn wire_ingress_may_use_progress(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        matches!(
            payload,
            wire::ConsensusMessageV2Payload::Vote(vote)
                if self.is_exact_locked_commit_vote(vote)
                    || self.is_exact_locked_reproposal_prepare_vote(vote)
        )
    }
    /// Return the body identity whose direct vote lacks a locally validated
    /// execution commitment.
    ///
    /// This is a non-authenticating dequeue hint, but it still applies every
    /// cheap structural check before retaining fair-ingress ownership.
    /// Malformed votes and invalid or conflicting commitments return `None`
    /// so the mutating admission seam can reject them instead of allowing a
    /// far-future malformed occurrence to pin one source lane.
    pub(crate) fn wire_ingress_missing_execution_commitment(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> Option<(wire::ConsensusRound, wire::BlockSubject)> {
        let wire::ConsensusMessageV2Payload::Vote(vote) = payload else {
            return None;
        };
        if vote.validate(&self.wire_context).is_err() {
            return None;
        }
        matches!(
            self.ensure_vote_execution_commitment_bound(
                vote.proposal_round,
                vote.subject,
                vote.execution_commitment,
            ),
            Err(AdapterError::MissingExecutionCommitment)
        )
        .then_some((vote.proposal_round, vote.subject))
    }
    fn ensure_authenticated_manifest_compatible(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> Result<(), AdapterError> {
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = authenticated.payload() else {
            return Ok(());
        };
        if self.registry.manifest_conflicts(&proposal.manifest) {
            return Err(AdapterError::ConflictingManifest);
        }
        Ok(())
    }
    fn ensure_authenticated_execution_commitments_compatible(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> Result<(), AdapterError> {
        let mut observed = Vec::new();
        match authenticated.payload() {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => match &proposal.justification {
                wire::ProposalJustification::ParentCommit(parent) => {
                    if let Some(certificate) = &parent.certificate {
                        self.ensure_qc_execution_commitment_compatible(certificate, &mut observed)?;
                    }
                }
                wire::ProposalJustification::Timeout(timeout) => {
                    self.ensure_tc_execution_commitments_compatible(
                        &timeout.timeout_certificate,
                        &mut observed,
                    )?;
                    if let Some(certificate) = &timeout.highest_prepare_qc {
                        self.ensure_qc_execution_commitment_compatible(certificate, &mut observed)?;
                    }
                }
            },
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                self.ensure_vote_execution_commitment_bound(
                    vote.proposal_round,
                    vote.subject,
                    vote.execution_commitment,
                )?;
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                self.ensure_qc_execution_commitment_compatible(certificate, &mut observed)?;
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                if let Some(certificate) = &vote.highest_prepare_qc {
                    self.ensure_qc_execution_commitment_compatible(certificate, &mut observed)?;
                }
            }
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                self.ensure_tc_execution_commitments_compatible(certificate, &mut observed)?;
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
                self.ensure_qc_execution_commitment_compatible(
                    &request.certificate,
                    &mut observed,
                )?;
            }
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
                self.ensure_qc_execution_commitment_compatible(
                    &response.certificate,
                    &mut observed,
                )?;
            }
            wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => {}
        }
        Ok(())
    }
    fn ensure_vote_execution_commitment_bound(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        commitment: wire::ExecutionCommitment,
    ) -> Result<(), AdapterError> {
        commitment.validate()?;
        let core_round = reducer::Round::new(round.height, round.view);
        let core_subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        if self
            .registry
            .subjects
            .get(&core_subject)
            .is_some_and(|registered| *registered != subject)
        {
            return Err(AdapterError::SubjectCollision);
        }
        if self.registry.execution_commitments.iter().any(
            |((_, registered_subject), registered)| {
                *registered_subject == core_subject && *registered != commitment
            },
        ) {
            return Err(AdapterError::ConflictingExecutionCommitment);
        }
        match self
            .registry
            .execution_commitments
            .get(&(core_round, core_subject))
        {
            Some(registered) if *registered == commitment => Ok(()),
            Some(_) => Err(AdapterError::ConflictingExecutionCommitment),
            None => Err(AdapterError::MissingExecutionCommitment),
        }
    }
    fn ensure_tc_execution_commitments_compatible(
        &self,
        certificate: &wire::TimeoutCertificate,
        observed: &mut Vec<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Result<(), AdapterError> {
        for group in &certificate.groups {
            if let Some(highest) = &group.highest_prepare_qc {
                self.ensure_qc_execution_commitment_compatible(highest, observed)?;
            }
        }
        Ok(())
    }
    fn ensure_qc_execution_commitment_compatible(
        &self,
        certificate: &wire::QuorumCertificate,
        observed: &mut Vec<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Result<(), AdapterError> {
        self.ensure_execution_commitment_compatible(
            certificate.proposal_round,
            certificate.subject,
            certificate.execution_commitment,
            observed,
        )
    }
    fn ensure_execution_commitment_compatible(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        commitment: wire::ExecutionCommitment,
        observed: &mut Vec<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Result<(), AdapterError> {
        commitment.validate()?;
        let core_subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        if self
            .registry
            .subjects
            .get(&core_subject)
            .is_some_and(|registered| *registered != subject)
        {
            return Err(AdapterError::SubjectCollision);
        }
        if self.registry.execution_commitments.iter().any(
            |((_, registered_subject), registered)| {
                *registered_subject == core_subject && *registered != commitment
            },
        ) || observed.iter().any(|(_, registered_subject, registered)| {
            *registered_subject == subject && *registered != commitment
        }) {
            return Err(AdapterError::ConflictingExecutionCommitment);
        }
        observed.push((round, subject, commitment));
        Ok(())
    }
    fn is_exact_locked_commit_vote(&self, vote: &wire::Vote) -> bool {
        if vote.phase != wire::GlobalPhase::Commit {
            return false;
        }
        let durable = self.reducer.durable_state();
        // A retained Prepare lock stops being a reconstruction witness once a
        // CommitQC is durable. Post-decision votes therefore use ordinary,
        // height-long duplicate admission and cannot repeatedly consume the
        // generation-scoped protected lane while application is pending.
        if durable.decision().is_some() {
            return false;
        }
        let Some(locked) = durable.locked() else {
            return false;
        };
        (locked.round().view() == self.reducer.current_tag().view()
            || durable.commit_intent_for_lock(locked).is_some())
            && vote.proposal_round.height == locked.round().height()
            && vote.proposal_round.view == locked.round().view()
            && vote.round == vote.proposal_round
            && self
                .registry
                .subject(locked.subject())
                .is_ok_and(|subject| subject == vote.subject)
            && self
                .registry
                .execution_commitment(locked.round(), locked.subject())
                .is_ok_and(|commitment| commitment == vote.execution_commitment)
    }
    /// Return whether `vote` is the exact current-view Prepare witness which
    /// can replace an older durable lock with an unchanged-body lock.
    ///
    /// This is a scheduling predicate only. Authentication and reducer
    /// admission remain mandatory. Requiring both the historical and current
    /// round registry bindings prevents an unknown or conflicting body from
    /// borrowing the protected Progress lane while allowing a durably
    /// validated reproposal to complete before rapid timeout churn clears its
    /// partial Prepare pool.
    fn is_exact_locked_reproposal_prepare_vote(&self, vote: &wire::Vote) -> bool {
        if vote.phase != wire::GlobalPhase::Prepare || vote.round != vote.proposal_round {
            return false;
        }
        let current = self.reducer.current_tag();
        if vote.round.context_id != self.wire_context.id()
            || vote.round.height != current.height()
            || vote.round.view != current.view()
        {
            return false;
        }
        let durable = self.reducer.durable_state();
        if durable.decision().is_some() {
            return false;
        }
        let Some(locked) = durable.locked() else {
            return false;
        };
        if locked.round().height() != current.height() || locked.round().view() >= current.view() {
            return false;
        }
        let current_round = reducer::Round::new(current.height(), current.view());
        self.registry
            .subject(locked.subject())
            .is_ok_and(|subject| subject == vote.subject)
            && self
                .registry
                .execution_commitment(locked.round(), locked.subject())
                .is_ok_and(|commitment| commitment == vote.execution_commitment)
            && self
                .registry
                .execution_commitment(current_round, locked.subject())
                .is_ok_and(|commitment| commitment == vote.execution_commitment)
    }
    /// Return the one current-view unchanged-lock body statement whose exact
    /// Prepare witnesses may cross an already-due timeout boundary.
    ///
    /// This is only a target projection. Every arriving Prepare vote or QC
    /// must still pass the ordinary wire/authentication checks and a cloned
    /// reducer preview before it can acquire the bounded scheduler exception.
    pub(crate) fn pre_timeout_locked_prepare_qc_target(
        &self,
    ) -> Option<super::v2_runtime::PreTimeoutLockedPrepareQcTargetV1> {
        let current_tag = self.reducer.current_tag();
        let current_round = reducer::Round::new(current_tag.height(), current_tag.view());
        let durable = self.reducer.durable_state();
        let locked = durable.locked()?;
        if self.fail_closed
            || !self.replay_complete
            || durable.decision().is_some()
            || locked.round().height() != current_round.height()
            || locked.round().view() >= current_round.view()
            || durable.timeout_intent(current_round).is_some()
            || durable.commit_intent(current_round).is_some()
            || self.reducer.local_validator().is_none()
            || !self.reducer.local_candidate_body_is_eligible()
            || self.reducer.pending_persistence_record().is_some()
            || self.reducer.awaiting_signature().is_some()
            || self.reducer.body_state(current_round, locked.subject())
                != reducer::BodyState::Validated
        {
            return None;
        }
        let subject = self.registry.subject(locked.subject()).ok()?;
        let locked_commitment = self
            .registry
            .execution_commitment(locked.round(), locked.subject())
            .ok()?;
        let current_commitment = self
            .registry
            .execution_commitment(current_round, locked.subject())
            .ok()?;
        (locked_commitment == current_commitment).then_some(
            super::v2_runtime::PreTimeoutLockedPrepareQcTargetV1 {
                round: self.registry.round_to_wire(current_round),
                subject,
                execution_commitment: current_commitment,
            },
        )
    }
    /// Deep-preview one wire PrepareQC against the exact pre-timeout target.
    ///
    /// The live reducer and registry remain untouched. Acceptance requires the
    /// same staged conversion as normal authenticated ingress and exactly one
    /// immediate `Persist(LockAndCommit)` effect for the arriving certificate;
    /// a duplicate, observer, signature/WAL fence, wrong body, or any fetch or
    /// ObservePrepare path therefore cannot delay the timeout.
    pub(crate) fn pre_timeout_locked_prepare_qc_stages_lock_and_commit(
        &self,
        certificate: &wire::QuorumCertificate,
        target: super::v2_runtime::PreTimeoutLockedPrepareQcTargetV1,
    ) -> bool {
        if certificate.phase != wire::GlobalPhase::Prepare
            || certificate.round != target.round
            || certificate.proposal_round != target.round
            || certificate.subject != target.subject
            || certificate.execution_commitment != target.execution_commitment
            || self.pre_timeout_locked_prepare_qc_target() != Some(target)
        {
            return false;
        }
        let mut registry = self.registry.clone();
        let Ok(core_certificate) = registry.qc_to_core(certificate, &self.wire_context) else {
            return false;
        };
        let mut reducer = self.reducer.clone();
        let Ok(outcome) = reducer.step(reducer::Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: core_certificate.clone(),
        }) else {
            return false;
        };
        let [reducer::Effect::Persist { entry, .. }] = outcome.effects() else {
            return false;
        };
        matches!(
            entry.record(),
            reducer::WalRecord::LockAndCommit { prepare, vote }
                if prepare == &core_certificate
                    && vote.context_id() == core_certificate.reference().context_id()
                    && vote.round() == core_certificate.round()
                    && vote.proposal_round() == core_certificate.proposal_round()
                    && vote.phase() == reducer::Phase::Commit
                    && vote.subject() == core_certificate.subject()
        )
    }
    /// Deep-preview one exact authenticated current-view Prepare witness for
    /// the unchanged older lock.
    ///
    /// A successful preview proves that the ordinary reducer transition adds
    /// exactly this previously absent signer to the target Prepare pool. The
    /// transition may be a pre-quorum stutter at the effect boundary, or it
    /// may form the exact PrepareQC and immediately stage `LockAndCommit`.
    /// No live registry, admission table, vote pool, or WAL state is mutated.
    fn pre_timeout_locked_reproposal_prepare_vote_advances(
        &self,
        vote: &wire::Vote,
        target: super::v2_runtime::PreTimeoutLockedPrepareQcTargetV1,
    ) -> bool {
        if vote.phase != wire::GlobalPhase::Prepare
            || vote.round != target.round
            || vote.proposal_round != target.round
            || vote.subject != target.subject
            || vote.execution_commitment != target.execution_commitment
            || !self.is_exact_locked_reproposal_prepare_vote(vote)
            || self.pre_timeout_locked_prepare_qc_target() != Some(target)
        {
            return false;
        }
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote.clone()));
        if verify_authenticated_message(
            &self.wire_context,
            self.parent_verification.as_ref(),
            &message,
            &self.proofs_of_possession,
        )
        .is_err()
        {
            return false;
        }
        let mut registry = self.registry.clone();
        let Ok(core_vote) = registry.vote_to_core(vote, &self.wire_context) else {
            return false;
        };
        let vote_statement = core_vote.vote();
        let pool_signers = |reducer: &reducer::Reducer| {
            reducer
                .vote_pool_snapshots()
                .into_iter()
                .find(|pool| {
                    pool.round == vote_statement.round()
                        && pool.proposal_round == vote_statement.proposal_round()
                        && pool.phase == reducer::Phase::Prepare
                        && pool.subject == vote_statement.subject()
                })
                .map_or_else(Vec::new, |pool| pool.signers)
        };
        let before = pool_signers(&self.reducer);
        if before.contains(&vote_statement.signer()) {
            return false;
        }
        let mut reducer = self.reducer.clone();
        let Ok(outcome) = reducer.step(reducer::Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: core_vote,
        }) else {
            return false;
        };
        let after = pool_signers(&reducer);
        if outcome.disposition() != reducer::StepDisposition::Applied
            || after.len() != before.len().saturating_add(1)
            || !after.contains(&vote_statement.signer())
            || !before.iter().all(|signer| after.contains(signer))
        {
            return false;
        }
        match outcome.effects() {
            [] => true,
            [
                reducer::Effect::Broadcast(reducer::ConsensusMessageV2::QuorumCertificate(
                    certificate,
                )),
                reducer::Effect::Persist { entry, .. },
            ] => matches!(
                entry.record(),
                reducer::WalRecord::LockAndCommit { prepare, vote }
                    if prepare == certificate
                        && certificate.phase() == reducer::Phase::Prepare
                        && certificate.round() == vote_statement.round()
                        && certificate.proposal_round() == vote_statement.proposal_round()
                        && certificate.subject() == vote_statement.subject()
                        && vote.context_id() == certificate.reference().context_id()
                        && vote.round() == certificate.round()
                        && vote.proposal_round() == certificate.proposal_round()
                        && vote.phase() == reducer::Phase::Commit
                        && vote.subject() == certificate.subject()
            ),
            _ => false,
        }
    }
    /// Return whether one fixed-cut carrier is productive exact locked-body
    /// Prepare progress in the current adapter state.
    pub(crate) fn pre_timeout_locked_prepare_progress_is_exact(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
        target: super::v2_runtime::PreTimeoutLockedPrepareQcTargetV1,
    ) -> bool {
        match payload {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                self.pre_timeout_locked_reproposal_prepare_vote_advances(vote, target)
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                self.pre_timeout_locked_prepare_qc_stages_lock_and_commit(certificate, target)
            }
            _ => false,
        }
    }
    fn deferred_owns_ingress(
        &self,
        key: IngressSemanticKey,
        fingerprint: IngressFingerprint,
    ) -> bool {
        // A certified EnterView may advance the reducer generation before its executor returns
        // control to the deferred runner. The old tagged event remains the sole owner during
        // that boundary, so an exact retransmission must coalesce instead of claiming the new
        // generation's protected slot.
        self.deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .any(|input| {
                input.admission.is_some_and(|admission| {
                    admission.key == key && admission.fingerprint == fingerprint
                })
            })
    }
    #[allow(clippy::too_many_lines)]
    fn admit_authenticated_payload(
        &mut self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> Result<(Option<AdapterOutcome>, Option<IngressAdmission>), AdapterError> {
        let current_tag = self.reducer.current_tag();
        let current_view = current_tag.view();
        self.prune_ingress_records();
        let locked_commit_progress = match payload {
            wire::ConsensusMessageV2Payload::Vote(vote) => self.is_exact_locked_commit_vote(vote),
            _ => false,
        };
        let locked_reproposal_prepare_progress = match payload {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                self.is_exact_locked_reproposal_prepare_vote(vote)
            }
            _ => false,
        };
        let unsafe_proposal = if let wire::ConsensusMessageV2Payload::Proposal(proposal) = payload
            && let Some(locked) = self.reducer.durable_state().locked()
        {
            let locked_round = self.registry.round_to_wire(locked.round());
            let locked_subject = self.registry.subject(locked.subject())?;
            !proposal_is_safe_for_lock(proposal, locked_round, locked_subject)
        } else {
            false
        };
        if !self
            .leader_wire_recovery_authority()?
            .admits_payload(payload)
        {
            // Already-owned future work remains retryable. Fresh ingress uses
            // the same policy before it can reserve a token or FIFO position.
            let future = match payload {
                wire::ConsensusMessageV2Payload::Proposal(p) => p.round.view > current_view,
                wire::ConsensusMessageV2Payload::Vote(v) => v.round.view > current_view,
                wire::ConsensusMessageV2Payload::TimeoutVote(v) => v.round.view > current_view,
                wire::ConsensusMessageV2Payload::QuorumCertificate(qc) => {
                    qc.round.view > current_view
                }
                _ => false,
            };
            return Ok((
                Some(Self::ignored_outcome(if future {
                    reducer::IgnoreReason::Busy
                } else {
                    reducer::IgnoreReason::IrrelevantView
                })),
                None,
            ));
        }
        if !matches!(
            payload,
            wire::ConsensusMessageV2Payload::Proposal(_)
                | wire::ConsensusMessageV2Payload::Vote(_)
                | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        ) {
            return Ok((None, None));
        }
        let (key, fingerprint) = ingress_equivocation_identity(payload)
            .ok_or(AdapterError::EquivocationArtifactMismatch)?;
        let artifact = IngressEquivocationArtifact::from_payload(payload)
            .ok_or(AdapterError::EquivocationArtifactMismatch)?;
        let deferred_owner = self.deferred_owns_ingress(key, fingerprint);
        let height_decided = self.reducer.durable_state().decision().is_some();
        if let Some(record) = self.ingress_equivocations.get_mut(&key) {
            if record.fingerprint == fingerprint {
                if deferred_owner
                    || self.ingress_deliveries.get(&key).is_some_and(|delivered| {
                        debug_assert_eq!(delivered.fingerprint, fingerprint);
                        let exact_protected_epoch = if locked_commit_progress {
                            delivered.locked_commit_progress
                                && !delivered.locked_reproposal_prepare_progress
                        } else if locked_reproposal_prepare_progress {
                            !delivered.locked_commit_progress
                                && delivered.locked_reproposal_prepare_progress
                        } else if matches!(
                            key,
                            IngressSemanticKey::Proposal { .. }
                                | IngressSemanticKey::Vote {
                                    phase: wire::GlobalPhase::Prepare,
                                    ..
                                }
                        ) {
                            // A strict same-round TC upgrade can change the
                            // lock without changing the view, so re-evaluate
                            // one exact proposal in the new consumer epoch.
                            true
                        } else {
                            return true;
                        };
                        exact_protected_epoch && delivered.consumer_tag == current_tag
                    })
                {
                    return Ok((
                        Some(Self::ignored_outcome(reducer::IgnoreReason::Duplicate)),
                        None,
                    ));
                }
                let admission = IngressAdmission {
                    key,
                    fingerprint,
                    consumer_tag: current_tag,
                    inserted_equivocation: false,
                    locked_commit_progress,
                    locked_reproposal_prepare_progress,
                };
                if unsafe_proposal {
                    self.record_ingress_delivery(admission);
                    return Ok((
                        Some(Self::ignored_outcome(reducer::IgnoreReason::UnsafeProposal)),
                        None,
                    ));
                }
                return Ok((None, Some(admission)));
            }
            if record.equivocation_reported {
                return Ok((
                    Some(Self::ignored_outcome(reducer::IgnoreReason::Duplicate)),
                    None,
                ));
            }
            if height_decided {
                // Once this height has a durable Decision, a newly observed
                // conflict cannot affect consensus safety. Emitting diagnostic
                // work here would put that non-critical output ahead of the
                // decided Apply and can deadlock a minimally sized executor.
                // Preserve the original semantic record and terminally absorb
                // the conflicting authenticated carrier instead.
                return Ok((
                    Some(Self::ignored_outcome(reducer::IgnoreReason::AlreadyDecided)),
                    None,
                ));
            }
            let evidence = record.artifact.conflict_with(payload)?;
            record.equivocation_reported = true;
            return Ok((
                Some(AdapterOutcome {
                    disposition: reducer::StepDisposition::Applied,
                    effects: vec![AdapterEffect::ReportEquivocation { evidence }],
                    deferred_admission_ordinal: None,
                    producer_handoff: None,
                }),
                None,
            ));
        }
        let capacity_bypass = self.ingress_equivocations.len() >= MAX_INGRESS_SEMANTIC_KEYS;
        let protected_capacity_bypass = locked_commit_progress
            || locked_reproposal_prepare_progress
            || matches!(key, IngressSemanticKey::TimeoutVote { .. });
        if capacity_bypass && !protected_capacity_bypass {
            // This is bounded backpressure for ordinary semantic traffic. QCs
            // and TCs do not consume this table. The at-most-roster-sized exact
            // locked Commit, exact current-view locked-reproposal Prepare, and
            // bounded current/future TimeoutVote sets bypass ordinary capacity
            // and use their independent reserved progress partitions.
            return Ok((
                Some(Self::ignored_outcome(reducer::IgnoreReason::Busy)),
                None,
            ));
        }
        self.ingress_equivocations.insert(
            key,
            IngressEquivocationRecord {
                fingerprint,
                artifact,
                equivocation_reported: false,
                capacity_bypass,
                admitted_at: Instant::now(),
            },
        );
        let admission = IngressAdmission {
            key,
            fingerprint,
            consumer_tag: current_tag,
            inserted_equivocation: true,
            locked_commit_progress,
            locked_reproposal_prepare_progress,
        };
        if unsafe_proposal {
            self.record_ingress_delivery(admission);
            return Ok((
                Some(Self::ignored_outcome(reducer::IgnoreReason::UnsafeProposal)),
                None,
            ));
        }
        Ok((None, Some(admission)))
    }
    fn prune_ingress_records(&mut self) {
        let current_view = self.reducer.current_tag().view();
        let current_height = self.wire_context.height;
        let retained_vote_views = u64::try_from(self.wire_context.roster.len()).unwrap_or(u64::MAX);
        let oldest_retained_view = current_view.saturating_sub(retained_vote_views);
        let durable_lock = self.reducer.durable_state().locked().and_then(|locked| {
            Some((
                self.registry.round_to_wire(locked.round()),
                self.registry.subject(locked.subject()).ok()?,
                self.registry
                    .execution_commitment(locked.round(), locked.subject())
                    .ok()?,
            ))
        });
        let current_locked_reproposal = self.reducer.durable_state().locked().and_then(|locked| {
            let current_round = reducer::Round::new(current_height, current_view);
            if locked.round().height() != current_height || locked.round().view() >= current_view {
                return None;
            }
            let subject = self.registry.subject(locked.subject()).ok()?;
            let locked_commitment = self
                .registry
                .execution_commitment(locked.round(), locked.subject())
                .ok()?;
            let current_commitment = self
                .registry
                .execution_commitment(current_round, locked.subject())
                .ok()?;
            (locked_commitment == current_commitment).then_some((
                self.registry.round_to_wire(current_round),
                subject,
                current_commitment,
            ))
        });
        let matches_current_lock = |key: IngressSemanticKey, fingerprint: IngressFingerprint| {
            matches!(
                (key, fingerprint, durable_lock),
                (
                    IngressSemanticKey::Vote {
                        round,
                        phase: wire::GlobalPhase::Commit,
                        ..
                    },
                    IngressFingerprint::Vote(
                        proposal_round,
                        subject,
                        execution_commitment,
                    ),
                    Some((locked_round, locked_subject, locked_execution_commitment))
                ) if proposal_round == locked_round
                    && round.height == locked_round.height
                    && subject == locked_subject
                    && execution_commitment == locked_execution_commitment
            )
        };
        let matches_current_locked_reproposal =
            |key: IngressSemanticKey, fingerprint: IngressFingerprint| {
                matches!(
                    (key, fingerprint, current_locked_reproposal),
                    (
                        IngressSemanticKey::Vote {
                            round,
                            phase: wire::GlobalPhase::Prepare,
                            ..
                        },
                        IngressFingerprint::Vote(
                            proposal_round,
                            subject,
                            execution_commitment,
                        ),
                        Some((current_round, locked_subject, current_execution_commitment))
                    ) if round == current_round
                        && proposal_round == current_round
                        && subject == locked_subject
                        && execution_commitment == current_execution_commitment
                )
            };
        let matches_retained_timeout = |key: IngressSemanticKey| {
            matches!(
                key,
                IngressSemanticKey::TimeoutVote { round, .. }
                    if round.height == current_height
                        && reducer::timeout_vote_view_is_admissible(current_view, round.view)
            )
        };
        self.ingress_equivocations.retain(|key, record| {
            if record.capacity_bypass {
                matches_current_lock(*key, record.fingerprint)
                    || matches_current_locked_reproposal(*key, record.fingerprint)
                    || matches_retained_timeout(*key)
            } else {
                key.round().view >= oldest_retained_view
                    || matches_current_lock(*key, record.fingerprint)
            }
        });
        let equivocations = &self.ingress_equivocations;
        self.ingress_deliveries.retain(|key, delivery| {
            equivocations
                .get(key)
                .is_some_and(|record| record.fingerprint == delivery.fingerprint)
        });
    }
    fn ignored_outcome(reason: reducer::IgnoreReason) -> AdapterOutcome {
        AdapterOutcome {
            disposition: reducer::StepDisposition::Ignored(reason),
            effects: Vec::new(),
            deferred_admission_ordinal: None,
            producer_handoff: None,
        }
    }
    /// Feed a signature-checked and structurally verified canonical message.
    fn receive_verified(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        message.validate_version()?;
        let authenticated_wire_identity = Arc::<[u8]>::from(message.encode());
        let (outcome, admission) = self.admit_authenticated_payload(&message.payload)?;
        if let Some(outcome) = outcome {
            self.record_disposition(outcome.disposition());
            self.publish_status()?;
            return Ok(outcome);
        }
        let result =
            self.receive_admitted_payload(message.payload, admission, authenticated_wire_identity);
        if result.is_err()
            && let Some(admission) = admission
            && admission.inserted_equivocation
            && self
                .ingress_equivocations
                .get(&admission.key)
                .is_some_and(|record| record.fingerprint == admission.fingerprint)
        {
            self.ingress_equivocations.remove(&admission.key);
        }
        result
    }
    fn receive_admitted_payload(
        &mut self,
        payload: wire::ConsensusMessageV2Payload,
        admission: Option<IngressAdmission>,
        authenticated_wire_identity: Arc<[u8]>,
    ) -> Result<AdapterOutcome, AdapterError> {
        // Conversion is intentionally staged. A malformed value or a subject
        // collision must not leave attacker-controlled registry entries behind.
        // Semantic admission above bounds values which the reducer may defer.
        let mut registry = self.registry.clone();
        let tag = self.reducer.current_tag();
        match payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                let proposal = registry.proposal_to_core(&proposal, &self.wire_context)?;
                let round = proposal.proposal().round();
                let subject = proposal.proposal().manifest().subject();
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::ProposalReceived { tag, proposal },
                    Some((round, subject)),
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let vote = registry.vote_to_core(&vote, &self.wire_context)?;
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::VoteReceived { tag, vote },
                    None,
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                let certificate = registry.qc_to_core(&certificate, &self.wire_context)?;
                let active_subject = Some((certificate.proposal_round(), certificate.subject()));
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::QuorumCertificateReceived { tag, certificate },
                    active_subject,
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                let vote = registry.timeout_vote_to_core(&vote, &self.wire_context)?;
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::TimeoutVoteReceived { tag, vote },
                    None,
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                let certificate = registry.tc_to_core(&certificate, &self.wire_context)?;
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::TimeoutCertificateReceived { tag, certificate },
                    None,
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => {
                return Err(AdapterError::TransportPayload);
            }
        }
    }
    fn dispatch_staged_authenticated_ingress(
        &mut self,
        registry: WireRegistry,
        event: reducer::Event,
        active_subject: Option<(reducer::Round, reducer::Subject)>,
        admission: Option<IngressAdmission>,
        authenticated_wire_identity: Arc<[u8]>,
    ) -> Result<AdapterOutcome, AdapterError> {
        let previous_registry = core::mem::replace(&mut self.registry, registry);
        let previous_active_subject = self.active_subject;
        if let Some(active_subject) = active_subject {
            self.active_subject = Some(active_subject);
        }
        let result = self.step_authenticated_ingress_with_ownership(
            event,
            admission,
            Some(authenticated_wire_identity),
        );
        if result.is_err() {
            // A reducer failure after conversion may have partially consumed an
            // authenticated transition. Keep its registry expansion aligned
            // with reducer state and require WAL replay before further ingress.
            self.fail_closed = true;
            return result.map(|result| result.outcome);
        }
        let retain = result.as_ref().is_ok_and(|result| {
            result.outcome.disposition() == reducer::StepDisposition::Applied
                || (result.outcome.disposition()
                    == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
                    && result.outcome.deferred_admission_ordinal().is_some())
        });
        if !retain {
            self.registry = previous_registry;
            self.active_subject = previous_active_subject;
            self.publish_status()?;
        }
        result.map(|result| result.outcome)
    }
    /// Pass an authenticated canonical envelope to the reducer.
    pub(crate) fn receive_authenticated(
        &mut self,
        message: AuthenticatedConsensusMessage,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.receive_verified(message.0)
    }
    /// Notify the reducer that its one constant round timer expired.
    pub(crate) fn timeout_elapsed(
        &mut self,
        tag: reducer::EventTag,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        self.step(reducer::Event::TimeoutElapsed { tag })
    }
    /// Retry any missing proposal or certified body after the derived
    /// retransmission interval.
    pub(crate) fn retransmit_elapsed(
        &mut self,
        tag: reducer::EventTag,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        self.step(reducer::Event::RetransmitElapsed { tag })
    }
    /// Submit a locally assembled, durably stored, deterministically validated body.
    ///
    /// While the height is undecided, only the expected leader can take this
    /// transition: the reducer first persists its proposal intent and only
    /// then exposes signing. If an exact matching Decision became durable
    /// while assembly was completing, the trusted manifest and execution
    /// commitment instead transfer directly to decided-body application and
    /// the reducer emits `Apply` without creating proposal-only work.
    pub(crate) fn local_proposal_ready(
        &mut self,
        tag: reducer::EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: &DurableBodyReceipt,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        if durable_receipt.context_id() != self.wire_context.id()
            || durable_receipt.round() != manifest.round
            || durable_receipt.subject() != manifest.subject
            || durable_receipt.manifest_hash() != HashOf::new(&manifest)
            || validated_receipt.durable() != durable_receipt
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let completion_evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            validated_receipt: validated_receipt.clone(),
        };
        // Manifest conversion registers body identity, while execution
        // commitment registration binds the deterministic validation result.
        // Stage both mutations so a conflict cannot install half of the trust
        // boundary and influence a later completion.
        let mut staged_registry = self.registry.clone();
        let core_manifest = staged_registry.manifest_to_core(&manifest, &self.wire_context)?;
        let round = staged_registry.round_to_core(manifest.round, &self.wire_context)?;
        let subject = core_manifest.subject();
        staged_registry.register_execution_commitment(
            round,
            subject,
            validated_receipt.execution_commitment(),
        )?;
        self.registry = staged_registry;
        let previous_active_subject = self.active_subject;
        self.active_subject = Some((round, subject));
        let result = self.step_with_completion_evidence_and_status(
            reducer::Event::LocalProposalReady {
                tag,
                manifest: core_manifest,
            },
            Some(completion_evidence),
            false,
        );
        match &result {
            Ok(outcome) => {
                let retain = outcome.disposition() == reducer::StepDisposition::Applied
                    || (outcome.disposition()
                        == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
                        && outcome.deferred_admission_ordinal().is_some());
                if !retain {
                    self.active_subject = previous_active_subject;
                }
                // Publish after deciding whether this completion owns the active
                // subject. In particular, the no-effect stale path must not expose a
                // provisional subject to the monotone external progress clock.
                self.publish_status()?;
            }
            Err(_) => self.active_subject = previous_active_subject,
        }
        result
    }
    /// Bind an exact body-store validation marker into the wire registry.
    ///
    /// This monotone authority update is independent of the reducer consumer
    /// incarnation. A validation worker can finish after its view-local
    /// consumer was retired; the obsolete reducer event must remain a
    /// stutter, but the independently fsynced execution commitment must still
    /// release authenticated votes for this exact `(round, subject)`.
    pub(crate) fn bind_validated_body(
        &mut self,
        manifest: &wire::PayloadManifest,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<(), AdapterError> {
        self.ensure_ingress()?;
        let durable_receipt = validated_receipt.durable();
        if durable_receipt.context_id() != self.wire_context.id()
            || durable_receipt.round() != manifest.round
            || durable_receipt.subject() != manifest.subject
            || durable_receipt.manifest_hash() != HashOf::new(manifest)
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        validated_receipt.execution_commitment().validate()?;
        // Stage registry expansion so any mismatch leaves canonical authority
        // unchanged. Registration is idempotent for the exact receipt and
        // rejects a conflicting commitment before mutation.
        let mut registry = self.registry.clone();
        let core_manifest = registry.manifest_to_core(manifest, &self.wire_context)?;
        let round = registry.round_to_core(manifest.round, &self.wire_context)?;
        registry.register_execution_commitment(
            round,
            core_manifest.subject(),
            validated_receipt.execution_commitment(),
        )?;
        self.registry = registry;
        Ok(())
    }
    /// Restore a body-store validation marker into the replayed wire registry.
    ///
    /// Proposal intent persistence deliberately precedes signing. On restart,
    /// the safety WAL reconstructs that intent while the exact execution
    /// commitment remains in the independently fsynced body store. Reassociating
    /// those same-round durable records before dispatching startup effects lets
    /// the replayed proposal continue directly into its Prepare vote.
    pub(crate) fn recover_validated_body(
        &mut self,
        manifest: &wire::PayloadManifest,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<(), AdapterError> {
        self.bind_validated_body(manifest, validated_receipt)
    }
    /// Stage the fixed recovered Decision body fast-forward on this owned cold adapter.
    ///
    /// The non-forgeable body-cut permit and opaque Decision-Fetch projection
    /// prevent arbitrary callers from submitting manifest, receipt, or effect
    /// parts. The existing direct previews thread cloned reducer/registry state
    /// through this unpublished owned adapter, rolling the exact cold state
    /// back on every typed failure. No intermediate Store or Validate effect
    /// can escape.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_recovered_decision_apply_fast_forward(
        mut self,
        permit: RecoveredDecisionApplyAdapterPreviewPermit,
        verified: &VerifiedHeightContext,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
        manifest: &wire::PayloadManifest,
        durable: &DurableBodyReceipt,
        validated: &ValidatedBodyReceipt,
    ) -> Result<RecoveredDecisionApplyStagedAdapterV1, RecoveredDecisionApplyAdapterStagingError>
    {
        macro_rules! fail_unstaged {
            ($error:expr) => {
                return Err(RecoveredDecisionApplyAdapterStagingError::retain(
                    $error, self,
                ))
            };
        }
        macro_rules! fail_staged {
            ($rollback:expr, $error:expr) => {{
                $rollback.restore(&mut self);
                return Err(RecoveredDecisionApplyAdapterStagingError::retain(
                    $error, self,
                ));
            }};
        }
        if verified.context() != &self.wire_context
            || durable.context_id() != self.wire_context.id()
            || durable.round() != manifest.round
            || durable.subject() != manifest.subject
            || durable.manifest_hash() != HashOf::new(manifest)
            || validated.durable() != durable
        {
            fail_unstaged!(AdapterError::RecoveredDecisionApplyFastForwardMismatch);
        }
        let Some(decision) = self.reducer.durable_state().decision().cloned() else {
            fail_unstaged!(AdapterError::RecoveredDecisionApplyFastForwardMismatch);
        };
        let mut comparison_registry = self.registry.clone();
        let certificate = match comparison_registry.qc_to_wire(&decision, self.aggregator.as_ref())
        {
            Ok(certificate) => certificate,
            Err(error) => fail_unstaged!(error),
        };
        let expected_fetch = AdapterEffect::FetchBody {
            tag: self.current_tag(),
            round: certificate.proposal_round,
            subject: certificate.subject,
            manifest: None,
            certified_sources: self
                .wire_context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect(),
            certificate: Some(certificate.clone()),
        };
        if certificate.phase != wire::GlobalPhase::Commit
            || certificate.proposal_round != manifest.round
            || certificate.subject != manifest.subject
            || certificate.execution_commitment != validated.execution_commitment()
            || !projection.matches_fast_forward_fetch(&permit, verified, &expected_fetch)
        {
            fail_unstaged!(AdapterError::RecoveredDecisionApplyFastForwardMismatch);
        }
        let tag = self.current_tag();
        let store_prepared = match self.prepare_direct_certified_body_available(tag, manifest) {
            Ok(DirectCertifiedBodyAvailablePreparation::Applied(prepared)) => prepared,
            Ok(other) => {
                drop(other);
                fail_unstaged!(AdapterError::RecoveredDecisionApplyFastForwardMismatch);
            }
            Err(error) => fail_unstaged!(error),
        };
        let PreparedDirectCertifiedBodyAvailable {
            adapter: _,
            next_reducer: store_reducer,
            next_registry: store_registry,
            event: store_event,
            core_effect: store_core_effect,
            store_effect,
            next_fence_generation: store_fence_generation,
        } = store_prepared;
        let rollback = RecoveredDecisionApplyAdapterRollback {
            reducer: std::mem::replace(&mut self.reducer, store_reducer),
            registry: std::mem::replace(&mut self.registry, store_registry),
            reducer_fence_generation: std::mem::replace(
                &mut self.reducer_fence_generation,
                store_fence_generation,
            ),
            last_progress: self.last_progress.clone(),
        };
        let validate_prepared =
            match self.prepare_direct_body_stored(tag, manifest.round, manifest.subject, durable) {
                Ok(DirectBodyStoredPreparation::Applied(prepared)) => prepared,
                Ok(other) => {
                    drop(other);
                    fail_staged!(
                        rollback,
                        AdapterError::RecoveredDecisionApplyFastForwardMismatch
                    );
                }
                Err(error) => fail_staged!(rollback, error),
            };
        let PreparedDirectBodyStored {
            adapter: _,
            next_reducer: validate_reducer,
            next_registry: validate_registry,
            event: validate_event,
            core_effect: validate_core_effect,
            validate_effect,
            next_fence_generation: validate_fence_generation,
        } = validate_prepared;
        self.reducer = validate_reducer;
        self.registry = validate_registry;
        self.reducer_fence_generation = validate_fence_generation;
        let apply_prepared = match self.prepare_direct_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            validated,
        ) {
            Ok(DirectValidationSucceededPreparation::Apply(prepared)) => prepared,
            Ok(other) => {
                drop(other);
                fail_staged!(
                    rollback,
                    AdapterError::RecoveredDecisionApplyFastForwardMismatch
                );
            }
            Err(error) => fail_staged!(rollback, error),
        };
        let PreparedDirectValidationSucceededApply {
            _adapter: _,
            next_reducer: apply_reducer,
            next_registry: apply_registry,
            event: apply_event,
            core_effect: apply_core_effect,
            apply_effect,
            next_fence_generation: apply_fence_generation,
        } = apply_prepared;
        if !matches!(
            &apply_effect,
            AdapterEffect::Apply {
                tag: apply_tag,
                subject: apply_subject,
                certificate: apply_certificate,
            } if *apply_tag == tag
                && *apply_subject == manifest.subject
                && apply_certificate == &certificate
        ) {
            fail_staged!(
                rollback,
                AdapterError::RecoveredDecisionApplyFastForwardMismatch
            );
        }
        let Some(pending) = projection.project_decision_apply_pending_lineage(
            &permit,
            verified,
            &expected_fetch,
            &store_effect,
            &validate_effect,
            &apply_effect,
        ) else {
            fail_staged!(
                rollback,
                AdapterError::RecoveredDecisionApplyFastForwardMismatch
            );
        };
        self.reducer = apply_reducer;
        self.registry = apply_registry;
        self.reducer_fence_generation = apply_fence_generation;
        let mut preview = RecoveredDecisionApplyStagedAdapterV1 {
            adapter: self,
            store_effect,
            validate_effect,
            apply_effect,
            pending,
        };
        if !preview.validates() {
            let RecoveredDecisionApplyStagedAdapterV1 {
                mut adapter,
                store_effect: _,
                validate_effect: _,
                apply_effect: _,
                pending: _,
            } = preview;
            rollback.restore(&mut adapter);
            return Err(RecoveredDecisionApplyAdapterStagingError::retain(
                AdapterError::RecoveredDecisionApplyFastForwardMismatch,
                adapter,
            ));
        }
        drop(rollback);
        for (event, effect) in [
            (&store_event, &store_core_effect),
            (&validate_event, &validate_core_effect),
            (&apply_event, &apply_core_effect),
        ] {
            preview.adapter.record_reducer_outcome(
                event,
                reducer::StepDisposition::Applied,
                core::slice::from_ref(effect),
            );
            preview
                .adapter
                .log_body_progress(event, reducer::StepDisposition::Applied, 1);
        }
        Ok(preview)
    }
    /// Preview the exact recovered Decision `BodyAvailable -> StoreBody` transition.
    ///
    /// The dedicated authority preserves the recovered WAL identity and opaque
    /// body receipt while the resulting token exclusively borrows the adapter
    /// until durable settlement either publishes or abandons the preview.
    pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_store(
        &mut self,
        authority: super::v2_lifecycle_coordinator::RecoveredDecisionFetchStoreAdapterAuthorityV1,
    ) -> Result<PreparedRecoveredDecisionFetchStoreAdapterV1<'_>, AdapterError> {
        let tag = authority.tag();
        let prepared =
            match self.prepare_direct_certified_body_available(tag, authority.manifest())? {
                DirectCertifiedBodyAvailablePreparation::Applied(prepared) => prepared,
                DirectCertifiedBodyAvailablePreparation::Blocked(_)
                | DirectCertifiedBodyAvailablePreparation::Inactive(_) => {
                    return Err(AdapterError::RecoveredDecisionFetchStoreMismatch);
                }
            };
        Ok(PreparedRecoveredDecisionFetchStoreAdapterV1 {
            preview: prepared,
            body: authority.into_body(),
        })
    }
    /// Preview one ordinary certified Fetch-to-Store lifecycle transition.
    ///
    /// The result never exposes raw reducer state.  Applied work remains
    /// borrow-bound until the registry and LedgerV1 transaction publish it;
    /// Busy returns the exact reducer-fence generation; every other inactive
    /// disposition is fail-closed because cold open reconstructed this carrier
    /// as live before scheduling it.
    pub(in crate::sumeragi) fn prepare_certified_fetch_store(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<CertifiedFetchStoreAdapterPreparationV1<'_>, AdapterError> {
        match self.prepare_direct_certified_body_available(tag, manifest)? {
            DirectCertifiedBodyAvailablePreparation::Applied(preview) => {
                Ok(CertifiedFetchStoreAdapterPreparationV1::Applied(
                    PreparedCertifiedFetchStoreAdapterV1 { preview },
                ))
            }
            DirectCertifiedBodyAvailablePreparation::Blocked(wait) => {
                Ok(CertifiedFetchStoreAdapterPreparationV1::Blocked(wait))
            }
            DirectCertifiedBodyAvailablePreparation::Inactive(inactive) => {
                drop(inactive);
                Ok(CertifiedFetchStoreAdapterPreparationV1::Inactive)
            }
        }
    }
    /// Preview one ordinary durable Store-to-Validate lifecycle transition.
    pub(in crate::sumeragi) fn prepare_durable_store_validate(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<DurableStoreValidateAdapterPreparationV1<'_>, AdapterError> {
        match self.prepare_direct_body_stored(tag, round, subject, receipt)? {
            DirectBodyStoredPreparation::Applied(preview) => {
                Ok(DurableStoreValidateAdapterPreparationV1::Applied(
                    PreparedDurableStoreValidateAdapterV1 { preview },
                ))
            }
            DirectBodyStoredPreparation::Blocked(wait) => {
                Ok(DurableStoreValidateAdapterPreparationV1::Blocked(wait))
            }
            DirectBodyStoredPreparation::Inactive(inactive) => {
                drop(inactive);
                Ok(DurableStoreValidateAdapterPreparationV1::Inactive)
            }
        }
    }
    /// Preview a certified Fetch completion directly against the sole reducer.
    ///
    /// No adapter-owned deferred queue, serviced-candidate marker, producer
    /// reservation, WAL publication, or concrete lifecycle work is changed by
    /// this method. An applied result retains the exclusive adapter borrow and
    /// carries the exact cloned reducer/registry state needed by the live
    /// output-permitted lifecycle transaction. `Busy` instead returns the
    /// monotone fence generation which that transaction must place in its
    /// explicit coordinator wait token.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or foreign manifest material, reducer
    /// refinement failure, fence-generation exhaustion, or any reducer effect
    /// shape other than the closed `BodyAvailable -> StoreBody` contract.
    #[cfg_attr(not(test), allow(dead_code))]
    fn prepare_direct_certified_body_available(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<DirectCertifiedBodyAvailablePreparation<'_>, AdapterError> {
        self.ensure_ingress()?;
        manifest.validate(&self.wire_context)?;
        let mut next_registry = self.registry.clone();
        let round = next_registry.round_to_core(manifest.round, &self.wire_context)?;
        let subject = next_registry.register_subject(manifest.subject)?;
        let core_manifest = next_registry.manifest_to_core(manifest, &self.wire_context)?;
        if core_manifest.subject() != subject {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let event = reducer::Event::BodyAvailable {
            tag,
            round,
            subject,
        };
        let mut next_reducer = self.reducer.clone();
        let outcome = next_reducer.step(event.clone())?;
        let disposition = outcome.disposition();
        let core_effects = outcome.into_effects();
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            if !core_effects.is_empty() {
                return Err(AdapterError::DirectCertifiedBodyAvailableContractViolation);
            }
            if self.reducer_fence_generation == u64::MAX {
                return Err(AdapterError::ReducerFenceGenerationExhausted);
            }
            return Ok(DirectCertifiedBodyAvailablePreparation::Blocked(
                PreparedReducerFenceWait {
                    context_id: self.wire_context.id(),
                    generation: self.reducer_fence_generation,
                    _adapter: self,
                },
            ));
        }
        if let reducer::StepDisposition::Ignored(reason) = disposition {
            if !core_effects.is_empty() {
                return Err(AdapterError::DirectCertifiedBodyAvailableContractViolation);
            }
            let disposition = match reason {
                reducer::IgnoreReason::NoMatchingWork => {
                    DirectCertifiedBodyAvailableInactive::Stutter(
                        DirectCertifiedBodyAvailableStutter::NoMatchingWork,
                    )
                }
                reducer::IgnoreReason::Duplicate => DirectCertifiedBodyAvailableInactive::Stutter(
                    DirectCertifiedBodyAvailableStutter::Duplicate,
                ),
                reason => DirectCertifiedBodyAvailableInactive::Superseded(reason),
            };
            return Ok(DirectCertifiedBodyAvailablePreparation::Inactive(
                PreparedDirectCertifiedBodyAvailableInactive {
                    _adapter: self,
                    disposition,
                },
            ));
        }
        let store_effect = match core_effects.as_slice() {
            [
                reducer::Effect::StoreBody {
                    tag: effect_tag,
                    round: effect_round,
                    subject: effect_subject,
                },
            ] if *effect_tag == tag && *effect_round == round && *effect_subject == subject => {
                AdapterEffect::StoreBody {
                    tag: *effect_tag,
                    round: next_registry.round_to_wire(*effect_round),
                    subject: next_registry.subject(*effect_subject)?,
                }
            }
            _ => return Err(AdapterError::DirectCertifiedBodyAvailableContractViolation),
        };
        let core_effect = core_effects
            .into_iter()
            .next()
            .expect("validated direct completion emits exactly one StoreBody effect");
        let next_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: self.replay_complete,
        };
        let next_fence_generation = if next_fence == self.reducer_fence_projection() {
            self.reducer_fence_generation
        } else {
            self.reducer_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
        };
        Ok(DirectCertifiedBodyAvailablePreparation::Applied(
            PreparedDirectCertifiedBodyAvailable {
                adapter: self,
                next_reducer,
                next_registry,
                event,
                core_effect,
                store_effect,
                next_fence_generation,
            },
        ))
    }
    /// Preview one durable Store completion directly against the sole reducer.
    ///
    /// The receipt is rebound to the exact registered manifest before any
    /// reducer work. No adapter-owned deferred queue, serviced-candidate
    /// marker, producer continuation, WAL publication, or concrete lifecycle
    /// work is changed. An applied result holds the exclusive adapter borrow
    /// together with the exact cloned reducer/registry state; `Busy` instead
    /// retains that borrow in the existing monotone reducer-fence wait token.
    ///
    /// # Errors
    ///
    /// Returns an error for a foreign or mismatched durable receipt, reducer
    /// refinement failure, fence-generation exhaustion, or any reducer effect
    /// shape other than the closed `BodyStored -> ValidateBody` contract.
    #[cfg_attr(not(test), allow(dead_code))]
    fn prepare_direct_body_stored(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<DirectBodyStoredPreparation<'_>, AdapterError> {
        self.ensure_ingress()?;
        if receipt.context_id() != self.wire_context.id()
            || receipt.round() != round
            || receipt.subject() != subject
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let mut next_registry = self.registry.clone();
        let core_round = next_registry.round_to_core(round, &self.wire_context)?;
        let core_subject = next_registry.register_subject(subject)?;
        let manifest = next_registry
            .manifests
            .get(&(core_round, core_subject))
            .ok_or(AdapterError::MissingManifest)?;
        if receipt.manifest_hash() != HashOf::new(manifest) {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let event = reducer::Event::BodyStored {
            tag,
            round: core_round,
            subject: core_subject,
        };
        let mut next_reducer = self.reducer.clone();
        let outcome = next_reducer.step(event.clone())?;
        let disposition = outcome.disposition();
        let core_effects = outcome.into_effects();
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            if !core_effects.is_empty() {
                return Err(AdapterError::DirectBodyStoredContractViolation);
            }
            if self.reducer_fence_generation == u64::MAX {
                return Err(AdapterError::ReducerFenceGenerationExhausted);
            }
            return Ok(DirectBodyStoredPreparation::Blocked(
                PreparedReducerFenceWait {
                    context_id: self.wire_context.id(),
                    generation: self.reducer_fence_generation,
                    _adapter: self,
                },
            ));
        }
        if let reducer::StepDisposition::Ignored(reason) = disposition {
            if !core_effects.is_empty() {
                return Err(AdapterError::DirectBodyStoredContractViolation);
            }
            let disposition = match reason {
                reducer::IgnoreReason::NoMatchingWork => {
                    DirectBodyStoredInactive::Stutter(DirectBodyStoredStutter::NoMatchingWork)
                }
                reducer::IgnoreReason::Duplicate => {
                    DirectBodyStoredInactive::Stutter(DirectBodyStoredStutter::Duplicate)
                }
                reason => DirectBodyStoredInactive::Superseded(reason),
            };
            return Ok(DirectBodyStoredPreparation::Inactive(
                PreparedDirectBodyStoredInactive {
                    _adapter: self,
                    disposition,
                },
            ));
        }
        let validate_effect = match core_effects.as_slice() {
            [
                reducer::Effect::ValidateBody {
                    tag: effect_tag,
                    round: effect_round,
                    subject: effect_subject,
                },
            ] if *effect_tag == tag
                && *effect_round == core_round
                && *effect_subject == core_subject =>
            {
                AdapterEffect::ValidateBody {
                    tag: *effect_tag,
                    round: next_registry.round_to_wire(*effect_round),
                    subject: next_registry.subject(*effect_subject)?,
                }
            }
            _ => return Err(AdapterError::DirectBodyStoredContractViolation),
        };
        let core_effect = core_effects
            .into_iter()
            .next()
            .expect("validated durable-body completion emits exactly one ValidateBody effect");
        let next_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: self.replay_complete,
        };
        let next_fence_generation = if next_fence == self.reducer_fence_projection() {
            self.reducer_fence_generation
        } else {
            self.reducer_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
        };
        Ok(DirectBodyStoredPreparation::Applied(
            PreparedDirectBodyStored {
                adapter: self,
                next_reducer,
                next_registry,
                event,
                core_effect,
                validate_effect,
                next_fence_generation,
            },
        ))
    }
    fn stage_direct_validation_registry(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        durable_receipt: &DurableBodyReceipt,
        local_origin_manifest: Option<&wire::PayloadManifest>,
    ) -> Result<(WireRegistry, reducer::Round, reducer::Subject), AdapterError> {
        if durable_receipt.context_id() != self.wire_context.id()
            || durable_receipt.round() != round
            || durable_receipt.subject() != subject
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let mut next_registry = self.registry.clone();
        let core_round = next_registry.round_to_core(round, &self.wire_context)?;
        let core_subject = next_registry.register_subject(subject)?;
        if let Some(manifest) = local_origin_manifest {
            if manifest.round != round
                || manifest.subject != subject
                || durable_receipt.manifest_hash() != HashOf::new(manifest)
            {
                return Err(AdapterError::DurableBodyMismatch);
            }
            let core_manifest = next_registry.manifest_to_core(manifest, &self.wire_context)?;
            if core_manifest.subject() != core_subject {
                return Err(AdapterError::DurableBodyMismatch);
            }
        }
        let manifest = next_registry
            .manifests
            .get(&(core_round, core_subject))
            .ok_or(AdapterError::MissingManifest)?;
        if durable_receipt.manifest_hash() != HashOf::new(manifest) {
            return Err(AdapterError::DurableBodyMismatch);
        }
        Ok((next_registry, core_round, core_subject))
    }
    /// Rebind an exact durable-body validation completion to the reducer's
    /// current view after its asynchronous worker outlives an ordinary view
    /// advance.
    ///
    /// Receipt, manifest, round, and subject authentication has already
    /// completed in [`Self::stage_direct_validation_registry`]. The rebinding
    /// is deliberately narrower than the reducer's generic event-tag rules:
    /// only a lower-view tag from the same height may acknowledge the exact
    /// body work which is still `Durable`. Generation is local to one view and
    /// resets on an ordinary view advance, so it cannot be compared across the
    /// strict lower-view boundary. Every other tag remains unchanged and
    /// therefore reaches the normal fail-closed `WrongHeight`,
    /// `StaleGeneration`, or `WrongView` classification.
    fn validation_completion_tag(
        &self,
        tag: reducer::EventTag,
        round: reducer::Round,
        subject: reducer::Subject,
    ) -> reducer::EventTag {
        let current = self.reducer.current_tag();
        if tag.height() == current.height()
            && tag.view() < current.view()
            && self.reducer.body_state(round, subject) == reducer::BodyState::Durable
        {
            current
        } else {
            tag
        }
    }
    /// Preview one exact successful deterministic validation directly against
    /// cloned reducer and wire-registry state.
    ///
    /// The validation marker is already independently durable. Preparation
    /// therefore registers its execution commitment in every staged result,
    /// even when a reducer fence or obsolete consumer prevents a child effect.
    /// No live adapter state, safety WAL, or lifecycle work changes here.
    ///
    /// # Errors
    ///
    /// Returns an error for a foreign body receipt, a missing or conflicting
    /// manifest/commitment, reducer refinement failure, fence-generation
    /// exhaustion, or any effect shape outside the closed successful-validation
    /// inventory.
    #[cfg_attr(not(test), allow(dead_code))]
    fn prepare_direct_validation_succeeded(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<DirectValidationSucceededPreparation<'_>, AdapterError> {
        self.prepare_direct_validation_succeeded_with_local_origin_manifest(
            tag,
            round,
            subject,
            validated_receipt,
            None,
        )
    }
    fn prepare_direct_validation_succeeded_with_local_origin_manifest(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        validated_receipt: &ValidatedBodyReceipt,
        local_origin_manifest: Option<&wire::PayloadManifest>,
    ) -> Result<DirectValidationSucceededPreparation<'_>, AdapterError> {
        self.ensure_ingress()?;
        let durable_receipt = validated_receipt.durable();
        let (mut next_registry, core_round, core_subject) = self.stage_direct_validation_registry(
            round,
            subject,
            durable_receipt,
            local_origin_manifest,
        )?;
        validated_receipt.execution_commitment().validate()?;
        next_registry.register_execution_commitment(
            core_round,
            core_subject,
            validated_receipt.execution_commitment(),
        )?;
        let tag = self.validation_completion_tag(tag, core_round, core_subject);
        let reducer_fence_generation = self.reducer_fence_generation;
        if reducer_fence_generation == u64::MAX {
            return Err(AdapterError::ReducerFenceGenerationExhausted);
        }
        let event = reducer::Event::ValidationCompleted {
            tag,
            round: core_round,
            subject: core_subject,
            valid: true,
        };
        let mut next_reducer = self.reducer.clone();
        let outcome = next_reducer.step(event.clone())?;
        let disposition = outcome.disposition();
        let mut core_effects = outcome.into_effects();
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            if !core_effects.is_empty() {
                return Err(AdapterError::DirectValidationSucceededContractViolation);
            }
            return Ok(DirectValidationSucceededPreparation::Busy(
                PreparedDirectValidationSucceededBusy {
                    _adapter: self,
                    next_registry,
                    context_id: round.context_id,
                    generation: reducer_fence_generation,
                },
            ));
        }
        if let reducer::StepDisposition::Ignored(reason) = disposition {
            if !core_effects.is_empty() {
                return Err(AdapterError::DirectValidationSucceededContractViolation);
            }
            let next_fence = ReducerFenceProjection {
                pending_persistence: next_reducer.pending_persistence_record().cloned(),
                awaiting_signature: next_reducer.awaiting_signature().cloned(),
                replay_complete: self.replay_complete,
            };
            let next_fence_generation = if next_fence == self.reducer_fence_projection() {
                reducer_fence_generation
            } else {
                reducer_fence_generation
                    .checked_add(1)
                    .filter(|next| *next != u64::MAX)
                    .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
            };
            let disposition = match reason {
                reducer::IgnoreReason::NoMatchingWork => {
                    DirectValidationSucceededInactive::Stutter(
                        DirectValidationSucceededStutter::NoMatchingWork,
                    )
                }
                reducer::IgnoreReason::Duplicate => DirectValidationSucceededInactive::Stutter(
                    DirectValidationSucceededStutter::Duplicate,
                ),
                reason => DirectValidationSucceededInactive::Superseded(reason),
            };
            return Ok(DirectValidationSucceededPreparation::Inactive(
                PreparedDirectValidationSucceededInactive {
                    _adapter: self,
                    next_reducer,
                    next_registry,
                    event,
                    disposition,
                    next_fence_generation,
                },
            ));
        }
        if disposition != reducer::StepDisposition::Applied {
            return Err(AdapterError::DirectValidationSucceededContractViolation);
        }
        let next_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: self.replay_complete,
        };
        let next_fence_generation = if next_fence == self.reducer_fence_projection() {
            reducer_fence_generation
        } else {
            reducer_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
        };
        match core_effects.as_slice() {
            [] => Ok(DirectValidationSucceededPreparation::NoEffect(
                PreparedDirectValidationSucceededNoEffect {
                    _adapter: self,
                    next_reducer,
                    next_registry,
                    event,
                    next_fence_generation,
                },
            )),
            [
                reducer::Effect::Apply {
                    tag: effect_tag,
                    subject: effect_subject,
                    certificate,
                },
            ] if *effect_tag == tag
                && *effect_subject == core_subject
                && certificate.subject() == core_subject
                && certificate.proposal_round() == core_round
                && next_reducer.durable_state().decision() == Some(certificate) =>
            {
                let wire_certificate =
                    next_registry.qc_to_wire(certificate, self.aggregator.as_ref())?;
                if wire_certificate.subject != subject || wire_certificate.proposal_round != round {
                    return Err(AdapterError::DirectValidationSucceededContractViolation);
                }
                let apply_effect = AdapterEffect::Apply {
                    tag: *effect_tag,
                    subject,
                    certificate: wire_certificate,
                };
                let core_effect = core_effects
                    .pop()
                    .expect("validated direct completion has one Apply effect");
                Ok(DirectValidationSucceededPreparation::Apply(
                    PreparedDirectValidationSucceededApply {
                        _adapter: self,
                        next_reducer,
                        next_registry,
                        event,
                        core_effect,
                        apply_effect,
                        next_fence_generation,
                    },
                ))
            }
            [
                reducer::Effect::Persist {
                    tag: effect_tag,
                    entry,
                },
            ] if *effect_tag == tag
                && next_reducer.pending_persistence_record() == Some(entry.record()) =>
            {
                let persist_effect = core_effects
                    .pop()
                    .expect("validated direct completion has one Persist effect");
                Ok(DirectValidationSucceededPreparation::Persist(
                    PreparedDirectValidationSucceededPersist {
                        _adapter: self,
                        next_reducer,
                        next_registry,
                        event,
                        persist_effect,
                        next_fence_generation,
                    },
                ))
            }
            _ => Err(AdapterError::DirectValidationSucceededContractViolation),
        }
    }
    /// Preview one exact deterministic rejection directly against cloned
    /// reducer and wire-registry state.
    ///
    /// The supplied receipt is rebound to this adapter's frozen height context
    /// and independently registered manifest before the reducer can consume
    /// the rejection. Preparation exposes neither persistence nor lifecycle
    /// machinery and leaves every live adapter field unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error for a foreign body receipt, a missing or conflicting
    /// manifest, reducer refinement failure, fence-generation exhaustion, or
    /// any effect shape outside the closed failed-validation inventory.
    #[cfg_attr(not(test), allow(dead_code))]
    fn prepare_direct_validation_failed(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<DirectValidationFailedPreparation<'_>, AdapterError> {
        self.prepare_direct_validation_failed_with_local_origin_manifest(
            tag,
            round,
            subject,
            durable_receipt,
            None,
        )
    }
    fn prepare_direct_validation_failed_with_local_origin_manifest(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        durable_receipt: &DurableBodyReceipt,
        local_origin_manifest: Option<&wire::PayloadManifest>,
    ) -> Result<DirectValidationFailedPreparation<'_>, AdapterError> {
        self.ensure_ingress()?;
        let (mut next_registry, core_round, core_subject) = self.stage_direct_validation_registry(
            round,
            subject,
            durable_receipt,
            local_origin_manifest,
        )?;
        let tag = self.validation_completion_tag(tag, core_round, core_subject);
        let reducer_fence_generation = self.reducer_fence_generation;
        if reducer_fence_generation == u64::MAX {
            return Err(AdapterError::ReducerFenceGenerationExhausted);
        }
        let event = reducer::Event::ValidationCompleted {
            tag,
            round: core_round,
            subject: core_subject,
            valid: false,
        };
        let mut next_reducer = self.reducer.clone();
        let outcome = next_reducer.step(event.clone())?;
        let disposition = outcome.disposition();
        let mut core_effects = outcome.into_effects();
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            if !core_effects.is_empty() {
                return Err(AdapterError::DirectValidationFailedContractViolation);
            }
            return Ok(DirectValidationFailedPreparation::Busy(
                PreparedDirectValidationFailedBusy {
                    _adapter: self,
                    next_registry,
                    context_id: round.context_id,
                    generation: reducer_fence_generation,
                },
            ));
        }
        if let reducer::StepDisposition::Ignored(reason) = disposition {
            if !core_effects.is_empty() {
                return Err(AdapterError::DirectValidationFailedContractViolation);
            }
            let next_fence = ReducerFenceProjection {
                pending_persistence: next_reducer.pending_persistence_record().cloned(),
                awaiting_signature: next_reducer.awaiting_signature().cloned(),
                replay_complete: self.replay_complete,
            };
            let next_fence_generation = if next_fence == self.reducer_fence_projection() {
                reducer_fence_generation
            } else {
                reducer_fence_generation
                    .checked_add(1)
                    .filter(|next| *next != u64::MAX)
                    .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
            };
            let disposition = match reason {
                reducer::IgnoreReason::NoMatchingWork => DirectValidationFailedInactive::Stutter(
                    DirectValidationFailedStutter::NoMatchingWork,
                ),
                reducer::IgnoreReason::Duplicate => DirectValidationFailedInactive::Stutter(
                    DirectValidationFailedStutter::Duplicate,
                ),
                reason => DirectValidationFailedInactive::Superseded(reason),
            };
            return Ok(DirectValidationFailedPreparation::Inactive(
                PreparedDirectValidationFailedInactive {
                    _adapter: self,
                    next_reducer,
                    next_registry,
                    event,
                    disposition,
                    next_fence_generation,
                },
            ));
        }
        if disposition != reducer::StepDisposition::Applied {
            return Err(AdapterError::DirectValidationFailedContractViolation);
        }
        let next_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: self.replay_complete,
        };
        let next_fence_generation = if next_fence == self.reducer_fence_projection() {
            reducer_fence_generation
        } else {
            reducer_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
        };
        match core_effects.as_slice() {
            [] => Ok(DirectValidationFailedPreparation::NoEffect(
                PreparedDirectValidationFailedNoEffect {
                    _adapter: self,
                    next_reducer,
                    next_registry,
                    event,
                    next_fence_generation,
                },
            )),
            [
                reducer::Effect::ReportInvalidCertifiedBody {
                    subject: effect_subject,
                    certificate,
                },
            ] if *effect_subject == core_subject
                && certificate.reference()
                    == reducer::CertificateRef::new(
                        next_reducer.context().id(),
                        core_round,
                        reducer::Phase::Prepare,
                        core_subject,
                    ) =>
            {
                let registered_certificate = next_registry
                    .certificates
                    .get(&certificate.reference())
                    .cloned()
                    .ok_or(AdapterError::DirectValidationFailedContractViolation)?;
                let wire_certificate =
                    next_registry.qc_to_wire(certificate, self.aggregator.as_ref())?;
                if wire_certificate != registered_certificate
                    || wire_certificate.round != round
                    || wire_certificate.proposal_round != round
                    || wire_certificate.phase != wire::GlobalPhase::Prepare
                    || wire_certificate.subject != subject
                {
                    return Err(AdapterError::DirectValidationFailedContractViolation);
                }
                let report_effect = AdapterEffect::ReportInvalidCertifiedBody {
                    subject,
                    certificate: wire_certificate,
                };
                let core_effect = core_effects
                    .pop()
                    .expect("failed direct validation has one PrepareQC report effect");
                Ok(DirectValidationFailedPreparation::Report(
                    PreparedDirectValidationFailedReport {
                        _adapter: self,
                        next_reducer,
                        next_registry,
                        event,
                        core_effect,
                        report_effect,
                        next_fence_generation,
                    },
                ))
            }
            _ => Err(AdapterError::DirectValidationFailedContractViolation),
        }
    }
    // READY_DURABLE_VALIDATE_ADAPTER_BRIDGE_BEGIN
    /// Preview one successful Ready Validate completion from sealed registry authority.
    ///
    /// The authority is constructible only while the exact completion remains
    /// exclusively borrowed by the registry-owned fixed join. Consuming it
    /// yields an opaque adapter token with no receipt or reducer-event accessor.
    pub(crate) fn prepare_sealed_ready_durable_validate_succeeded<'adapter>(
        &'adapter mut self,
        authority: ReadyValidatedAdapterAuthority<'_>,
    ) -> Result<SealedReadyDurableValidateAdapterPreview<'adapter>, AdapterError> {
        let (tag, round, subject, receipt, local_origin_manifest) = authority.into_parts();
        self.prepare_direct_validation_succeeded_with_local_origin_manifest(
            tag,
            round,
            subject,
            receipt,
            local_origin_manifest.as_ref(),
        )
        .map(|preview| {
            SealedReadyDurableValidateAdapterPreview(match preview {
                DirectValidationSucceededPreparation::Busy(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::ValidatedBusy(adapter)
                }
                DirectValidationSucceededPreparation::Inactive(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::ValidatedInactive(adapter)
                }
                DirectValidationSucceededPreparation::NoEffect(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::ValidatedNoEffect(adapter)
                }
                DirectValidationSucceededPreparation::Apply(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::ValidatedApply(adapter)
                }
                DirectValidationSucceededPreparation::Persist(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::ValidatedPersist(adapter)
                }
            })
        })
    }
    /// Preview one rejected Ready Validate completion from sealed registry authority.
    ///
    /// Diagnostic rejection text never crosses this boundary. The registry
    /// authority proves the canonical rejection identity before this method
    /// can stage the closed direct-reducer classifications.
    pub(crate) fn prepare_sealed_ready_durable_validate_failed<'adapter>(
        &'adapter mut self,
        authority: ReadyRejectedAdapterAuthority<'_>,
    ) -> Result<SealedReadyDurableValidateAdapterPreview<'adapter>, AdapterError> {
        let (tag, round, subject, receipt, local_origin_manifest) = authority.into_parts();
        self.prepare_direct_validation_failed_with_local_origin_manifest(
            tag,
            round,
            subject,
            receipt,
            local_origin_manifest.as_ref(),
        )
        .map(|preview| {
            SealedReadyDurableValidateAdapterPreview(match preview {
                DirectValidationFailedPreparation::Busy(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::RejectedBusy(adapter)
                }
                DirectValidationFailedPreparation::Inactive(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::RejectedInactive(adapter)
                }
                DirectValidationFailedPreparation::NoEffect(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::RejectedNoEffect(adapter)
                }
                DirectValidationFailedPreparation::Report(adapter) => {
                    ReadyDurableValidateAdapterPreviewKind::RejectedReport(adapter)
                }
            })
        })
    }
    // READY_DURABLE_VALIDATE_ADAPTER_BRIDGE_END
    /// Complete a body reconstruction requested by [`AdapterEffect::FetchBody`].
    pub(crate) fn body_available(
        &mut self,
        tag: reducer::EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        let completion_evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let round = self
            .registry
            .round_to_core(manifest.round, &self.wire_context)?;
        let subject = self.registry.register_subject(manifest.subject)?;
        self.rollback_deferred_conflicting_proposal(round, subject, &manifest)?;
        let core_manifest = self
            .registry
            .manifest_to_core(&manifest, &self.wire_context)?;
        if core_manifest.subject() != subject {
            return Err(AdapterError::DurableBodyMismatch);
        }
        self.step_with_completion_evidence(
            reducer::Event::BodyAvailable {
                tag,
                round,
                subject,
            },
            Some(completion_evidence),
        )
    }
    /// Retag one Busy-deferred body completion for the reducer incarnation installed by a TC.
    ///
    /// Only lifecycle ownership changes; the manifest proposal round and
    /// subject remain exact.
    pub(crate) fn rebind_deferred_body_available(
        &mut self,
        previous: reducer::EventTag,
        rebound: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> usize {
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
        let mut rebound_count = 0usize;
        for input in &mut self.deferred_completions {
            if let reducer::Event::BodyAvailable {
                tag,
                round: queued_round,
                subject: queued_subject,
            } = &mut input.event
                && *tag == previous
                && *queued_round == round
                && *queued_subject == subject
            {
                *tag = rebound;
                rebound_count = rebound_count.saturating_add(1);
            }
        }
        rebound_count
    }
    /// Retire one Busy-deferred body completion whose exact pipeline was superseded.
    pub(crate) fn retire_deferred_body_available(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<usize, AdapterError> {
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
        let matches = |input: &DeferredInput| {
            matches!(
                &input.event,
                reducer::Event::BodyAvailable {
                    tag: queued_tag,
                    round: queued_round,
                    subject: queued_subject,
                } if *queued_tag == tag && *queued_round == round && *queued_subject == subject
            )
        };
        let retiring = self
            .deferred_completions
            .iter()
            .filter(|input| matches(input))
            .map(|input| input.admission_ordinal)
            .collect::<BTreeSet<_>>();
        self.release_deferred_producer_continuations_before_owner_removal(&retiring)?;
        let before = self.deferred_completions.len();
        self.deferred_completions.retain(|input| !matches(input));
        let retired = before.saturating_sub(self.deferred_completions.len());
        debug_assert_eq!(retired, retiring.len());
        Ok(retired)
    }
    /// Count every Busy-deferred completion stage for one exact body pipeline.
    pub(crate) fn deferred_body_pipeline_completion_counts(
        &self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> super::v2_runtime::RetiredBodyPipelineCompletions {
        let round = reducer::Round::new(round.height, round.view);
        let subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        let mut counts = super::v2_runtime::RetiredBodyPipelineCompletions::default();
        for input in self
            .deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
        {
            match deferred_body_pipeline_completion_stage(input, tag, round, subject) {
                Some(DeferredBodyPipelineCompletionStage::LocalProposalReady) => {
                    counts.record_local_proposal();
                }
                Some(DeferredBodyPipelineCompletionStage::BodyAvailable) => {
                    counts.record_body_available();
                }
                Some(DeferredBodyPipelineCompletionStage::BodyStored) => {
                    counts.record_body_stored();
                }
                None => {}
            }
        }
        counts
    }
    /// Retire every Busy-deferred completion stage for one exact body pipeline.
    pub(crate) fn retire_deferred_body_pipeline_completions(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<super::v2_runtime::RetiredBodyPipelineCompletions, AdapterError> {
        let round = reducer::Round::new(round.height, round.view);
        let subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        let retirements = self
            .deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
            .filter_map(|input| {
                deferred_body_pipeline_completion_stage(input, tag, round, subject)
                    .map(|stage| (input.admission_ordinal, stage))
            })
            .collect::<Vec<_>>();
        let retiring = retirements
            .iter()
            .map(|(ordinal, _)| *ordinal)
            .collect::<BTreeSet<_>>();
        if retiring.len() != retirements.len() {
            return Err(self.fail_serviced_candidate_store(
                "one deferred body occurrence occupied multiple serialized queues".to_owned(),
            ));
        }
        self.release_deferred_producer_continuations_before_owner_removal(&retiring)?;
        let mut retired = super::v2_runtime::RetiredBodyPipelineCompletions::default();
        let mut retire = |queue: &mut VecDeque<DeferredInput>| {
            queue.retain(|input| {
                match deferred_body_pipeline_completion_stage(input, tag, round, subject) {
                    Some(DeferredBodyPipelineCompletionStage::LocalProposalReady) => {
                        retired.record_local_proposal();
                        false
                    }
                    Some(DeferredBodyPipelineCompletionStage::BodyAvailable) => {
                        retired.record_body_available();
                        false
                    }
                    Some(DeferredBodyPipelineCompletionStage::BodyStored) => {
                        retired.record_body_stored();
                        false
                    }
                    None => true,
                }
            });
        };
        retire(&mut self.deferred_completions);
        retire(&mut self.deferred_inputs);
        Ok(retired)
    }
    /// Count logical and exact completion owners in the Busy-deferred lane.
    ///
    /// A logical owner occupies the same tag/stage/round/subject slot. An
    /// exact owner additionally retains evidence equal to `candidate`.
    pub(crate) fn deferred_body_pipeline_completion_ownership(
        &self,
        tag: reducer::EventTag,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> (usize, usize) {
        let (wire_round, wire_subject) = match candidate {
            BodyPipelineCompletionEvidence::LocalProposalReady { manifest, .. }
            | BodyPipelineCompletionEvidence::BodyAvailable { manifest } => {
                (manifest.round, manifest.subject)
            }
            BodyPipelineCompletionEvidence::BodyStored { round, subject, .. } => (*round, *subject),
        };
        let round = reducer::Round::new(wire_round.height, wire_round.view);
        let subject = reducer::Subject::new(Hash::new(wire_subject.encode()).into());
        self.deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
            .fold((0usize, 0usize), |(owners, exact), input| {
                let owns_slot = match (&input.event, candidate) {
                    (
                        reducer::Event::LocalProposalReady {
                            tag: queued_tag,
                            manifest,
                        },
                        BodyPipelineCompletionEvidence::LocalProposalReady { .. },
                    ) => {
                        *queued_tag == tag
                            && tag.height() == round.height()
                            && tag.view() == round.view()
                            && manifest.subject() == subject
                    }
                    (
                        reducer::Event::BodyAvailable {
                            tag: queued_tag,
                            round: queued_round,
                            subject: queued_subject,
                        },
                        BodyPipelineCompletionEvidence::BodyAvailable { .. },
                    )
                    | (
                        reducer::Event::BodyStored {
                            tag: queued_tag,
                            round: queued_round,
                            subject: queued_subject,
                        },
                        BodyPipelineCompletionEvidence::BodyStored { .. },
                    ) => *queued_tag == tag && *queued_round == round && *queued_subject == subject,
                    _ => false,
                };
                if !owns_slot {
                    return (owners, exact);
                }
                (
                    owners.saturating_add(1),
                    exact.saturating_add(usize::from(
                        input.completion_evidence.as_ref() == Some(candidate),
                    )),
                )
            })
    }
    /// Return the adapter admission ordinals of exact Busy-deferred owners.
    ///
    /// The serialized runtime joins these ordinals to its retained lifecycle
    /// sidecars before it permits an owner-aware completion to coalesce.
    pub(crate) fn deferred_body_pipeline_completion_exact_owner_ordinals(
        &self,
        tag: reducer::EventTag,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> Vec<u128> {
        let (wire_round, wire_subject, expected_stage) = match candidate {
            BodyPipelineCompletionEvidence::LocalProposalReady { manifest, .. } => (
                manifest.round,
                manifest.subject,
                DeferredBodyPipelineCompletionStage::LocalProposalReady,
            ),
            BodyPipelineCompletionEvidence::BodyAvailable { manifest } => (
                manifest.round,
                manifest.subject,
                DeferredBodyPipelineCompletionStage::BodyAvailable,
            ),
            BodyPipelineCompletionEvidence::BodyStored { round, subject, .. } => (
                *round,
                *subject,
                DeferredBodyPipelineCompletionStage::BodyStored,
            ),
        };
        let round = reducer::Round::new(wire_round.height, wire_round.view);
        let subject = reducer::Subject::new(Hash::new(wire_subject.encode()).into());
        self.deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
            .filter(|input| {
                input.completion_evidence.as_ref() == Some(candidate)
                    && deferred_body_pipeline_completion_stage(input, tag, round, subject)
                        == Some(expected_stage)
            })
            .map(|input| input.admission_ordinal)
            .collect()
    }
    /// Return exact body-stage terminal evidence retained behind the Busy
    /// reducer boundary. `BodyAvailable` is intentionally excluded because
    /// its persistent producer and restart aliases have stricter ownership
    /// rules than the cached Store terminal stage.
    pub(crate) fn deferred_body_pipeline_terminal_candidates(
        &self,
    ) -> Vec<(u128, reducer::EventTag, BodyPipelineCompletionEvidence)> {
        self.deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
            .filter_map(|input| {
                let evidence = input.completion_evidence.as_ref()?;
                let tag = match (&input.event, evidence) {
                    (
                        reducer::Event::LocalProposalReady { tag, .. },
                        BodyPipelineCompletionEvidence::LocalProposalReady { .. },
                    )
                    | (
                        reducer::Event::BodyStored { tag, .. },
                        BodyPipelineCompletionEvidence::BodyStored { .. },
                    ) => *tag,
                    _ => return None,
                };
                let (wire_round, wire_subject, expected_stage) = match evidence {
                    BodyPipelineCompletionEvidence::LocalProposalReady { manifest, .. } => (
                        manifest.round,
                        manifest.subject,
                        DeferredBodyPipelineCompletionStage::LocalProposalReady,
                    ),
                    BodyPipelineCompletionEvidence::BodyStored { round, subject, .. } => (
                        *round,
                        *subject,
                        DeferredBodyPipelineCompletionStage::BodyStored,
                    ),
                    BodyPipelineCompletionEvidence::BodyAvailable { .. } => return None,
                };
                let round = reducer::Round::new(wire_round.height, wire_round.view);
                let subject = reducer::Subject::new(Hash::new(wire_subject.encode()).into());
                if deferred_body_pipeline_completion_stage(input, tag, round, subject)
                    != Some(expected_stage)
                {
                    return None;
                }
                Some((input.admission_ordinal, tag, evidence.clone()))
            })
            .collect()
    }
    /// Report whether the exact Busy-deferred `BodyAvailable` owner carries
    /// the adapter's sole persistent producer reservation.
    ///
    /// View-change coalescence must never retire this owner in favour of an
    /// ordinary volatile runtime destination: doing so would remove the only
    /// record from which a same-height restart can reconstruct `FetchBody`.
    /// Validate every process/durable alias before granting that authority so
    /// corrupt metadata fails closed before either queue is mutated.
    pub(crate) fn deferred_body_available_has_persistent_producer(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, AdapterError> {
        let candidate = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let ordinals = self.deferred_body_pipeline_completion_exact_owner_ordinals(tag, &candidate);
        let ordinal = match ordinals.as_slice() {
            [] => return Ok(false),
            [ordinal] => *ordinal,
            _ => {
                return Err(self.fail_serviced_candidate_store(
                    "one exact deferred body completion occupied multiple Busy owners".to_owned(),
                ));
            }
        };
        let Some(reservation) = self.deferred_producer_continuations.get(&ordinal) else {
            return Ok(false);
        };
        let address = reservation.address;
        let Some(record) = self.producer_continuations.get(&address) else {
            return Err(self.fail_serviced_candidate_store(
                "deferred body producer lost its process reservation".to_owned(),
            ));
        };
        if record.status() != ProducerContinuationStatus::Reserved
            || record.source_class() != ProducerContinuationSourceClass::VolatileBody
            || record.identity().address() != address
            || record.identity().stage() != ServicedCandidateStage::BodyAvailable as u8
            || self.durable_producer_continuations.get(&address) != Some(record)
            || self
                .restored_dormant_producer_continuations
                .contains(&address)
            || self.pending_producer_handoffs.contains_key(&address)
        {
            return Err(self.fail_serviced_candidate_store(
                "deferred body producer did not retain one exact live durable reservation"
                    .to_owned(),
            ));
        }
        Ok(true)
    }
    /// Classify exact decided `LocalProposalReady` owners without mutating any
    /// Busy-deferred lane.
    pub(crate) fn deferred_decided_local_proposal_counts(
        &self,
        decision_tag: reducer::EventTag,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> super::v2_runtime::DecisionLocalProposalCounts {
        let mut counts = super::v2_runtime::DecisionLocalProposalCounts::default();
        for input in self
            .deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
        {
            if let Some(disposition) = classify_deferred_decided_local_proposal(
                input,
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            ) {
                counts.record(disposition);
            }
        }
        counts
    }
    /// Retire Busy-deferred proposal work after one exact decision is installed.
    ///
    /// All authenticated proposals and nonmatching local completions for the
    /// decided height are terminal. Body recovery and application
    /// completions remain owned because the decision may still need them before
    /// application. A unique current-tag completion whose full receipts match
    /// the Decision remains in place for canonical application. Stale exact
    /// completions are retired so the durable reconstruction path can re-enter
    /// the reducer.
    pub(crate) fn retire_deferred_proposal_work_after_decision(
        &mut self,
        decision_tag: reducer::EventTag,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> Result<(), AdapterError> {
        let core_round = reducer::Round::new(decision_round.height, decision_round.view);
        let core_subject = reducer::Subject::new(Hash::new(decision_subject.encode()).into());
        let remove = |input: &DeferredInput| match &input.event {
            reducer::Event::ProposalReceived { proposal, .. }
                if proposal.proposal().round().height() == decision_round.height =>
            {
                true
            }
            reducer::Event::LocalProposalReady { tag, .. } => input
                .completion_evidence
                .as_ref()
                .and_then(|evidence| match evidence {
                    BodyPipelineCompletionEvidence::LocalProposalReady { manifest, .. } => {
                        Some(manifest.round.height)
                    }
                    BodyPipelineCompletionEvidence::BodyAvailable { .. }
                    | BodyPipelineCompletionEvidence::BodyStored { .. } => None,
                })
                .is_some_and(|height| height == decision_round.height)
                .then(|| {
                    !matches!(
                        classify_deferred_decided_local_proposal(
                            input,
                            decision_tag,
                            decision_round,
                            decision_subject,
                            decision_commitment,
                        ),
                        Some(DecisionLocalProposalDisposition::Retain)
                    )
                })
                .unwrap_or(tag.height() == decision_round.height),
            reducer::Event::ResumeAfterReplay { .. }
            | reducer::Event::ProposalReceived { .. }
            | reducer::Event::VoteReceived { .. }
            | reducer::Event::QuorumCertificateReceived { .. }
            | reducer::Event::TimeoutVoteReceived { .. }
            | reducer::Event::TimeoutCertificateReceived { .. }
            | reducer::Event::TimeoutElapsed { .. }
            | reducer::Event::RetransmitElapsed { .. }
            | reducer::Event::BodyAvailable { .. }
            | reducer::Event::BodyStored { .. }
            | reducer::Event::ValidationCompleted { .. }
            | reducer::Event::Persisted { .. }
            | reducer::Event::PersistenceFailed { .. }
            | reducer::Event::Signed { .. }
            | reducer::Event::ApplicationCompleted { .. } => false,
        };
        let retiring = self
            .deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .filter(|input| remove(input))
            .map(|input| input.admission_ordinal)
            .collect::<BTreeSet<_>>();
        self.release_deferred_producer_continuations_before_owner_removal(&retiring)?;
        self.deferred_completions.retain(|input| !remove(input));
        self.deferred_progress_inputs.retain(|input| !remove(input));
        self.deferred_inputs.retain(|input| !remove(input));
        if self.active_subject.is_some_and(|(round, subject)| {
            round.height() == decision_round.height
                && (round != core_round || subject != core_subject)
        }) {
            self.active_subject = None;
        }
        Ok(())
    }
    /// Retire deferred proposals made unsafe by an installed durable lock.
    ///
    /// The locked subject may remain queued in a later justified view. A
    /// different subject survives only when the proposal carries a strictly
    /// higher matching PrepareQC.
    pub(crate) fn retire_deferred_unsafe_proposals_for_lock(
        &mut self,
        locked_round: wire::ConsensusRound,
        locked_subject: wire::BlockSubject,
    ) -> Result<usize, AdapterError> {
        let locked_round = reducer::Round::new(locked_round.height, locked_round.view);
        let locked_subject = reducer::Subject::new(Hash::new(locked_subject.encode()).into());
        let context_id = self.reducer.context().id();
        let remove = |input: &DeferredInput| {
            let reducer::Event::ProposalReceived { proposal, .. } = &input.event else {
                return false;
            };
            let proposal = proposal.proposal();
            if proposal.context_id() != context_id
                || proposal.round().height() != locked_round.height()
            {
                return false;
            }
            if proposal.round().view() < locked_round.view() {
                return true;
            }
            if proposal.manifest().subject() == locked_subject {
                return false;
            }
            let reducer::ProposalJustification::Timeout(certificate) = proposal.justification()
            else {
                return true;
            };
            !certificate.highest_prepare().is_some_and(|highest| {
                highest.phase() == reducer::Phase::Prepare
                    && highest.subject() == proposal.manifest().subject()
                    && highest.round().view() > locked_round.view()
            })
        };
        let retiring = self
            .deferred_inputs
            .iter()
            .filter(|input| remove(input))
            .map(|input| input.admission_ordinal)
            .collect::<BTreeSet<_>>();
        self.release_deferred_producer_continuations_before_owner_removal(&retiring)?;
        let before = self.deferred_inputs.len();
        self.deferred_inputs.retain(|input| !remove(input));
        let retired = before.saturating_sub(self.deferred_inputs.len());
        self.active_subject = Some((locked_round, locked_subject));
        debug_assert_eq!(retired, retiring.len());
        Ok(retired)
    }
    /// Stage one exact completion at the adapter boundary for runtime/executor seam tests.
    #[cfg(test)]
    pub(crate) fn defer_body_available_for_test(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<(), AdapterError> {
        let core_manifest = self
            .registry
            .manifest_to_core(manifest, &self.wire_context)?;
        let admission_capability = self.mint_deferred_admission_ordinal(false)?;
        let admission_ordinal = admission_capability.ordinal;
        self.deferred_completions.push_back(DeferredInput {
            admission_ordinal,
            admission_capability,
            event: reducer::Event::BodyAvailable {
                tag,
                round: reducer::Round::new(manifest.round.height, manifest.round.view),
                subject: core_manifest.subject(),
            },
            completion_evidence: Some(BodyPipelineCompletionEvidence::BodyAvailable {
                manifest: manifest.clone(),
            }),
            retag_authenticated_ingress: false,
            priority: DeferredPriority::Completion,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
        Ok(())
    }
    /// Stage one authenticated proposal and its exact semantic admission
    /// records in the Busy-deferred lane for runtime seam tests.
    #[cfg(test)]
    pub(crate) fn defer_authenticated_proposal_for_test(
        &mut self,
        tag: reducer::EventTag,
        proposal: &wire::Proposal,
    ) -> Result<(), AdapterError> {
        let admission_key = IngressSemanticKey::Proposal {
            round: proposal.round,
            proposer: proposal.proposer,
        };
        let fingerprint = IngressFingerprint::Proposal(Hash::new(proposal.signature_preimage()));
        let authenticated_wire_identity = Arc::<[u8]>::from(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            ))
            .encode(),
        );
        let wire_proposal = proposal.clone();
        let proposal = self
            .registry
            .proposal_to_core(proposal, &self.wire_context)?;
        let round = proposal.proposal().round();
        let subject = proposal.proposal().manifest().subject();
        self.active_subject = Some((round, subject));
        let admission_capability = self.mint_deferred_admission_ordinal(true)?;
        let admission_ordinal = admission_capability.ordinal;
        let admitted_at = Instant::now();
        self.deferred_inputs.push_back(DeferredInput {
            admission_ordinal,
            admission_capability,
            event: reducer::Event::ProposalReceived { tag, proposal },
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Normal,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: Some(authenticated_wire_identity),
            admitted_at,
            eligible_skips: 0,
        });
        assert!(
            self.ingress_equivocations
                .insert(
                    admission_key,
                    IngressEquivocationRecord {
                        fingerprint,
                        artifact: IngressEquivocationArtifact::Proposal(Arc::new(wire_proposal)),
                        equivocation_reported: false,
                        capacity_bypass: false,
                        admitted_at,
                    },
                )
                .is_none(),
            "authenticated Busy test seam cannot replace semantic admission ownership"
        );
        assert!(
            self.ingress_deliveries
                .insert(
                    admission_key,
                    IngressDeliveryRecord {
                        fingerprint,
                        consumer_tag: tag,
                        locked_commit_progress: false,
                        locked_reproposal_prepare_progress: false,
                    },
                )
                .is_none(),
            "authenticated Busy test seam cannot replace delivery ownership"
        );
        Ok(())
    }
    /// Stage one non-fetch body completion in the Busy-deferred lane for seam tests.
    #[cfg(test)]
    pub(crate) fn defer_body_pipeline_stage_for_test(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
        stage: DeferredBodyPipelineStageForTest,
    ) -> Result<(), AdapterError> {
        let core_manifest = self
            .registry
            .manifest_to_core(manifest, &self.wire_context)?;
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let subject = core_manifest.subject();
        let durable_receipt = DurableBodyReceipt::for_test(
            self.wire_context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(manifest),
        );
        let validated_receipt = ValidatedBodyReceipt::for_test(durable_receipt.clone());
        let completion_evidence = match stage {
            DeferredBodyPipelineStageForTest::BodyAvailable => {
                BodyPipelineCompletionEvidence::BodyAvailable {
                    manifest: manifest.clone(),
                }
            }
            DeferredBodyPipelineStageForTest::BodyStored => {
                BodyPipelineCompletionEvidence::BodyStored {
                    round: manifest.round,
                    subject: manifest.subject,
                    receipt: durable_receipt,
                }
            }
            DeferredBodyPipelineStageForTest::LocalProposalReady => {
                BodyPipelineCompletionEvidence::LocalProposalReady {
                    manifest: manifest.clone(),
                    durable_receipt,
                    validated_receipt,
                }
            }
        };
        let event = match stage {
            DeferredBodyPipelineStageForTest::BodyAvailable => reducer::Event::BodyAvailable {
                tag,
                round,
                subject,
            },
            DeferredBodyPipelineStageForTest::BodyStored => reducer::Event::BodyStored {
                tag,
                round,
                subject,
            },
            DeferredBodyPipelineStageForTest::LocalProposalReady => {
                reducer::Event::LocalProposalReady {
                    tag,
                    manifest: core_manifest,
                }
            }
        };
        let admission_capability = self.mint_deferred_admission_ordinal(false)?;
        let admission_ordinal = admission_capability.ordinal;
        self.deferred_completions.push_back(DeferredInput {
            admission_ordinal,
            admission_capability,
            event,
            completion_evidence: Some(completion_evidence),
            retag_authenticated_ingress: false,
            priority: DeferredPriority::Completion,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
        Ok(())
    }
    fn deferred_conflicting_proposal_owner(
        &self,
        round: reducer::Round,
        subject: reducer::Subject,
        canonical: &wire::PayloadManifest,
    ) -> Option<(
        wire::PayloadManifest,
        wire::Proposal,
        IngressSemanticKey,
        IngressEquivocationRecord,
    )> {
        // Busy authenticated ingress deliberately retains its staged registry
        // expansion. A canonical body completion may overtake that deferred
        // proposal, but may roll back only the exact proposal-owned manifest;
        // independently verified justification, QC, and subject material stays
        // registered for subsequent progress.
        let key = (round, subject);
        let Some(registered_manifest) = self.registry.manifests.get(&key).cloned() else {
            return None;
        };
        if registered_manifest == *canonical {
            return None;
        }
        let Some(registered_proposal) = self.registry.proposals.get(&key).cloned() else {
            return None;
        };
        if registered_proposal.round != canonical.round
            || registered_proposal.subject != canonical.subject
            || registered_proposal.manifest != registered_manifest
        {
            return None;
        }
        let admission_key = IngressSemanticKey::Proposal {
            round: registered_proposal.round,
            proposer: registered_proposal.proposer,
        };
        let expected_fingerprint =
            IngressFingerprint::Proposal(Hash::new(registered_proposal.signature_preimage()));
        let Some(registered_equivocation) = self.ingress_equivocations.get(&admission_key).cloned()
        else {
            return None;
        };
        if registered_equivocation.fingerprint != expected_fingerprint {
            return None;
        }
        let owns_conflict = |input: &DeferredInput| {
            Self::deferred_input_owns_registered_proposal(
                input,
                round,
                subject,
                &registered_proposal,
            )
        };
        if !self.deferred_inputs.iter().any(owns_conflict) {
            return None;
        }
        Some((
            registered_manifest,
            registered_proposal,
            admission_key,
            registered_equivocation,
        ))
    }
    fn rollback_deferred_conflicting_proposal(
        &mut self,
        round: reducer::Round,
        subject: reducer::Subject,
        canonical: &wire::PayloadManifest,
    ) -> Result<bool, AdapterError> {
        let Some((
            registered_manifest,
            registered_proposal,
            admission_key,
            registered_equivocation,
        )) = self.deferred_conflicting_proposal_owner(round, subject, canonical)
        else {
            return Ok(false);
        };
        let key = (round, subject);
        let owns_conflict = |input: &DeferredInput| {
            Self::deferred_input_owns_registered_proposal(
                input,
                round,
                subject,
                &registered_proposal,
            )
        };
        let retiring = self
            .deferred_inputs
            .iter()
            .filter(|input| owns_conflict(input))
            .map(|input| input.admission_ordinal)
            .collect::<BTreeSet<_>>();
        self.release_deferred_producer_continuations_before_owner_removal(&retiring)?;
        self.deferred_inputs.retain(|input| !owns_conflict(input));
        let removed_proposal = self.registry.proposals.remove(&key);
        let removed_manifest = self.registry.manifests.remove(&key);
        let removed_equivocation = self.ingress_equivocations.remove(&admission_key);
        self.ingress_deliveries.remove(&admission_key);
        debug_assert_eq!(removed_proposal, Some(registered_proposal));
        debug_assert_eq!(removed_manifest, Some(registered_manifest));
        debug_assert_eq!(removed_equivocation, Some(registered_equivocation));
        Ok(true)
    }
    fn deferred_input_owns_registered_proposal(
        input: &DeferredInput,
        round: reducer::Round,
        subject: reducer::Subject,
        registered: &wire::Proposal,
    ) -> bool {
        if !input.retag_authenticated_ingress || input.priority != DeferredPriority::Normal {
            return false;
        }
        let reducer::Event::ProposalReceived { proposal, .. } = &input.event else {
            return false;
        };
        let core = proposal.proposal();
        let core_manifest = core.manifest();
        core.context_id() == context_id(registered.round.context_id)
            && core.round() == round
            && core.proposer() == validator_token(registered.proposer)
            && core_manifest.subject() == subject
            && core_manifest.payload_hash()
                == reducer::Digest::new(*registered.manifest.subject.payload_hash.as_ref())
            && core_manifest.chunk_root()
                == reducer::Digest::new(*registered.manifest.chunk_root.as_ref())
            && core_manifest.byte_len() == registered.manifest.payload_size_bytes
            && u32::try_from(registered.manifest.chunk_hashes.len()).ok()
                == Some(core_manifest.chunk_count())
            && proposal.signature().as_bytes() == registered.signature
    }
    /// Acknowledge durable storage requested by [`AdapterEffect::StoreBody`].
    pub(crate) fn body_stored(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<AdapterOutcome, AdapterError> {
        if receipt.context_id() != self.wire_context.id()
            || receipt.round() != round
            || receipt.subject() != subject
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let round = self.registry.round_to_core(round, &self.wire_context)?;
        let subject = self.registry.register_subject(subject)?;
        let manifest = self
            .registry
            .manifests
            .get(&(round, subject))
            .ok_or(AdapterError::MissingManifest)?;
        if receipt.manifest_hash() != HashOf::new(manifest) {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let completion_evidence = BodyPipelineCompletionEvidence::BodyStored {
            round: receipt.round(),
            subject: receipt.subject(),
            receipt: receipt.clone(),
        };
        self.step_with_completion_evidence(
            reducer::Event::BodyStored {
                tag,
                round,
                subject,
            },
            Some(completion_evidence),
        )
    }
    /// Return the opaque signature requested by [`AdapterEffect::Sign`].
    pub(crate) fn signature_completed(
        &mut self,
        tag: reducer::EventTag,
        signature: Vec<u8>,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.step(reducer::Event::Signed {
            tag,
            signature: reducer::OpaqueSignature::new(signature),
        })
    }
}
include!("v2_recovered_lifecycle_sign_completion.rs");
impl SumeragiV2Adapter {
    /// Acknowledge successful application of the exact tagged decision.
    ///
    /// The reducer validates the current `(height, view, generation)` tag and
    /// refuses a completion for an undecided, unvalidated, stale, or different
    /// subject.
    pub(crate) fn application_completed(
        &mut self,
        tag: reducer::EventTag,
        subject: wire::BlockSubject,
    ) -> Result<AdapterOutcome, AdapterError> {
        let subject = self.registry.register_subject(subject)?;
        self.step(reducer::Event::ApplicationCompleted { tag, subject })
    }
    /// Preview the sole registry-owned lifecycle Decision Apply completion.
    ///
    /// The adapter transition runs on cloned reducer and registry state. The
    /// returned token keeps the exclusive adapter borrow and can install that
    /// exact state only after lifecycle LedgerV1 publication succeeds.
    pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(
        &mut self,
        authority: LifecycleDecisionApplyAdapterCompletionAuthorityV1,
    ) -> Result<PreparedLifecycleDecisionApplyAdapterCompletionV1<'_>, AdapterError> {
        self.ensure_ingress()?;
        let LifecycleDecisionApplyAdapterCompletionAuthorityV1 {
            tag,
            subject,
            dispatch_key,
            validate_predecessor_ordinal,
            receipt,
            artifact,
        } = authority;
        if self.current_tag() != tag
            || !dispatch_key.matches_height_context(&self.wire_context)
            || validate_predecessor_ordinal == 0
            || validate_predecessor_ordinal >= dispatch_key.lifecycle_ordinal()
            || artifact.height_context != self.wire_context
            || artifact.subject != subject
            || receipt.height() != self.wire_context.height
            || receipt.context_id() != self.wire_context.id()
            || receipt.subject() != subject
            || receipt.block_hash() != subject.block_hash
            || receipt.certificate() != artifact.commit_qc.as_ref()
            || receipt.artifact_hash() != HashOf::new(&artifact)
        {
            return Err(AdapterError::LifecycleDecisionApplyCompletionMismatch);
        }
        let mut next_registry = self.registry.clone();
        let core_subject = next_registry.register_subject(subject)?;
        let event = reducer::Event::ApplicationCompleted {
            tag,
            subject: core_subject,
        };
        let mut next_reducer = self.reducer.clone();
        let outcome = next_reducer.step(event.clone())?;
        if outcome.disposition() != reducer::StepDisposition::Applied
            || !outcome.effects().is_empty()
            || self.pending_persistence_id.is_some()
            || !self.deferred_completions.is_empty()
            || !self.deferred_progress_inputs.is_empty()
            || !self.deferred_inputs.is_empty()
            || next_reducer.applied_subject() != Some(core_subject)
            || next_reducer.pending_persistence_record().is_some()
            || next_reducer.awaiting_signature().is_some()
            || !next_reducer.ready_to_finish()
        {
            return Err(AdapterError::LifecycleDecisionApplyCompletionMismatch);
        }
        let next_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: self.replay_complete,
        };
        let next_fence_generation = if next_fence == self.reducer_fence_projection() {
            self.reducer_fence_generation
        } else {
            self.reducer_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
        };
        // Precompute the exact status before LedgerV1 can advance. The old
        // adapter state and progress marker are restored before returning.
        core::mem::swap(&mut self.reducer, &mut next_reducer);
        core::mem::swap(&mut self.registry, &mut next_registry);
        let prior_last_progress = self.last_progress;
        self.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &[]);
        let committed_status = self.status();
        self.last_progress = prior_last_progress;
        core::mem::swap(&mut self.registry, &mut next_registry);
        core::mem::swap(&mut self.reducer, &mut next_reducer);
        let committed_status = committed_status?;
        Ok(PreparedLifecycleDecisionApplyAdapterCompletionV1 {
            adapter: self,
            next_reducer,
            next_registry,
            event,
            next_fence_generation,
            dispatch_key,
            validate_predecessor_ordinal,
            receipt,
            artifact,
            committed_status,
        })
    }
    /// Decide whether an exact internal callback still needs a serialized
    /// runtime admission.
    ///
    /// The projection uses cloned registry/reducer state, so malformed, stale,
    /// monotone-complete, and durably tombstoned callbacks consume neither an
    /// admission ordinal nor a physical FIFO slot. Authenticated wire ingress
    /// deliberately bypasses this seam and remains governed by canonical
    /// authentication and semantic-delivery ownership.
    pub(crate) fn preflight_runtime_command_admission(
        &self,
        tag: reducer::EventTag,
        command: &super::v2_runtime::AdapterCommand,
    ) -> super::v2_runtime::RuntimeCommandAdmissionPreflight {
        use super::v2_runtime::{AdapterCommand, RuntimeCommandAdmissionPreflight as Preflight};
        if matches!(command, AdapterCommand::Authenticated(_)) {
            return Preflight::Admit;
        }
        if self.fail_closed || !self.replay_complete {
            return Preflight::Reject;
        }
        let projected = (|| -> Result<_, AdapterError> {
            let mut registry = self.registry.clone();
            let (event, completion_evidence) = match command {
                AdapterCommand::Authenticated(_) => unreachable!("handled above"),
                AdapterCommand::LocalProposalReady {
                    manifest,
                    durable_receipt,
                    validated_receipt,
                } => {
                    if durable_receipt.context_id() != self.wire_context.id()
                        || durable_receipt.round() != manifest.round
                        || durable_receipt.subject() != manifest.subject
                        || durable_receipt.manifest_hash() != HashOf::new(manifest)
                        || validated_receipt.durable() != durable_receipt
                    {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    let core_manifest = registry.manifest_to_core(manifest, &self.wire_context)?;
                    let round = registry.round_to_core(manifest.round, &self.wire_context)?;
                    registry.register_execution_commitment(
                        round,
                        core_manifest.subject(),
                        validated_receipt.execution_commitment(),
                    )?;
                    (
                        reducer::Event::LocalProposalReady {
                            tag,
                            manifest: core_manifest,
                        },
                        Some(BodyPipelineCompletionEvidence::LocalProposalReady {
                            manifest: manifest.clone(),
                            durable_receipt: durable_receipt.clone(),
                            validated_receipt: validated_receipt.clone(),
                        }),
                    )
                }
                AdapterCommand::BodyAvailable { manifest } => {
                    let round = registry.round_to_core(manifest.round, &self.wire_context)?;
                    let subject = registry.register_subject(manifest.subject)?;
                    if self
                        .deferred_conflicting_proposal_owner(round, subject, manifest)
                        .is_some()
                    {
                        // Mirror the exact dispatch-side rollback in the
                        // cloned preflight registry. No live proposal,
                        // delivery, equivocation, or deferred owner is retired
                        // until the admitted callback actually dispatches.
                        registry.proposals.remove(&(round, subject));
                        registry.manifests.remove(&(round, subject));
                    }
                    let core_manifest = registry.manifest_to_core(manifest, &self.wire_context)?;
                    if core_manifest.subject() != subject {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    (
                        reducer::Event::BodyAvailable {
                            tag,
                            round,
                            subject,
                        },
                        Some(BodyPipelineCompletionEvidence::BodyAvailable {
                            manifest: manifest.clone(),
                        }),
                    )
                }
                AdapterCommand::BodyStored {
                    round,
                    subject,
                    receipt,
                } => {
                    if receipt.context_id() != self.wire_context.id()
                        || receipt.round() != *round
                        || receipt.subject() != *subject
                    {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    let core_round = registry.round_to_core(*round, &self.wire_context)?;
                    let core_subject = registry.register_subject(*subject)?;
                    let manifest = registry
                        .manifests
                        .get(&(core_round, core_subject))
                        .ok_or(AdapterError::MissingManifest)?;
                    if receipt.manifest_hash() != HashOf::new(manifest) {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    (
                        reducer::Event::BodyStored {
                            tag,
                            round: core_round,
                            subject: core_subject,
                        },
                        Some(BodyPipelineCompletionEvidence::BodyStored {
                            round: *round,
                            subject: *subject,
                            receipt: receipt.clone(),
                        }),
                    )
                }
                AdapterCommand::SignatureCompleted(signature) => (
                    reducer::Event::Signed {
                        tag,
                        signature: reducer::OpaqueSignature::new(signature.clone()),
                    },
                    None,
                ),
                AdapterCommand::ApplicationCompleted(subject) => (
                    reducer::Event::ApplicationCompleted {
                        tag,
                        subject: registry.register_subject(*subject)?,
                    },
                    None,
                ),
            };
            Ok((event, completion_evidence))
        })();
        let Ok((event, completion_evidence)) = projected else {
            return Preflight::Reject;
        };
        // Internal completions retain their originating reducer incarnation.
        // A delayed completion from an obsolete incarnation is a harmless
        // stutter, but it must be discarded before allocating a new runtime
        // ordinal. The exact payload was validated above first so a malformed
        // internal callback cannot hide behind a stale tag.
        if tag != self.reducer.current_tag() {
            return Preflight::Coalesce;
        }
        let serviced_candidate = self.serviced_candidate(
            &event,
            DeferredPriority::Completion,
            completion_evidence.as_ref(),
            None,
        );
        if let Some((key, _, _)) = serviced_candidate {
            let serviced = self.serviced_candidates.contains_key(&key);
            let matching = self
                .producer_continuations
                .iter()
                .filter(|(_, record)| record.identity().candidate() == key)
                .collect::<Vec<_>>();
            match matching.len() {
                0 if serviced => return Preflight::Coalesce,
                0 => {}
                1 => {
                    let (address, record) = matching[0];
                    let identity = record.identity();
                    if serviced
                        || record.status() != ProducerContinuationStatus::Reserved
                        || !self
                            .restored_dormant_producer_continuations
                            .contains(address)
                        || self.durable_producer_continuations.get(address) != Some(record)
                    {
                        return Preflight::CoalesceOwned {
                            causal_lifecycle_key: identity.causal_lifecycle_key(),
                            admission_ordinal: identity.admission_ordinal(),
                        };
                    }
                    // `ServicedCandidateKey` is deliberately route/priority
                    // neutral. This branch is nevertheless class-exact:
                    // only internal completion commands reach this
                    // preflight, and the serialized runtime rejects
                    // `ReuseDormant` in Normal or Progress before allocating
                    // a FIFO ordinal. Authenticated traffic retains the
                    // separate leader-wire lifecycle gate above.
                    return Preflight::ReuseDormant {
                        causal_lifecycle_key: identity.causal_lifecycle_key(),
                        admission_ordinal: identity.admission_ordinal(),
                        producer_stage: identity.stage(),
                    };
                }
                _ => return Preflight::Reject,
            }
        }
        // The reducer's persistence/signing fences intentionally report Busy
        // before dispatching an event to its phase handler. Consult the
        // phase-specific monotone facts first so an exact callback which has
        // already handed ownership to its successor cannot be admitted again
        // merely because unrelated durable work is now fenced.
        let phase_fact = match &event {
            reducer::Event::LocalProposalReady { manifest, .. } => {
                let round = reducer::Round::new(tag.height(), tag.view());
                let classify_proposal = |proposal: &reducer::Proposal| {
                    (proposal.round() == round).then(|| {
                        if proposal.manifest() == manifest {
                            Preflight::Coalesce
                        } else {
                            Preflight::Reject
                        }
                    })
                };
                self.reducer
                    .pending_persistence_record()
                    .and_then(|record| match record {
                        reducer::WalRecord::ProposalIntent(proposal) => classify_proposal(proposal),
                        _ => None,
                    })
                    .or_else(|| {
                        self.reducer
                            .durable_state()
                            .proposal_intent(round)
                            .and_then(classify_proposal)
                    })
                    .or_else(|| {
                        self.reducer
                            .awaiting_signature()
                            .and_then(|signable| match signable {
                                reducer::SignableMessage::Proposal(proposal) => {
                                    classify_proposal(proposal)
                                }
                                reducer::SignableMessage::Vote(_)
                                | reducer::SignableMessage::TimeoutVote(_) => None,
                            })
                    })
                    .or_else(|| {
                        self.reducer
                            .durable_state()
                            .decision()
                            .and_then(|decision| {
                                let exact_decided_body = decision.proposal_round() == round
                                    && decision.subject() == manifest.subject();
                                (exact_decided_body
                                    && (self.reducer.applied_subject() == Some(decision.subject())
                                        || self.reducer.body_state(round, decision.subject())
                                            == reducer::BodyState::Validated))
                                    .then_some(Preflight::Coalesce)
                            })
                    })
            }
            reducer::Event::BodyAvailable { round, subject, .. } => {
                (self.reducer.body_state(*round, *subject) != reducer::BodyState::Missing)
                    .then_some(Preflight::Coalesce)
            }
            reducer::Event::BodyStored { round, subject, .. } => {
                match self.reducer.body_state(*round, *subject) {
                    reducer::BodyState::Missing | reducer::BodyState::Available => None,
                    reducer::BodyState::Durable
                    | reducer::BodyState::Validated
                    | reducer::BodyState::Invalid => Some(Preflight::Coalesce),
                }
            }
            reducer::Event::ValidationCompleted {
                round,
                subject,
                valid,
                ..
            } => match (self.reducer.body_state(*round, *subject), *valid) {
                (reducer::BodyState::Missing | reducer::BodyState::Durable, _) => None,
                (reducer::BodyState::Validated, true)
                | (reducer::BodyState::Invalid, false)
                | (reducer::BodyState::Available, _) => Some(Preflight::Coalesce),
                (reducer::BodyState::Validated, false) | (reducer::BodyState::Invalid, true) => {
                    Some(Preflight::Reject)
                }
            },
            reducer::Event::Signed { .. } => self
                .reducer
                .awaiting_signature()
                .is_none()
                .then_some(Preflight::Coalesce),
            reducer::Event::ApplicationCompleted { subject, .. } => {
                self.reducer.applied_subject().map(|applied| {
                    if applied == *subject {
                        Preflight::Coalesce
                    } else {
                        Preflight::Reject
                    }
                })
            }
            reducer::Event::ResumeAfterReplay { .. }
            | reducer::Event::ProposalReceived { .. }
            | reducer::Event::VoteReceived { .. }
            | reducer::Event::QuorumCertificateReceived { .. }
            | reducer::Event::TimeoutVoteReceived { .. }
            | reducer::Event::TimeoutCertificateReceived { .. }
            | reducer::Event::TimeoutElapsed { .. }
            | reducer::Event::RetransmitElapsed { .. }
            | reducer::Event::Persisted { .. }
            | reducer::Event::PersistenceFailed { .. } => None,
        };
        if let Some(preflight) = phase_fact {
            return preflight;
        }
        let mut projected_reducer = self.reducer.clone();
        match projected_reducer.step(event) {
            Ok(outcome) => match outcome.disposition() {
                reducer::StepDisposition::Applied
                | reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
                | reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork) => {
                    Preflight::Admit
                }
                reducer::StepDisposition::Ignored(_) => Preflight::Coalesce,
            },
            Err(_) => Preflight::Reject,
        }
    }
    /// Consume an applied height after Kura has durably associated the exact
    /// canonical block and CommitQC artifact.
    ///
    /// This is the only production path which retires the height safety WAL.
    /// It compares the non-forgeable Kura receipt, the persisted artifact, and
    /// the reducer's cryptographically verified decision before consuming the
    /// reducer, then transfers the live safety stores into a sealed owner.
    /// That owner keeps both stores intact until durable rollover completes.
    pub(crate) fn finish_height(
        mut self,
        kura_receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<FinalizedV2Height, AdapterError> {
        self.ensure_ingress()?;
        artifact
            .verify()
            .map_err(|error| AdapterError::Cryptography(error.to_string()))?;
        let core_decision = self
            .reducer
            .durable_state()
            .decision()
            .cloned()
            .ok_or(AdapterError::DurableCommitMismatch)?;
        let wire_decision = self
            .registry
            .qc_to_wire(&core_decision, self.aggregator.as_ref())?;
        let wire_subject = self.registry.subject(core_decision.subject())?;
        if artifact.height_context != self.wire_context
            || artifact.validator_set_pops != self.proofs_of_possession
            || artifact.subject != wire_subject
            || artifact.commit_qc != wire_decision
            || kura_receipt.height() != self.wire_context.height
            || kura_receipt.context_id() != self.wire_context.id()
            || kura_receipt.block_hash() != wire_subject.block_hash
            || kura_receipt.subject() != wire_subject
            || kura_receipt.certificate() != wire_decision.as_ref()
            || kura_receipt.artifact_hash() != HashOf::new(artifact)
        {
            return Err(AdapterError::DurableCommitMismatch);
        }
        let reducer_receipt = reducer::DurableCommitReceipt::from_trusted_storage(
            context_id(self.wire_context.id()),
            self.wire_context.height,
            core_decision.subject(),
            core_decision.reference(),
        );
        let closed = self.reducer.finish_height(reducer_receipt)?;
        let retirement = reducer::WalRetirementAuthorization::from_finalized_height(&closed);
        if !retirement.matches_finalized_height(&closed) {
            return Err(AdapterError::DurableCommitMismatch);
        }
        Ok(FinalizedV2Height {
            wal: self.wal,
            serviced_candidate_store: self.serviced_candidate_store,
            retirement,
        })
    }
    /// Build the compact canonical status payload from durable reducer state.
    pub(crate) fn status(&mut self) -> Result<wire::SumeragiV2Status, AdapterError> {
        let durable = self.reducer.durable_state();
        let view = durable.current_view();
        let leader = self
            .registry
            .validator_index(self.reducer.context().leader(view))?;
        let locked_prepare_qc = durable
            .locked()
            .map(|certificate| {
                self.registry
                    .qc_to_wire(certificate, self.aggregator.as_ref())
                    .map(|certificate| certificate.as_ref())
            })
            .transpose()?;
        let highest_prepare_qc = durable
            .highest_prepare()
            .map(|certificate| {
                self.registry
                    .qc_to_wire(certificate, self.aggregator.as_ref())
                    .map(|certificate| certificate.as_ref())
            })
            .transpose()?;
        let last_timeout_certificate = durable
            .last_timeout()
            .map(|certificate| {
                self.registry
                    .tc_to_wire(certificate, self.aggregator.as_ref())
                    .map(|certificate| certificate.as_ref())
            })
            .transpose()?;
        let decision = durable.decision().cloned();
        let (last_committed_height, last_committed_subject, last_commit_qc) =
            if let Some(certificate) = &decision {
                let certificate = self
                    .registry
                    .qc_to_wire(certificate, self.aggregator.as_ref())?;
                (
                    certificate.round.height,
                    Some(certificate.subject),
                    Some(commit_qc_status(&certificate, &self.wire_context)?),
                )
            } else if let Some(parent) = &self.wire_context.parent_commit_qc {
                let verification = self
                    .parent_verification
                    .as_ref()
                    .ok_or(AdapterError::ParentContextMismatch)?;
                let summary = commit_qc_status(parent, &verification.context)?;
                (parent.round.height, Some(parent.subject), Some(summary))
            } else if let Some(anchor) = &self.wire_context.snapshot_bootstrap {
                (anchor.snapshot_height, None, None)
            } else {
                (0, None, None)
            };
        let validator_count = u32::try_from(self.wire_context.roster.len())
            .map_err(|_| wire::ValidationError::RosterTooLarge)?;
        let height_context = wire::SumeragiV2HeightContextStatus {
            epoch: self.wire_context.epoch,
            epoch_end_height: self.wire_context.epoch_end_height,
            mode: self.wire_context.mode,
            epoch_seed: self.wire_context.leader_seed,
            validator_count,
            quorum: self.wire_context.quorum,
        };
        let (phase, body_state) = if let Some(decision) = &decision {
            if self.reducer.applied_subject() == Some(decision.subject()) {
                (
                    wire::SumeragiV2StatusPhase::PendingApply,
                    wire::SumeragiV2BodyState::Applied,
                )
            } else {
                (
                    wire::SumeragiV2StatusPhase::PendingApply,
                    wire::SumeragiV2BodyState::PendingApply,
                )
            }
        } else if let Some((round, subject)) = self.active_subject {
            match self.reducer.body_state(round, subject) {
                reducer::BodyState::Missing => (
                    wire::SumeragiV2StatusPhase::ReconstructingPayload,
                    wire::SumeragiV2BodyState::Reconstructing,
                ),
                reducer::BodyState::Available => (
                    wire::SumeragiV2StatusPhase::ReconstructingPayload,
                    wire::SumeragiV2BodyState::Reconstructing,
                ),
                reducer::BodyState::Durable => (
                    wire::SumeragiV2StatusPhase::ValidatingPayload,
                    wire::SumeragiV2BodyState::Stored,
                ),
                reducer::BodyState::Validated => {
                    if durable.locked().is_some() {
                        (
                            wire::SumeragiV2StatusPhase::Commit,
                            wire::SumeragiV2BodyState::Validated,
                        )
                    } else {
                        (
                            wire::SumeragiV2StatusPhase::Prepare,
                            wire::SumeragiV2BodyState::Validated,
                        )
                    }
                }
                reducer::BodyState::Invalid => (
                    wire::SumeragiV2StatusPhase::AwaitingProposal,
                    wire::SumeragiV2BodyState::Missing,
                ),
            }
        } else {
            (
                wire::SumeragiV2StatusPhase::AwaitingProposal,
                wire::SumeragiV2BodyState::Missing,
            )
        };
        #[cfg(not(test))]
        let output_guard_restart_required =
            super::output_guard::process_consensus_output_guard().restart_required();
        #[cfg(test)]
        let output_guard_restart_required = false;
        let liveness = self.liveness_status()?;
        Ok(wire::SumeragiV2Status {
            protocol_version: wire::PROTOCOL_VERSION,
            node_fingerprint: self.fingerprints.node,
            build_fingerprint: self.fingerprints.build,
            config_fingerprint: self.fingerprints.config,
            restart_required: self.fail_closed || output_guard_restart_required,
            height_context_id: self.wire_context.id(),
            height: self.wire_context.height,
            view,
            phase,
            leader,
            locked_prepare_qc,
            highest_prepare_qc,
            last_timeout_certificate,
            body_state,
            pending_persistence_id: self.pending_persistence_id,
            last_committed_height,
            last_committed_subject,
            height_context,
            last_commit_qc,
            liveness,
        })
    }
    /// Record and snapshot the runner-owned live-successor boundary.
    ///
    /// The marker lives in the adapter rather than only in the global status
    /// registry. Consequently a later ignored input or retransmission cannot
    /// restore the older replay marker and erase the activation witness.
    pub(crate) fn successor_activation_status(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        let round = reducer::Round::new(
            self.reducer.context().height(),
            self.reducer.current_tag().view(),
        );
        self.last_progress = Some((
            self.reducer.generation(),
            round,
            wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
        ));
        let status = self.status()?;
        self.status_publication_enabled = true;
        Ok(status)
    }
    /// Snapshot and publish an applied PendingKura height without recording a
    /// successor marker or arming the pacemaker.
    pub(in crate::sumeragi) fn pending_kura_activation_status(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        let status = self.status()?;
        self.status_publication_enabled = true;
        Ok(status)
    }
    fn liveness_status(&mut self) -> Result<wire::SumeragiV2LivenessStatus, AdapterError> {
        let min_signers = u32::try_from(self.reducer.context().minimum_signer_count())
            .map_err(|_| wire::ValidationError::RosterTooLarge)?;
        let total_power = self.reducer.context().total_voting_power().get();
        let current_view = self.reducer.current_tag().view();
        let mut prepare_quorums = Vec::new();
        let mut commit_quorums = Vec::new();
        for snapshot in self.reducer.vote_pool_snapshots() {
            let quorum = wire::SumeragiV2VoteQuorumStatus {
                round: self.registry.round_to_wire(snapshot.round),
                proposal_round: self.registry.round_to_wire(snapshot.proposal_round),
                subject: self.registry.subject(snapshot.subject)?,
                execution_commitment: self
                    .registry
                    .execution_commitment(snapshot.proposal_round, snapshot.subject)?,
                signer_count: u32::try_from(snapshot.signers.len())
                    .map_err(|_| wire::ValidationError::TooManySigners)?,
                signed_power: snapshot.signed_power.get(),
                min_signers,
                total_power,
            };
            match snapshot.phase {
                reducer::Phase::Prepare => prepare_quorums.push(quorum),
                reducer::Phase::Commit => commit_quorums.push(quorum),
            }
        }
        let timeout_quorums = self
            .reducer
            .timeout_pool_snapshots()
            .into_iter()
            // The reducer deliberately retains the adjacent-future pool so a
            // lagging peer can form a TC and catch up. Public liveness status
            // remains a projection of the reported current view, however;
            // exposing that private catch-up pool would violate the status
            // schema's non-future-round contract.
            .filter(|snapshot| snapshot.round.view() <= current_view)
            .map(|snapshot| {
                Ok(wire::SumeragiV2TimeoutQuorumStatus {
                    round: self.registry.round_to_wire(snapshot.round),
                    signer_count: u32::try_from(snapshot.signers.len())
                        .map_err(|_| wire::ValidationError::TooManySigners)?,
                    signed_power: snapshot.signed_power.get(),
                    min_signers,
                    total_power,
                    certificate_formed: snapshot.certificate_formed,
                })
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let outbound_intents = self.outbound_intent_statuses()?;
        let work = self.local_work_status();
        let queues = self.adapter_queue_statuses();
        let last_progress = self
            .last_progress
            .filter(|(_, round, transition)| {
                progress_transition_is_public_at_view(*transition, round.view(), current_view)
            })
            .map(
                |(generation, round, transition)| wire::SumeragiV2ProgressTransitionStatus {
                    generation: generation.get(),
                    round: self.registry.round_to_wire(round),
                    transition,
                    age_ms: 0,
                },
            );
        let ignore_counts = ALL_IGNORE_REASONS
            .into_iter()
            .map(|(core, wire)| wire::SumeragiV2IgnoreCount {
                reason: wire,
                count: self.ignore_counts.get(&core).copied().unwrap_or_default(),
            })
            .collect();
        Ok(wire::SumeragiV2LivenessStatus {
            generation: self.reducer.generation().get(),
            prepare_quorums,
            commit_quorums,
            timeout_quorums,
            outbound_intents,
            work,
            queues,
            last_progress,
            no_progress_age_ms: 0,
            blocker: None,
            ignore_counts,
        })
    }
    fn outbound_intent_statuses(
        &self,
    ) -> Result<Vec<wire::SumeragiV2OutboundIntentStatus>, AdapterError> {
        let mut intents = BTreeMap::<
            wire::SumeragiV2OutboundIntentKind,
            wire::SumeragiV2OutboundIntentStatus,
        >::new();
        if let Some(record) = self.reducer.pending_persistence_record() {
            let intent = self.intent_from_wal_record(
                record,
                wire::SumeragiV2OutboundIntentStage::PendingPersistence,
            )?;
            Self::retain_intent(&mut intents, intent);
        }
        if let Some(signable) = self.reducer.awaiting_signature() {
            let intent = self.intent_from_signable(
                signable,
                wire::SumeragiV2OutboundIntentStage::PendingSignature,
            )?;
            Self::retain_intent(&mut intents, intent);
        }
        for signable in self.reducer.queued_signatures() {
            let intent =
                self.intent_from_signable(signable, wire::SumeragiV2OutboundIntentStage::Queued)?;
            Self::retain_intent(&mut intents, intent);
        }
        for message in self.reducer.outbound_messages() {
            if let Some(intent) =
                self.intent_from_message(message, wire::SumeragiV2OutboundIntentStage::Sent)?
            {
                Self::retain_intent(&mut intents, intent);
            }
        }
        Ok(intents.into_values().collect())
    }
    fn retain_intent(
        intents: &mut BTreeMap<
            wire::SumeragiV2OutboundIntentKind,
            wire::SumeragiV2OutboundIntentStatus,
        >,
        candidate: wire::SumeragiV2OutboundIntentStatus,
    ) {
        let candidate_rank = outbound_stage_rank(candidate.stage);
        match intents.get_mut(&candidate.kind) {
            Some(current)
                if candidate_rank < outbound_stage_rank(current.stage)
                    || (candidate_rank == outbound_stage_rank(current.stage)
                        && candidate.round.view > current.round.view) =>
            {
                *current = candidate;
            }
            Some(_) => {}
            None => {
                intents.insert(candidate.kind, candidate);
            }
        }
    }
    fn intent_from_wal_record(
        &self,
        record: &reducer::WalRecord,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        match record {
            reducer::WalRecord::ProposalIntent(proposal) => {
                self.intent_for_proposal(proposal, stage)
            }
            reducer::WalRecord::PrepareIntent(vote) => self.intent_for_vote(vote, stage),
            reducer::WalRecord::ObservePrepare(certificate) => {
                self.intent_for_certificate(certificate, stage)
            }
            reducer::WalRecord::LockAndCommit { vote, .. } => self.intent_for_vote(vote, stage),
            reducer::WalRecord::TimeoutIntent(vote) => {
                Ok(self.intent_for_timeout_vote(vote, stage))
            }
            reducer::WalRecord::InstallTimeout(certificate) => {
                Ok(self.intent_for_timeout_certificate(certificate, stage))
            }
            reducer::WalRecord::Decision(certificate) => {
                self.intent_for_certificate(certificate, stage)
            }
        }
    }
    fn intent_from_signable(
        &self,
        signable: &reducer::SignableMessage,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        match signable {
            reducer::SignableMessage::Proposal(proposal) => {
                self.intent_for_proposal(proposal, stage)
            }
            reducer::SignableMessage::Vote(vote) => self.intent_for_vote(vote, stage),
            reducer::SignableMessage::TimeoutVote(vote) => {
                Ok(self.intent_for_timeout_vote(vote, stage))
            }
        }
    }
    fn intent_from_message(
        &self,
        message: &reducer::ConsensusMessageV2,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<Option<wire::SumeragiV2OutboundIntentStatus>, AdapterError> {
        let intent = match message {
            reducer::ConsensusMessageV2::Proposal(proposal) => {
                self.intent_for_proposal(proposal.proposal(), stage)?
            }
            reducer::ConsensusMessageV2::Vote(vote) => self.intent_for_vote(&vote.vote(), stage)?,
            reducer::ConsensusMessageV2::QuorumCertificate(certificate) => {
                self.intent_for_certificate(certificate, stage)?
            }
            reducer::ConsensusMessageV2::TimeoutVote(vote) => {
                self.intent_for_timeout_vote(&vote.vote(), stage)
            }
            reducer::ConsensusMessageV2::TimeoutCertificate(certificate) => {
                self.intent_for_timeout_certificate(certificate, stage)
            }
        };
        Ok(Some(intent))
    }
    fn intent_for_proposal(
        &self,
        proposal: &reducer::Proposal,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        Ok(wire::SumeragiV2OutboundIntentStatus {
            kind: wire::SumeragiV2OutboundIntentKind::Proposal,
            round: self.registry.round_to_wire(proposal.round()),
            proposal_round: Some(self.registry.round_to_wire(proposal.round())),
            subject: Some(self.registry.subject(proposal.manifest().subject())?),
            execution_commitment: None,
            stage,
        })
    }
    fn intent_for_vote(
        &self,
        vote: &reducer::Vote,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        Ok(wire::SumeragiV2OutboundIntentStatus {
            kind: match vote.phase() {
                reducer::Phase::Prepare => wire::SumeragiV2OutboundIntentKind::PrepareVote,
                reducer::Phase::Commit => wire::SumeragiV2OutboundIntentKind::CommitVote,
            },
            round: self.registry.round_to_wire(vote.round()),
            proposal_round: Some(self.registry.round_to_wire(vote.proposal_round())),
            subject: Some(self.registry.subject(vote.subject())?),
            execution_commitment: Some(
                self.registry
                    .execution_commitment(vote.proposal_round(), vote.subject())?,
            ),
            stage,
        })
    }
    fn intent_for_certificate(
        &self,
        certificate: &reducer::QuorumCertificate,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        Ok(wire::SumeragiV2OutboundIntentStatus {
            kind: match certificate.phase() {
                reducer::Phase::Prepare => wire::SumeragiV2OutboundIntentKind::PrepareQc,
                reducer::Phase::Commit => wire::SumeragiV2OutboundIntentKind::CommitQc,
            },
            round: self.registry.round_to_wire(certificate.round()),
            proposal_round: Some(self.registry.round_to_wire(certificate.proposal_round())),
            subject: Some(self.registry.subject(certificate.subject())?),
            execution_commitment: Some(
                self.registry
                    .execution_commitment(certificate.proposal_round(), certificate.subject())?,
            ),
            stage,
        })
    }
    fn intent_for_timeout_vote(
        &self,
        vote: &reducer::TimeoutVote,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> wire::SumeragiV2OutboundIntentStatus {
        wire::SumeragiV2OutboundIntentStatus {
            kind: wire::SumeragiV2OutboundIntentKind::TimeoutVote,
            round: self.registry.round_to_wire(vote.round()),
            proposal_round: None,
            subject: None,
            execution_commitment: None,
            stage,
        }
    }
    fn intent_for_timeout_certificate(
        &self,
        certificate: &reducer::TimeoutCertificate,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> wire::SumeragiV2OutboundIntentStatus {
        wire::SumeragiV2OutboundIntentStatus {
            kind: wire::SumeragiV2OutboundIntentKind::TimeoutCertificate,
            round: self.registry.round_to_wire(certificate.round()),
            proposal_round: None,
            subject: None,
            execution_commitment: None,
            stage,
        }
    }
    fn local_work_status(&self) -> wire::SumeragiV2WorkStatus {
        use wire::SumeragiV2LocalWorkStage::{Complete, Idle, Queued};
        let durable = self.reducer.durable_state();
        let decision = durable.decision();
        let applied = decision.is_some_and(|certificate| {
            self.reducer.applied_subject() == Some(certificate.subject())
        });
        let mut work = wire::SumeragiV2WorkStatus {
            candidate: if self.active_subject.is_some() {
                Complete
            } else {
                Idle
            },
            application: if decision.is_some() {
                if applied { Complete } else { Queued }
            } else {
                Idle
            },
            successor_height: if applied { Queued } else { Idle },
            ..wire::SumeragiV2WorkStatus::default()
        };
        if let Some((round, subject)) = self.active_subject {
            match self.reducer.body_state(round, subject) {
                reducer::BodyState::Missing => work.body_recovery = Queued,
                reducer::BodyState::Available => {
                    work.body_recovery = Complete;
                    work.body_store = Queued;
                }
                reducer::BodyState::Durable => {
                    work.body_recovery = Complete;
                    work.body_store = Complete;
                    work.validation = Queued;
                }
                reducer::BodyState::Validated | reducer::BodyState::Invalid => {
                    work.body_recovery = Complete;
                    work.body_store = Complete;
                    work.validation = Complete;
                }
            }
        }
        work
    }
    fn adapter_queue_statuses(&self) -> Vec<wire::SumeragiV2QueueStatus> {
        let now = Instant::now();
        let ingress_oldest = self
            .ingress_equivocations
            .values()
            .map(|record| record.admitted_at)
            .min();
        let progress_capacity = deferred_progress_capacity(self.wire_context.roster.len());
        vec![
            queue_status(
                wire::SumeragiV2QueueKind::Ingress,
                self.ingress_equivocations.len(),
                semantic_ingress_capacity(self.wire_context.roster.len()),
                ingress_oldest.map(|oldest| now.saturating_duration_since(oldest)),
                0,
            ),
            deferred_queue_status(
                wire::SumeragiV2QueueKind::DeferredNormal,
                &self.deferred_inputs,
                MAX_DEFERRED_INPUTS,
                now,
            ),
            deferred_queue_status(
                wire::SumeragiV2QueueKind::DeferredProgress,
                &self.deferred_progress_inputs,
                progress_capacity,
                now,
            ),
            deferred_queue_status(
                wire::SumeragiV2QueueKind::DeferredCompletion,
                &self.deferred_completions,
                MAX_DEFERRED_INPUTS,
                now,
            ),
        ]
    }
    fn serviced_candidate(
        &self,
        event: &reducer::Event,
        _priority: DeferredPriority,
        completion_evidence: Option<&BodyPipelineCompletionEvidence>,
        authenticated_wire_identity: Option<&[u8]>,
    ) -> Option<(ServicedCandidateKey, wire::View, ServicedCandidatePolicy)> {
        let policy = serviced_candidate_policy(event)?;
        let service_view = self.reducer.current_tag().view();
        let (source_view, target, phase) = serviced_candidate_event_fields(event);
        let leader = self.wire_context.leader(source_view);
        let owner: [u8; 32] = self.fingerprints.node.into();
        let mut projection = Vec::new();
        append_deferred_projection_field(&mut projection, &self.wire_context.id().encode());
        append_deferred_projection_u64(&mut projection, self.wire_context.height);
        append_deferred_projection_field(&mut projection, &owner);
        append_deferred_projection_field(&mut projection, &leader.encode());
        append_deferred_projection_u64(&mut projection, source_view);
        match target {
            Some(target) => {
                projection.push(1);
                append_deferred_projection_field(&mut projection, &target);
            }
            None => projection.push(0),
        }
        projection.push(phase);
        append_serviced_candidate_event(&mut projection, event);
        append_deferred_projection_completion_evidence(&mut projection, completion_evidence);
        // Authenticated network inputs have one semantic reducer occurrence
        // even when another valid signature, quorum subset, nested aggregate,
        // or canonical envelope carries them. The exact raw carrier remains
        // bound to deferred ownership and is revalidated immediately before
        // reducer service; after successful service it must not create a new
        // logical tombstone. Local completion evidence remains exact below.
        let carrier_identity = if matches!(
            event,
            reducer::Event::ProposalReceived { .. }
                | reducer::Event::VoteReceived { .. }
                | reducer::Event::QuorumCertificateReceived { .. }
                | reducer::Event::TimeoutVoteReceived { .. }
                | reducer::Event::TimeoutCertificateReceived { .. }
        ) {
            None
        } else {
            authenticated_wire_identity
        };
        match carrier_identity {
            Some(identity) => {
                projection.push(1);
                append_deferred_projection_field(&mut projection, identity);
            }
            None => projection.push(0),
        }
        let evidence: [u8; 32] = Hash::new(projection).into();
        Some((
            ServicedCandidateKey::new(
                self.wire_context.id(),
                self.wire_context.height,
                owner,
                leader,
                source_view,
                target,
                phase,
                ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
                deferred_event_kind(event).code(),
                evidence,
            ),
            service_view,
            policy,
        ))
    }
    fn fail_serviced_candidate_store(&mut self, reason: String) -> AdapterError {
        self.fail_closed = true;
        AdapterError::ServicedCandidateStore(reason)
    }
    /// Verify the sole producer-owner state permitted after a durable Decision.
    ///
    /// The Decision WAL acknowledgement reclaims every candidate and producer
    /// owner for the height in one adjacent durable snapshot. Producer tokens
    /// reserved before that acknowledgement are therefore obsolete once this
    /// canonical empty epoch is published; applying their undo payload would
    /// resurrect state which the Decision permanently retired.
    fn ensure_canonical_reclaimed_producer_state_after_decision(
        &mut self,
    ) -> Result<bool, AdapterError> {
        if self.reducer.durable_state().decision().is_none() {
            return Ok(false);
        }
        if !self.serviced_candidates_decision_reclaimed
            || !self.serviced_candidates.is_empty()
            || !self.durable_serviced_candidates.is_empty()
            || !self.producer_continuations.is_empty()
            || !self.durable_producer_continuations.is_empty()
            || !self.restored_dormant_producer_continuations.is_empty()
            || !self.deferred_producer_continuations.is_empty()
            || !self.pending_producer_handoffs.is_empty()
        {
            return Err(self.fail_serviced_candidate_store(
                "durable Decision did not retain canonical reclaimed producer state".to_owned(),
            ));
        }
        Ok(true)
    }
    /// Bind the immutable lifecycle selected by the serialized runtime to the
    /// next adapter transition.
    ///
    /// The runtime has already validated this carrier against its scheduler
    /// sidecar. Keeping the seam explicit prevents direct adapter tests and
    /// startup replay from accidentally minting production ownership.
    pub(crate) fn bind_selected_producer_lifecycle(
        &mut self,
        causal_lifecycle_key: Hash,
        admission_ordinal: u128,
    ) -> Result<(), AdapterError> {
        if admission_ordinal == 0 || self.selected_producer_lifecycle.is_some() {
            return Err(self.fail_serviced_candidate_store(
                "selected producer lifecycle was zero or already bound".to_owned(),
            ));
        }
        self.selected_producer_lifecycle = Some(SelectedProducerLifecycle {
            causal_lifecycle_key,
            admission_ordinal,
        });
        Ok(())
    }
    /// Clear the one-transition runtime binding.
    pub(crate) fn clear_selected_producer_lifecycle(&mut self) {
        self.selected_producer_lifecycle = None;
    }
    fn producer_lifecycle_slot(
        &self,
        candidate: ServicedCandidateKey,
        selected: &SelectedProducerLifecycle,
    ) -> Result<u64, String> {
        let mut existing_slot = None;
        for record in self.producer_continuations.values().filter(|record| {
            let identity = record.identity();
            identity.admission_ordinal() == selected.admission_ordinal
                && identity.causal_lifecycle_key() == selected.causal_lifecycle_key
        }) {
            let slot = record.identity().address().lifecycle_slot();
            if existing_slot
                .replace(slot)
                .is_some_and(|existing| existing != slot)
            {
                return Err("one producer lifecycle occupied multiple bounded slots".to_owned());
            }
        }
        if let Some(slot) = existing_slot {
            return Ok(slot);
        }
        (1..=self.producer_continuation_lifecycle_capacity)
            .find(|slot| {
                self.producer_continuations
                    .values()
                    .filter(|record| record.identity().address().lifecycle_slot() == *slot)
                    .all(|record| {
                        let identity = record.identity();
                        record.status() == ProducerContinuationStatus::Terminal
                            && identity.admission_ordinal() < selected.admission_ordinal
                            && identity.candidate().source_view() < candidate.source_view()
                    })
            })
            .ok_or_else(|| "bounded producer lifecycle slots are exhausted".to_owned())
    }
    /// Return whether an independently authenticated route selected a
    /// semantic producer which is already owned by one live Busy-deferred
    /// occurrence.
    ///
    /// Fair ingress deliberately gives distinct authenticated origins
    /// independent outer lifecycle tokens. Two such tokens can both reach the
    /// serialized FIFO before the first occurrence crosses into Busy storage;
    /// a later pacemaker escape may therefore select the second token while
    /// the first still owns the sole route-neutral producer continuation.
    /// Treat that second occurrence as an in-flight duplicate only after
    /// proving the original producer, deferred input, and runtime ownership
    /// seal form one exact live chain. Restart-dormant or otherwise unowned
    /// producer metadata must continue through the strict reservation path and
    /// fail closed on an immutable-identity mismatch.
    fn live_deferred_producer_alias(
        &mut self,
        candidate: (ServicedCandidateKey, wire::View, ServicedCandidatePolicy),
        authenticated_ingress: bool,
    ) -> Result<bool, AdapterError> {
        if !authenticated_ingress {
            return Ok(false);
        }
        let Some(selected) = self.selected_producer_lifecycle.clone() else {
            return Ok(false);
        };
        let matches = self
            .producer_continuations
            .iter()
            .filter(|(_, record)| record.identity().candidate() == candidate.0)
            .map(|(address, record)| (*address, record.clone()))
            .collect::<Vec<_>>();
        let (address, record) = match matches.as_slice() {
            [] => return Ok(false),
            [entry] => entry.clone(),
            _ => {
                return Err(self.fail_serviced_candidate_store(
                    "one logical producer candidate occupied multiple bounded addresses".to_owned(),
                ));
            }
        };
        let identity = record.identity();
        let same_key = identity.causal_lifecycle_key() == selected.causal_lifecycle_key;
        let same_ordinal = identity.admission_ordinal() == selected.admission_ordinal;
        if same_key && same_ordinal {
            return Ok(false);
        }
        if same_key || same_ordinal {
            return Err(self.fail_serviced_candidate_store(
                "live producer alias partially changed its immutable key or ordinal".to_owned(),
            ));
        }
        let owners = self
            .deferred_producer_continuations
            .iter()
            .filter(|(_, reservation)| reservation.address == address)
            .map(|(ordinal, reservation)| (*ordinal, reservation.clone()))
            .collect::<Vec<_>>();
        let (deferred_ordinal, reservation) = match owners.as_slice() {
            [] => return Ok(false),
            [entry] => entry.clone(),
            _ => {
                return Err(self.fail_serviced_candidate_store(
                    "one live producer continuation had multiple Busy owners".to_owned(),
                ));
            }
        };
        let inputs = self
            .deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .filter(|input| input.admission_ordinal == deferred_ordinal)
            .cloned()
            .collect::<Vec<_>>();
        let input = match inputs.as_slice() {
            [input] => input,
            [] => {
                return Err(self.fail_serviced_candidate_store(
                    "live producer continuation lost its Busy occurrence".to_owned(),
                ));
            }
            _ => {
                return Err(self.fail_serviced_candidate_store(
                    "one live producer continuation had multiple deferred occurrences".to_owned(),
                ));
            }
        };
        let input_candidate = self.serviced_candidate(
            &input.event,
            input.priority,
            input.completion_evidence.as_ref(),
            input.authenticated_wire_identity.as_deref(),
        );
        let runtime_binding = input.admission_capability.runtime_ownership.as_ref();
        let exact_live_owner = reservation.address == address
            && record.status() == ProducerContinuationStatus::Reserved
            && record.source_class() == ProducerContinuationSourceClass::ConditionalTransport
            && identity.address() == address
            && self.durable_producer_continuations.get(&address) == Some(&record)
            && !self
                .restored_dormant_producer_continuations
                .contains(&address)
            && !self.pending_producer_handoffs.contains_key(&address)
            && input_candidate == Some(candidate)
            && input.retag_authenticated_ingress
            && input.authenticated_wire_identity.is_some()
            && input.admission_capability.origin.is_authenticated()
            && Arc::ptr_eq(
                &input.admission_capability.source_identity,
                &self.deferred_admission_ordinals.identity,
            )
            && !input.admission_capability.adapter_service_is_claimed()
            && !input.admission_capability.runtime_handoff_is_claimed()
            && runtime_binding.is_some_and(|binding| {
                binding.validate_exact()
                    && binding.authenticated_ingress
                    && binding.source_physical_ordinal.is_some()
                    && binding.causal_lifecycle_key == identity.causal_lifecycle_key()
                    && binding.initial_lifecycle_ordinal == identity.admission_ordinal()
            });
        if !exact_live_owner {
            return Err(self.fail_serviced_candidate_store(
                "live producer alias did not retain one exact authenticated Busy owner".to_owned(),
            ));
        }
        Ok(true)
    }
    /// Reserve the exact selected lifecycle-stage address before reducer
    /// service can retire its source.
    fn reserve_selected_producer_continuation(
        &mut self,
        candidate: Option<(ServicedCandidateKey, wire::View, ServicedCandidatePolicy)>,
    ) -> Result<Option<ProducerReservationToken>, AdapterError> {
        if self.ensure_canonical_reclaimed_producer_state_after_decision()? {
            // The durable Decision is the sole restart owner for the rest of
            // this height. Reclamation publishes an empty owner epoch before
            // any post-Decision application, timer, or queued ingress can be
            // serviced. Reserving another producer here would persist a live
            // owner beside `decision_reclaimed = true`, contradicting that
            // durable boundary before the reducer can discard the occurrence.
            return Ok(None);
        }
        let (Some((candidate, _, _)), Some(selected)) =
            (candidate, self.selected_producer_lifecycle.clone())
        else {
            return Ok(None);
        };
        let existing = self
            .producer_continuations
            .iter()
            .filter(|(_, record)| record.identity().candidate() == candidate)
            .map(|(address, _)| *address)
            .collect::<Vec<_>>();
        match existing.as_slice() {
            [address] => {
                let record = self.producer_continuations[address].clone();
                if self.durable_producer_continuations.get(address) != Some(&record) {
                    return Err(self.fail_serviced_candidate_store(
                        "active producer identity was not present in durable admission metadata"
                            .to_owned(),
                    ));
                }
                if record.status() != ProducerContinuationStatus::Reserved {
                    return Err(self.fail_serviced_candidate_store(
                        "a terminal producer identity reached live reservation".to_owned(),
                    ));
                }
                let identity = record.identity();
                if identity.admission_ordinal() != selected.admission_ordinal
                    || identity.causal_lifecycle_key() != selected.causal_lifecycle_key
                {
                    return Err(self.fail_serviced_candidate_store(
                        "replayed producer lifecycle changed its immutable key or ordinal"
                            .to_owned(),
                    ));
                }
                let change = if self.restored_dormant_producer_continuations.remove(address) {
                    ProducerReservationChange::ClaimedDormant
                } else {
                    ProducerReservationChange::Unchanged
                };
                if matches!(change, ProducerReservationChange::ClaimedDormant)
                    && (self
                        .deferred_producer_continuations
                        .values()
                        .any(|reservation| reservation.address == *address)
                        || self.pending_producer_handoffs.contains_key(address))
                {
                    self.restored_dormant_producer_continuations
                        .insert(*address);
                    return Err(self.fail_serviced_candidate_store(
                        "restart-dormant producer already had a live process alias".to_owned(),
                    ));
                }
                return Ok(Some(ProducerReservationToken {
                    address: *address,
                    change,
                }));
            }
            [] => {}
            _ => {
                return Err(self.fail_serviced_candidate_store(
                    "one logical producer candidate occupied multiple bounded addresses".to_owned(),
                ));
            }
        }
        let lifecycle_slot = self
            .producer_lifecycle_slot(candidate, &selected)
            .map_err(|reason| self.fail_serviced_candidate_store(reason))?;
        let identity = ProducerContinuationIdentity::new(
            candidate,
            selected.causal_lifecycle_key,
            lifecycle_slot,
            selected.admission_ordinal,
        )
        .map_err(|reason| self.fail_serviced_candidate_store(reason))?;
        let address = identity.address();
        let record = ProducerContinuationRecord::new(
            identity,
            ProducerContinuationStatus::Reserved,
            Vec::new(),
        )
        .map_err(|reason| self.fail_serviced_candidate_store(reason))?;
        let process_previous = self.producer_continuations.get(&address).cloned();
        let reservation = self
            .serviced_candidate_store
            .reserve_producer_continuation(&mut self.producer_continuations, record);
        let reservation = match reservation {
            Ok(reservation) => reservation,
            Err(reason) => return Err(self.fail_serviced_candidate_store(reason)),
        };
        let durable_previous = self
            .durable_producer_continuations
            .insert(address, self.producer_continuations[&address].clone());
        let change = match reservation {
            ProducerContinuationReservation::Inserted => ProducerReservationChange::Inserted,
            ProducerContinuationReservation::Coalesced => ProducerReservationChange::Unchanged,
            ProducerContinuationReservation::ReplacedTerminal => {
                ProducerReservationChange::ReplacedTerminal {
                    process_previous: process_previous.ok_or_else(|| {
                        self.fail_serviced_candidate_store(
                            "terminal replacement omitted its process incumbent".to_owned(),
                        )
                    })?,
                    durable_previous: durable_previous.clone(),
                }
            }
        };
        if let Err(reason) = self
            .serviced_candidate_store
            .persist_with_producer_continuations(
                &self.durable_serviced_candidates,
                &self.durable_producer_continuations,
                self.serviced_candidates_decision_reclaimed,
            )
        {
            match durable_previous {
                Some(previous) => {
                    self.durable_producer_continuations
                        .insert(address, previous);
                }
                None => {
                    self.durable_producer_continuations.remove(&address);
                }
            }
            match &change {
                ProducerReservationChange::Unchanged => {}
                ProducerReservationChange::Inserted => {
                    self.producer_continuations.remove(&address);
                }
                ProducerReservationChange::ClaimedDormant => {
                    self.restored_dormant_producer_continuations.insert(address);
                }
                ProducerReservationChange::ReplacedTerminal {
                    process_previous, ..
                } => {
                    self.producer_continuations
                        .insert(address, process_previous.clone());
                }
            }
            return Err(self.fail_serviced_candidate_store(reason));
        }
        Ok(Some(ProducerReservationToken { address, change }))
    }
    fn persist_producer_lifecycles(&mut self) -> Result<(), AdapterError> {
        self.serviced_candidate_store
            .persist_with_producer_continuations(
                &self.durable_serviced_candidates,
                &self.durable_producer_continuations,
                self.serviced_candidates_decision_reclaimed,
            )
            .map_err(|reason| self.fail_serviced_candidate_store(reason))
    }
    fn rollback_producer_reservation(
        &mut self,
        token: Option<ProducerReservationToken>,
    ) -> Result<(), AdapterError> {
        let Some(token) = token else {
            return Ok(());
        };
        self.pending_producer_handoffs.remove(&token.address);
        match token.change {
            ProducerReservationChange::Unchanged => return Ok(()),
            ProducerReservationChange::Inserted => {
                self.producer_continuations.remove(&token.address);
                self.durable_producer_continuations.remove(&token.address);
            }
            ProducerReservationChange::ClaimedDormant => {
                self.restored_dormant_producer_continuations
                    .insert(token.address);
            }
            ProducerReservationChange::ReplacedTerminal {
                process_previous,
                durable_previous,
            } => {
                self.producer_continuations
                    .insert(token.address, process_previous);
                match durable_previous {
                    Some(previous) => {
                        self.durable_producer_continuations
                            .insert(token.address, previous);
                    }
                    None => {
                        self.durable_producer_continuations.remove(&token.address);
                    }
                }
            }
        }
        self.persist_producer_lifecycles()
    }
    fn release_unrecorded_producer(
        &mut self,
        token: Option<ProducerReservationToken>,
    ) -> Result<(), AdapterError> {
        let Some(token) = token else {
            return Ok(());
        };
        self.persist_unrecorded_producer_releases(std::slice::from_ref(&token))
    }
    /// Publish one or more producer releases as a single durable transition.
    ///
    /// Every caller still owns the source occurrence while this method runs.
    /// The durable snapshot is therefore the first externally visible step:
    /// persistence failure restores every process alias, and success permits
    /// the caller to remove its volatile queue owner without a restart window
    /// which could resurrect the retired producer.
    fn persist_unrecorded_producer_releases(
        &mut self,
        tokens: &[ProducerReservationToken],
    ) -> Result<(), AdapterError> {
        if tokens.is_empty() {
            return Ok(());
        }
        if self.ensure_canonical_reclaimed_producer_state_after_decision()? {
            // Decision reclamation is the authoritative release for every
            // token reserved before its WAL acknowledgement. Applying an old
            // token's undo payload here would resurrect the reclaimed epoch.
            return Ok(());
        }
        let mut addresses = BTreeSet::new();
        for token in tokens {
            if !addresses.insert(token.address) {
                return Err(self.fail_serviced_candidate_store(
                    "one producer address had multiple simultaneous release authorities".to_owned(),
                ));
            }
            let Some(current) = self.producer_continuations.get(&token.address) else {
                return Err(self.fail_serviced_candidate_store(
                    "producer release authority lost its process record".to_owned(),
                ));
            };
            if current.status() != ProducerContinuationStatus::Reserved
                || current.identity().address() != token.address
                || self.durable_producer_continuations.get(&token.address) != Some(current)
                || self.pending_producer_handoffs.contains_key(&token.address)
            {
                return Err(self.fail_serviced_candidate_store(
                    "producer release authority did not own one exact durable reservation"
                        .to_owned(),
                ));
            }
            let dormant = self
                .restored_dormant_producer_continuations
                .contains(&token.address);
            if dormant {
                return Err(self.fail_serviced_candidate_store(
                    "a live producer release authority was still restart-dormant".to_owned(),
                ));
            }
        }
        let process_previous = self.producer_continuations.clone();
        let durable_previous = self.durable_producer_continuations.clone();
        let dormant_previous = self.restored_dormant_producer_continuations.clone();
        let handoffs_previous = self.pending_producer_handoffs.clone();
        for token in tokens {
            self.pending_producer_handoffs.remove(&token.address);
            match &token.change {
                ProducerReservationChange::Unchanged
                | ProducerReservationChange::Inserted
                | ProducerReservationChange::ClaimedDormant => {
                    self.producer_continuations.remove(&token.address);
                    self.durable_producer_continuations.remove(&token.address);
                    self.restored_dormant_producer_continuations
                        .remove(&token.address);
                }
                ProducerReservationChange::ReplacedTerminal {
                    process_previous,
                    durable_previous,
                } => {
                    self.producer_continuations
                        .insert(token.address, process_previous.clone());
                    match durable_previous {
                        Some(previous) => {
                            self.durable_producer_continuations
                                .insert(token.address, previous.clone());
                        }
                        None => {
                            self.durable_producer_continuations.remove(&token.address);
                        }
                    }
                }
            }
        }
        if let Err(reason) = self
            .serviced_candidate_store
            .persist_with_producer_continuations(
                &self.durable_serviced_candidates,
                &self.durable_producer_continuations,
                self.serviced_candidates_decision_reclaimed,
            )
        {
            self.producer_continuations = process_previous;
            self.durable_producer_continuations = durable_previous;
            self.restored_dormant_producer_continuations = dormant_previous;
            self.pending_producer_handoffs = handoffs_previous;
            return Err(self.fail_serviced_candidate_store(reason));
        }
        Ok(())
    }
    fn terminalize_producer_continuation(
        &mut self,
        address: Option<ProducerContinuationAddress>,
    ) -> Result<Option<ProducerContinuationRecord>, AdapterError> {
        let Some(address) = address else {
            return Ok(None);
        };
        let Some(previous) = self.producer_continuations.get(&address).cloned() else {
            return Err(self.fail_serviced_candidate_store(
                "selected producer reservation disappeared before terminalization".to_owned(),
            ));
        };
        if previous.status() == ProducerContinuationStatus::Terminal {
            return Ok(Some(previous));
        }
        let terminal = ProducerContinuationRecord::new(
            previous.identity(),
            ProducerContinuationStatus::Terminal,
            Vec::new(),
        )
        .map_err(|reason| self.fail_serviced_candidate_store(reason))?;
        self.producer_continuations.insert(address, terminal);
        Ok(Some(previous))
    }
    /// Persistently release producer reservations before their exact
    /// adapter-owned Busy occurrences are removed.
    fn release_deferred_producer_continuations_before_owner_removal(
        &mut self,
        retiring: &BTreeSet<u128>,
    ) -> Result<(), AdapterError> {
        let active = self.all_deferred_admission_ordinals();
        if !retiring.is_subset(&active)
            || !self
                .deferred_producer_continuations
                .keys()
                .all(|ordinal| active.contains(ordinal))
        {
            return Err(self.fail_serviced_candidate_store(
                "deferred producer release did not retain one exact Busy owner".to_owned(),
            ));
        }
        let tokens = self
            .deferred_producer_continuations
            .iter()
            .filter(|(ordinal, _)| retiring.contains(ordinal))
            .map(|(_, reservation)| reservation.clone())
            .collect::<Vec<_>>();
        self.persist_unrecorded_producer_releases(&tokens)?;
        for ordinal in retiring {
            self.deferred_producer_continuations.remove(ordinal);
        }
        Ok(())
    }
    /// Release a speculative active record when the same macro-step reached a
    /// durable goal (Decision or strict view advance) before a producer
    /// continuation was needed. A strict view advance restores an exact older
    /// incumbent temporarily replaced at the same bounded address. A Decision
    /// instead discards the obsolete token after verifying its canonical empty
    /// owner epoch.
    fn release_goal_reached_producer(
        &mut self,
        reservation: Option<ProducerReservationToken>,
    ) -> Result<(), AdapterError> {
        self.release_unrecorded_producer(reservation)
    }
    /// Reserve bounded serviced-identity capacity before mutating the reducer.
    ///
    /// The fast path needs no speculative reducer step. Only a theoretically
    /// full table projects the deterministic transition on a clone, allowing
    /// ignored Byzantine/stale traffic to remain marker-free while refusing a
    /// genuinely consuming transition before its physical owner is released.
    fn ensure_serviced_candidate_capacity_before_step(
        &mut self,
        event: &reducer::Event,
        candidate: Option<(ServicedCandidateKey, wire::View, ServicedCandidatePolicy)>,
    ) -> Result<(), AdapterError> {
        let Some((key, _, _)) = candidate else {
            return Ok(());
        };
        if self.serviced_candidates.contains_key(&key) {
            return Ok(());
        }
        let capacity = self.serviced_candidate_capacity;
        if self.serviced_candidates.len() < capacity {
            return Ok(());
        }
        let mut projected = self.reducer.clone();
        let disposition = projected.step(event.clone())?.disposition();
        if serviced_candidate_record_kind(event, disposition).is_none() {
            return Ok(());
        }
        Err(self.fail_serviced_candidate_store(format!(
            "derived serviced-candidate capacity {capacity} is exhausted before semantic service"
        )))
    }
    /// Mark one non-Busy reducer occurrence before its final owner is returned
    /// to the caller or serialized runtime.
    ///
    /// Applied occurrences retain a process-generation marker through the
    /// same-view episode. Only an exact internal callback drained after its
    /// asynchronous item disappeared also receives the restart-stable marker.
    fn record_serviced_candidate(
        &mut self,
        candidate: Option<(ServicedCandidateKey, wire::View, ServicedCandidatePolicy)>,
        durable_terminal_retirement: bool,
        durable_terminal_evidence: bool,
        producer_reservation: Option<ProducerReservationToken>,
    ) -> Result<Option<ProducerContinuationHandoffToken>, AdapterError> {
        let Some((key, service_view, _)) = candidate else {
            self.release_unrecorded_producer(producer_reservation)?;
            return Ok(None);
        };
        if self.reducer.durable_state().decision().is_some() {
            // Decision closes this height's candidate-service episode. The
            // reducer's durable Decision owns all remaining application and
            // replay progress, so no post-Decision occurrence may recreate a
            // tombstone reclaimed by that same macro-step.
            self.release_goal_reached_producer(producer_reservation)?;
            return Ok(None);
        }
        if service_view < self.reducer.current_tag().view() {
            // The same macro-step durably advanced the view and reclaimed the
            // completed old-view epoch before this owner reached its return
            // seam. Recreating that obsolete key would undo strict-view
            // reclamation.
            self.release_goal_reached_producer(producer_reservation)?;
            return Ok(None);
        }
        let capacity = self.serviced_candidate_capacity;
        let process_marker_exists = self.serviced_candidates.contains_key(&key);
        if !process_marker_exists && self.serviced_candidates.len() >= capacity {
            return Err(self.fail_serviced_candidate_store(format!(
                "derived serviced-candidate capacity {capacity} is exhausted"
            )));
        }
        if !process_marker_exists {
            assert_eq!(self.serviced_candidates.insert(key, service_view), None);
        }
        let Some(reservation) = producer_reservation else {
            if !durable_terminal_retirement || self.durable_serviced_candidates.contains_key(&key) {
                return Ok(None);
            }
            if self.durable_serviced_candidates.len() >= capacity {
                if !process_marker_exists {
                    self.serviced_candidates.remove(&key);
                }
                return Err(self.fail_serviced_candidate_store(format!(
                    "derived durable serviced-candidate capacity {capacity} is exhausted"
                )));
            }
            assert_eq!(
                self.durable_serviced_candidates.insert(key, service_view),
                None
            );
            if let Err(reason) = self
                .serviced_candidate_store
                .persist_with_producer_continuations(
                    &self.durable_serviced_candidates,
                    &self.durable_producer_continuations,
                    self.serviced_candidates_decision_reclaimed,
                )
            {
                self.durable_serviced_candidates.remove(&key);
                if !process_marker_exists {
                    self.serviced_candidates.remove(&key);
                }
                return Err(self.fail_serviced_candidate_store(reason));
            }
            return Ok(None);
        };
        let address = reservation.address;
        let token = self
            .producer_continuations
            .get(&address)
            .and_then(ProducerContinuationRecord::handoff_token)
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "selected producer reservation was not live at the runtime handoff".to_owned(),
                )
            })?;
        let consumes_volatile_dormant_body = matches!(
            &reservation.change,
            ProducerReservationChange::ClaimedDormant
        ) && token.identity().stage()
            == ServicedCandidateStage::BodyAvailable as u8;
        let pending = PendingProducerHandoff {
            token,
            service_view,
            // Stage 7 persists only the logical producer lifecycle. The body
            // bytes and physical FIFO owner are deliberately reacquired after
            // restart, so servicing that restored BodyAvailable consumes the
            // durable reservation instead of replacing it with a terminal
            // which could resurrect the already-spent reconstruction.
            durable_store_terminal: durable_terminal_retirement && !consumes_volatile_dormant_body,
            durable_terminal_evidence: durable_terminal_evidence && !consumes_volatile_dormant_body,
            durable_previous: match reservation.change {
                ProducerReservationChange::ReplacedTerminal {
                    durable_previous, ..
                } => durable_previous,
                ProducerReservationChange::Unchanged
                | ProducerReservationChange::Inserted
                | ProducerReservationChange::ClaimedDormant => None,
            },
        };
        match self.pending_producer_handoffs.get(&address) {
            Some(existing) if *existing != pending => {
                return Err(self.fail_serviced_candidate_store(
                    "an exact producer handoff changed its terminal policy".to_owned(),
                ));
            }
            Some(_) => {}
            None => {
                self.pending_producer_handoffs.insert(address, pending);
            }
        }
        Ok(Some(token))
    }
    /// Classify the exact replacement evidence retained for one pending handoff.
    ///
    /// A non-empty effect batch is a concrete causal successor. An empty batch
    /// is restart-stable only when the source retained independent durable
    /// terminal evidence; every other empty last consumer is explicitly
    /// volatile and reopens after same-height restart.
    pub(crate) fn producer_handoff_evidence(
        &self,
        token: ProducerContinuationHandoffToken,
        has_concrete_successor: bool,
    ) -> Result<ProducerContinuationHandoffEvidence, AdapterError> {
        let pending = self
            .pending_producer_handoffs
            .get(&token.address())
            .ok_or(AdapterError::RuntimeIngressOwnershipViolation)?;
        if pending.token != token {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        Ok(if has_concrete_successor {
            ProducerContinuationHandoffEvidence::ConcreteSuccessor
        } else if pending.durable_terminal_evidence {
            ProducerContinuationHandoffEvidence::DurableTerminal
        } else {
            ProducerContinuationHandoffEvidence::VolatileTerminal
        })
    }
    /// Consume one exact runtime handoff after its replacement owner exists.
    ///
    /// The opaque token is checked against both the live continuation record
    /// and the pending service metadata. Durable publication commits the
    /// service tombstone and producer terminal in one source-sealed snapshot.
    pub(crate) fn acknowledge_producer_handoff(
        &mut self,
        token: ProducerContinuationHandoffToken,
        evidence: ProducerContinuationHandoffEvidence,
    ) -> Result<ProducerContinuationTerminalToken, AdapterError> {
        let address = token.address();
        let pending = self
            .pending_producer_handoffs
            .get(&address)
            .cloned()
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "producer handoff acknowledgement had no pending reservation".to_owned(),
                )
            })?;
        let record = self
            .producer_continuations
            .get(&address)
            .cloned()
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "producer handoff acknowledgement lost its reservation".to_owned(),
                )
            })?;
        if pending.token != token || !token.matches_reserved(&record) {
            return Err(self.fail_serviced_candidate_store(
                "producer handoff acknowledgement changed exact identity".to_owned(),
            ));
        }
        if evidence == ProducerContinuationHandoffEvidence::DurableTerminal
            && !pending.durable_terminal_evidence
        {
            return Err(self.fail_serviced_candidate_store(
                "producer handoff claimed terminal evidence not retained by its source".to_owned(),
            ));
        }
        if evidence == ProducerContinuationHandoffEvidence::VolatileTerminal
            && pending.durable_terminal_evidence
        {
            return Err(self.fail_serviced_candidate_store(
                "producer handoff weakened retained durable terminal evidence".to_owned(),
            ));
        }
        let previous = self
            .terminalize_producer_continuation(Some(address))?
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "producer handoff terminalization returned no incumbent".to_owned(),
                )
            })?;
        let terminal = self
            .producer_continuations
            .get(&address)
            .cloned()
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "producer handoff terminal disappeared after terminalization".to_owned(),
                )
            })?;
        if pending.durable_store_terminal {
            let key = token.identity().candidate();
            let previous_service = self
                .durable_serviced_candidates
                .insert(key, pending.service_view);
            let previous_durable = self
                .durable_producer_continuations
                .insert(address, terminal.clone());
            if let Err(reason) = self
                .serviced_candidate_store
                .persist_with_producer_continuations(
                    &self.durable_serviced_candidates,
                    &self.durable_producer_continuations,
                    self.serviced_candidates_decision_reclaimed,
                )
            {
                match previous_service {
                    Some(view) => {
                        self.durable_serviced_candidates.insert(key, view);
                    }
                    None => {
                        self.durable_serviced_candidates.remove(&key);
                    }
                }
                match previous_durable {
                    Some(record) => {
                        self.durable_producer_continuations.insert(address, record);
                    }
                    None => {
                        self.durable_producer_continuations.remove(&address);
                    }
                }
                self.producer_continuations.insert(address, previous);
                return Err(self.fail_serviced_candidate_store(reason));
            }
        } else {
            match pending.durable_previous.clone() {
                Some(previous) => {
                    self.durable_producer_continuations
                        .insert(address, previous);
                }
                None => {
                    self.durable_producer_continuations.remove(&address);
                }
            }
            if let Err(error) = self.persist_producer_lifecycles() {
                self.durable_producer_continuations.insert(address, record);
                self.producer_continuations.insert(address, previous);
                return Err(error);
            }
        }
        self.pending_producer_handoffs.remove(&address);
        self.restored_dormant_producer_continuations
            .remove(&address);
        terminal.terminal_token().ok_or_else(|| {
            self.fail_serviced_candidate_store(
                "producer handoff did not produce an exact terminal token".to_owned(),
            )
        })
    }
    /// Return the safety-WAL replay cut used to reconcile generic ingress.
    ///
    /// The adapter is opened before the adjacent leader-wire store. Its
    /// current view is therefore backed by replayed timeout-certificate state,
    /// and its Decision bit is backed by the durable Decision record. The
    /// opaque capability lets the generic store retire only records whose
    /// protocol episode is already impossible to re-enter.
    pub(crate) fn leader_wire_recovery_authority(
        &self,
    ) -> Result<LeaderWireRecoveryAuthority, AdapterError> {
        LeaderWireRecoveryAuthority::from_adapter(self)
    }
    /// Mint the sole fixed leader-wire sibling owner from this exact open WAL.
    pub(crate) fn mint_leader_wire_store_authority(
        &self,
        expected_wal_path: &std::path::Path,
    ) -> Result<SafetyWalLeaderWireStoreAuthority, AdapterError> {
        self.ensure_ingress()?;
        self.wal
            .mint_leader_wire_store_authority(expected_wal_path)
            .map_err(AdapterError::from)
    }
    /// Read-only restart-stable producer terminals restored from the adjacent
    /// serviced-candidate snapshot.
    pub(crate) fn durable_producer_terminal_tokens(
        &self,
    ) -> Vec<ProducerContinuationTerminalToken> {
        self.durable_producer_continuations
            .values()
            .filter_map(ProducerContinuationRecord::terminal_token)
            .collect()
    }
    /// Reconcile restart-only Reserved producers against the replayed durable
    /// frontier before the runtime can install dormant capacity.
    ///
    /// During a live EnterView macro-step an older Reserved record still has a
    /// process owner and must survive until that owner explicitly hands off.
    /// After a crash no such alias exists. The replayed current view therefore
    /// retires every older Reserved producer except the exact body pipeline of
    /// the durable protected lock. Persisting this cut before runtime creation
    /// closes the WAL-before-executor-cleanup crash window.
    fn reconcile_restored_reserved_producer_frontier(&mut self) -> Result<(), AdapterError> {
        if self.reducer.durable_state().decision().is_some() {
            // Decision reclamation below owns the stronger all-producer cut.
            return Ok(());
        }
        let current_view = self.reducer.current_tag().view();
        let protected = self.reducer.durable_state().locked().map(|certificate| {
            (
                certificate.proposal_round().view(),
                *certificate.subject().as_bytes(),
            )
        });
        let restored = self
            .producer_continuations
            .iter()
            .map(|(address, record)| (*address, record.clone()))
            .collect::<Vec<_>>();
        let mut retiring = Vec::new();
        for (address, record) in restored {
            if record.status() == ProducerContinuationStatus::Terminal {
                continue;
            }
            if record.status() != ProducerContinuationStatus::Reserved
                || record.identity().address() != address
                || self.durable_producer_continuations.get(&address) != Some(&record)
                || !self
                    .restored_dormant_producer_continuations
                    .contains(&address)
            {
                return Err(self.fail_serviced_candidate_store(
                    "restored producer frontier contained an inexact Reserved alias".to_owned(),
                ));
            }
            let identity = record.identity();
            let candidate = identity.candidate();
            if candidate.source_view() > current_view {
                return Err(self.fail_serviced_candidate_store(
                    "restored producer originated beyond the replayed durable view".to_owned(),
                ));
            }
            if candidate.source_view() == current_view {
                continue;
            }
            let stage = ServicedCandidateStage::from_code(identity.stage()).ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "restored producer frontier carried an unknown stage".to_owned(),
                )
            })?;
            let protects_body_pipeline = protected.is_some_and(|(view, subject)| {
                candidate.source_view() == view
                    && candidate.target() == Some(subject)
                    && matches!(
                        stage,
                        ServicedCandidateStage::LocalProposalReady
                            | ServicedCandidateStage::BodyAvailable
                            | ServicedCandidateStage::BodyStored
                            | ServicedCandidateStage::ValidationCompleted
                    )
            });
            if !protects_body_pipeline {
                retiring.push(address);
            }
        }
        if retiring.is_empty() {
            return Ok(());
        }
        let process_previous = self.producer_continuations.clone();
        let durable_previous = self.durable_producer_continuations.clone();
        let dormant_previous = self.restored_dormant_producer_continuations.clone();
        for address in retiring {
            self.producer_continuations.remove(&address);
            self.durable_producer_continuations.remove(&address);
            self.restored_dormant_producer_continuations
                .remove(&address);
        }
        if let Err(reason) = self
            .serviced_candidate_store
            .persist_with_producer_continuations(
                &self.durable_serviced_candidates,
                &self.durable_producer_continuations,
                self.serviced_candidates_decision_reclaimed,
            )
        {
            self.producer_continuations = process_previous;
            self.durable_producer_continuations = durable_previous;
            self.restored_dormant_producer_continuations = dormant_previous;
            return Err(self.fail_serviced_candidate_store(reason));
        }
        Ok(())
    }
    /// Reclaim only epochs made obsolete by a strict certified view advance
    /// or by the first durable Decision in this height.
    fn reclaim_serviced_candidates(&mut self) -> Result<(), AdapterError> {
        let current_view = self.reducer.current_tag().view();
        let decision_durable = self.reducer.durable_state().decision().is_some();
        if self.serviced_candidates_decision_reclaimed && !decision_durable {
            return Err(self.fail_serviced_candidate_store(
                "snapshot claims durable-Decision reclamation before a durable Decision".to_owned(),
            ));
        }
        let retired_process_candidates = self
            .serviced_candidates
            .iter()
            .filter_map(|(candidate, service_view)| {
                (*service_view < current_view).then_some(*candidate)
            })
            .collect::<BTreeSet<_>>();
        self.serviced_candidates
            .retain(|_, service_view| *service_view >= current_view);
        let previous_durable_len = self.durable_serviced_candidates.len();
        let previous_durable_producer_len = self.durable_producer_continuations.len();
        self.durable_serviced_candidates
            .retain(|_, service_view| *service_view >= current_view);
        if !decision_durable {
            // A strict certified view advance is itself the durable reason an
            // older terminal lifecycle cannot re-enter. Remove its paired
            // producer tombstone whenever the exact service tombstone is
            // reclaimed so every non-Decision snapshot keeps the two tables
            // atomic. A live reservation is different: its runtime owner can
            // still be completing the exact transition which installed the
            // newer view. Retain that restart-safe admission metadata until
            // the owner explicitly hands off or releases it; dropping only
            // the durable half here leaves a process-local ghost which the
            // next legitimate retry must fail closed.
            self.durable_producer_continuations.retain(|_, record| {
                if record.status() == ProducerContinuationStatus::Terminal {
                    self.durable_serviced_candidates
                        .contains_key(&record.identity().candidate())
                } else {
                    true
                }
            });
            // Process-only terminals close same-episode ABA retries, but a
            // certified view advance is already the durable retirement reason
            // for every service marker pruned above. Retaining only this half
            // of the pair makes a retagged deferred occurrence look like a
            // corrupt live reservation. Reclaim the terminal with its exact
            // process marker while preserving every Reserved owner.
            self.producer_continuations.retain(|_, record| {
                record.status() != ProducerContinuationStatus::Terminal
                    || !retired_process_candidates.contains(&record.identity().candidate())
            });
        }
        let mut durable_changed = self.durable_serviced_candidates.len() != previous_durable_len
            || self.durable_producer_continuations.len() != previous_durable_producer_len;
        if decision_durable && !self.serviced_candidates_decision_reclaimed {
            self.serviced_candidates.clear();
            self.durable_serviced_candidates.clear();
            self.producer_continuations.clear();
            self.durable_producer_continuations.clear();
            self.restored_dormant_producer_continuations.clear();
            self.deferred_producer_continuations.clear();
            self.pending_producer_handoffs.clear();
            self.serviced_candidates_decision_reclaimed = true;
            durable_changed = true;
        }
        let retired_dormant = self
            .restored_dormant_producer_continuations
            .iter()
            .filter(|address| !self.durable_producer_continuations.contains_key(address))
            .copied()
            .collect::<Vec<_>>();
        for address in retired_dormant {
            self.restored_dormant_producer_continuations
                .remove(&address);
            self.producer_continuations.remove(&address);
        }
        debug_assert!(
            self.durable_serviced_candidates
                .keys()
                .all(|key| self.serviced_candidates.contains_key(key))
        );
        if durable_changed
            && let Err(reason) = self
                .serviced_candidate_store
                .persist_with_producer_continuations(
                    &self.durable_serviced_candidates,
                    &self.durable_producer_continuations,
                    self.serviced_candidates_decision_reclaimed,
                )
        {
            return Err(self.fail_serviced_candidate_store(reason));
        }
        Ok(())
    }
    #[cfg(test)]
    fn serviced_candidate_count_for_test(&self) -> usize {
        self.serviced_candidates.len()
    }
    #[cfg(test)]
    pub(crate) fn producer_continuation_counts_for_test(&self) -> (usize, usize, usize) {
        (
            self.producer_continuations.len(),
            self.durable_producer_continuations.len(),
            self.deferred_producer_continuations.len(),
        )
    }
    #[cfg(test)]
    fn serviced_candidate_store_path_for_test(&self) -> &std::path::Path {
        self.serviced_candidate_store.path_for_test()
    }
    #[cfg(test)]
    fn serviced_candidate_views_for_test(&self) -> BTreeSet<wire::View> {
        self.serviced_candidates.values().copied().collect()
    }
    fn reducer_fence_projection(&self) -> ReducerFenceProjection {
        ReducerFenceProjection {
            pending_persistence: self.reducer.pending_persistence_record().cloned(),
            awaiting_signature: self.reducer.awaiting_signature().cloned(),
            replay_complete: self.replay_complete,
        }
    }
    fn advance_reducer_fence_generation(&mut self) -> Result<(), AdapterError> {
        let Some(next) = self
            .reducer_fence_generation
            .checked_add(1)
            .filter(|next| *next != u64::MAX)
        else {
            self.fail_closed = true;
            return Err(AdapterError::ReducerFenceGenerationExhausted);
        };
        self.reducer_fence_generation = next;
        Ok(())
    }
    fn step_reducer(
        &mut self,
        event: reducer::Event,
    ) -> Result<reducer::StepOutcome, AdapterError> {
        let before = self.reducer_fence_projection();
        let outcome = self.reducer.step(event)?;
        if self.reducer_fence_projection() != before {
            self.advance_reducer_fence_generation()?;
        }
        Ok(outcome)
    }
    /// Return the current process-local reducer-fence generation.
    ///
    /// The lifecycle scheduler must pair this value with a domain-separated
    /// source derived from [`Self::wire_context`] and sample both while it owns
    /// the same adapter borrow as the attempted direct completion.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn reducer_fence_generation(&self) -> u64 {
        self.reducer_fence_generation
    }
    /// Seal the current reducer-fence source and generation for lifecycle planning.
    pub(in crate::sumeragi) fn lifecycle_reducer_fence_observation(
        &self,
    ) -> LifecycleReducerFenceObservationV1 {
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(self.wire_context.id().0.as_ref());
        let context =
            LifecycleContext::new(LifecycleDigest::new(context_id), self.wire_context.height);
        LifecycleReducerFenceObservationV1 {
            source: super::v2_lifecycle_coordinator::reducer_fence_wait_source(context),
            generation: self.reducer_fence_generation,
        }
    }
    fn ensure_ingress(&self) -> Result<(), AdapterError> {
        if self.fail_closed {
            Err(AdapterError::FailClosed)
        } else if !self.replay_complete {
            Err(AdapterError::ReplayNotComplete)
        } else {
            Ok(())
        }
    }
    fn step(&mut self, event: reducer::Event) -> Result<AdapterOutcome, AdapterError> {
        self.step_with_completion_evidence(event, None)
    }
    fn step_with_completion_evidence(
        &mut self,
        event: reducer::Event,
        completion_evidence: Option<BodyPipelineCompletionEvidence>,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.step_with_completion_evidence_and_status(event, completion_evidence, true)
    }
    fn step_with_completion_evidence_and_status(
        &mut self,
        event: reducer::Event,
        completion_evidence: Option<BodyPipelineCompletionEvidence>,
        publish_status: bool,
    ) -> Result<AdapterOutcome, AdapterError> {
        let priority = match &event {
            reducer::Event::ResumeAfterReplay { .. }
            | reducer::Event::LocalProposalReady { .. }
            | reducer::Event::BodyAvailable { .. }
            | reducer::Event::BodyStored { .. }
            | reducer::Event::ValidationCompleted { .. }
            | reducer::Event::Persisted { .. }
            | reducer::Event::PersistenceFailed { .. }
            | reducer::Event::Signed { .. }
            | reducer::Event::ApplicationCompleted { .. }
            | reducer::Event::TimeoutElapsed { .. }
            | reducer::Event::RetransmitElapsed { .. } => DeferredPriority::Completion,
            reducer::Event::TimeoutVoteReceived { .. } => DeferredPriority::Progress,
            reducer::Event::ProposalReceived { .. }
            | reducer::Event::VoteReceived { .. }
            | reducer::Event::QuorumCertificateReceived { .. }
            | reducer::Event::TimeoutCertificateReceived { .. } => DeferredPriority::Normal,
        };
        self.step_with_defer_policy(
            event,
            false,
            priority,
            None,
            completion_evidence,
            None,
            publish_status,
        )
        .map(|result| result.outcome)
    }
    #[cfg(test)]
    fn step_authenticated_ingress(
        &mut self,
        event: reducer::Event,
        admission: Option<IngressAdmission>,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.step_authenticated_ingress_with_ownership(event, admission, None)
            .map(|result| result.outcome)
    }
    fn step_authenticated_ingress_with_ownership(
        &mut self,
        event: reducer::Event,
        admission: Option<IngressAdmission>,
        authenticated_wire_identity: Option<Arc<[u8]>>,
    ) -> Result<DeferPolicyOutcome, AdapterError> {
        let priority = if matches!(
            &event,
            reducer::Event::QuorumCertificateReceived { .. }
                | reducer::Event::TimeoutVoteReceived { .. }
                | reducer::Event::TimeoutCertificateReceived { .. }
        ) || admission.is_some_and(|admission| admission.locked_commit_progress)
            || admission.is_some_and(|admission| admission.locked_reproposal_prepare_progress)
        {
            DeferredPriority::Progress
        } else {
            DeferredPriority::Normal
        };
        self.step_with_defer_policy(
            event,
            true,
            priority,
            admission,
            None,
            authenticated_wire_identity,
            true,
        )
    }
    fn step_with_defer_policy(
        &mut self,
        event: reducer::Event,
        retag_authenticated_ingress: bool,
        priority: DeferredPriority,
        admission: Option<IngressAdmission>,
        completion_evidence: Option<BodyPipelineCompletionEvidence>,
        authenticated_wire_identity: Option<Arc<[u8]>>,
        publish_status: bool,
    ) -> Result<DeferPolicyOutcome, AdapterError> {
        self.ensure_ingress()?;
        let queued = event.clone();
        let serviced_candidate = self.serviced_candidate(
            &queued,
            priority,
            completion_evidence.as_ref(),
            authenticated_wire_identity.as_deref(),
        );
        if serviced_candidate.is_some_and(|(key, _, policy)| {
            policy == ServicedCandidatePolicy::Suppress
                && self.serviced_candidates.contains_key(&key)
        }) {
            if let Some(admission) = admission {
                self.record_ingress_delivery(admission);
            }
            let disposition = reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate);
            self.record_disposition(disposition);
            if publish_status {
                self.publish_status()?;
            }
            self.log_body_progress(&queued, disposition, 0);
            return Ok(DeferPolicyOutcome {
                outcome: AdapterOutcome {
                    disposition,
                    effects: Vec::new(),
                    deferred_admission_ordinal: None,
                    producer_handoff: None,
                },
            });
        }
        let producer_stage = serviced_candidate_stage(&queued);
        let locally_reconstructible_producer =
            producer_stage.is_some_and(producer_parent_is_locally_reconstructible);
        if self.selected_producer_lifecycle.is_some()
            && serviced_candidate.is_some()
            && locally_reconstructible_producer
            && !producer_parent_has_exact_local_replay_binding(
                &queued,
                completion_evidence.as_ref(),
                self.reducer.durable_state().decision().is_some(),
            )
        {
            return Err(self.fail_serviced_candidate_store(
                "selected producer kind had no exact replayable parent binding".to_owned(),
            ));
        }
        // Every selected exact producer class reserves its immutable lifecycle
        // before the reducer step. Conditional transport and volatile-body
        // parents are reopened through the durable generic ingress token;
        // only the Local class additionally requires an immediate local replay
        // binding at this boundary.
        let producer_candidate = if producer_stage.is_some() {
            serviced_candidate
        } else {
            None
        };
        let live_producer_alias = producer_candidate
            .map(|candidate| {
                self.live_deferred_producer_alias(candidate, retag_authenticated_ingress)
            })
            .transpose()?
            .unwrap_or(false);
        if live_producer_alias {
            if let Some(admission) = admission {
                self.record_ingress_delivery(admission);
            }
            let disposition = reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate);
            self.record_disposition(disposition);
            if publish_status {
                self.publish_status()?;
            }
            self.log_body_progress(&queued, disposition, 0);
            return Ok(DeferPolicyOutcome {
                outcome: AdapterOutcome {
                    disposition,
                    effects: Vec::new(),
                    deferred_admission_ordinal: None,
                    producer_handoff: None,
                },
            });
        }
        let producer_reservation =
            self.reserve_selected_producer_continuation(producer_candidate)?;
        if let Err(error) =
            self.ensure_serviced_candidate_capacity_before_step(&queued, serviced_candidate)
        {
            self.rollback_producer_reservation(producer_reservation)?;
            return Err(error);
        }
        let outcome = match self.step_reducer(event) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.rollback_producer_reservation(producer_reservation)?;
                return Err(error);
            }
        };
        let disposition = outcome.disposition();
        self.record_reducer_outcome(&queued, disposition, outcome.effects());
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            self.log_body_progress(&queued, disposition, 0);
            let deferred_admission_ordinal = match self.enqueue_deferred(
                queued,
                retag_authenticated_ingress,
                priority,
                admission,
                completion_evidence,
                authenticated_wire_identity,
            ) {
                Ok(ordinal) => ordinal,
                Err(error) => {
                    self.rollback_producer_reservation(producer_reservation)?;
                    return Err(error);
                }
            };
            match (deferred_admission_ordinal, producer_reservation) {
                (Some(ordinal), Some(reservation)) => {
                    match self.deferred_producer_continuations.get(&ordinal) {
                        Some(existing) if existing.address != reservation.address => {
                            // `enqueue_deferred` coalesced with an older exact
                            // owner. Keep that owner's address and undo only
                            // this speculative reservation.
                            self.rollback_producer_reservation(Some(reservation))?;
                        }
                        Some(_) => {}
                        None => {
                            self.deferred_producer_continuations
                                .insert(ordinal, reservation);
                        }
                    }
                }
                (None, reservation) => self.rollback_producer_reservation(reservation)?,
                (Some(_), None) => {}
            }
            if deferred_admission_ordinal.is_some()
                && let Some(admission) = admission
            {
                self.record_ingress_delivery(admission);
            }
            if publish_status {
                self.publish_status()?;
            }
            return Ok(DeferPolicyOutcome {
                outcome: AdapterOutcome {
                    disposition,
                    effects: Vec::new(),
                    deferred_admission_ordinal,
                    producer_handoff: None,
                },
            });
        }
        // Busy is the reducer's only retryable disposition. Every applied or
        // safely ignored authenticated input has crossed its consumer
        // boundary, so retain the delivery record and coalesce an exact
        // retransmission before conversion. A Commit ignored before its exact
        // lock is durable records an ordinary delivery; once that lock is
        // installed, `locked_commit_progress` changes the consumer epoch and
        // admits the same authenticated vote once under the current full event
        // tag. Later pool resets remain scoped by view and generation.
        // One adapter invocation returns exactly one reducer macro-step. Busy-
        // deferred inputs remain adapter-owned and the serialized runtime
        // schedules them explicitly after this batch reaches the executor.
        // Concatenating them here would erase the reducer transition boundary
        // and could exceed the executor's retained-batch capacity.
        let effects = match self.drive_effects(outcome.into_effects()) {
            Ok(effects) => effects,
            Err(error) => {
                self.release_unrecorded_producer(producer_reservation)?;
                return Err(error);
            }
        };
        let record_kind = serviced_candidate_record_kind(&queued, disposition);
        let serviced_candidate = record_kind.and(serviced_candidate);
        let durable_terminal_retirement =
            record_kind == Some(ServicedCandidateRecordKind::DurableTerminal);
        let durable_terminal_evidence =
            durable_terminal_retirement || completion_evidence.is_some();
        let producer_handoff = if record_kind.is_some() {
            self.record_serviced_candidate(
                serviced_candidate,
                durable_terminal_retirement,
                durable_terminal_evidence,
                producer_reservation,
            )?
        } else {
            self.release_unrecorded_producer(producer_reservation)?;
            None
        };
        if let Some(admission) = admission {
            self.record_ingress_delivery(admission);
        }
        if publish_status {
            self.publish_status()?;
        }
        self.log_body_progress(&queued, disposition, effects.len());
        Ok(DeferPolicyOutcome {
            outcome: AdapterOutcome {
                disposition,
                effects,
                deferred_admission_ordinal: None,
                producer_handoff,
            },
        })
    }
    fn record_ingress_delivery(&mut self, admission: IngressAdmission) {
        self.ingress_deliveries.insert(
            admission.key,
            IngressDeliveryRecord {
                fingerprint: admission.fingerprint,
                consumer_tag: admission.consumer_tag,
                locked_commit_progress: admission.locked_commit_progress,
                locked_reproposal_prepare_progress: admission.locked_reproposal_prepare_progress,
            },
        );
    }
    fn record_disposition(&mut self, disposition: reducer::StepDisposition) {
        if let reducer::StepDisposition::Ignored(reason) = disposition {
            let count = self.ignore_counts.entry(reason).or_default();
            *count = count.saturating_add(1);
        }
    }
    fn record_reducer_outcome(
        &mut self,
        event: &reducer::Event,
        disposition: reducer::StepDisposition,
        effects: &[reducer::Effect],
    ) {
        self.record_disposition(disposition);
        if disposition != reducer::StepDisposition::Applied
            || effects
                .iter()
                .any(|effect| matches!(effect, reducer::Effect::ReportEquivocation { .. }))
        {
            return;
        }
        let current = reducer::Round::new(
            self.reducer.context().height(),
            self.reducer.current_tag().view(),
        );
        let progress = match event {
            reducer::Event::ResumeAfterReplay { .. } => Some((
                wire::SumeragiV2ProgressTransition::RecoveryReplayed,
                current,
            )),
            reducer::Event::LocalProposalReady { tag, .. } => Some((
                if effects
                    .iter()
                    .any(|effect| matches!(effect, reducer::Effect::Apply { .. }))
                {
                    wire::SumeragiV2ProgressTransition::BodyValidated
                } else {
                    wire::SumeragiV2ProgressTransition::ProposalAdmitted
                },
                reducer::Round::new(tag.height(), tag.view()),
            )),
            reducer::Event::ProposalReceived { proposal, .. } => Some((
                wire::SumeragiV2ProgressTransition::ProposalAdmitted,
                proposal.proposal().round(),
            )),
            reducer::Event::VoteReceived { vote, .. } => Some((
                match vote.vote().phase() {
                    reducer::Phase::Prepare => {
                        wire::SumeragiV2ProgressTransition::PrepareVoteAdmitted
                    }
                    reducer::Phase::Commit => {
                        wire::SumeragiV2ProgressTransition::CommitVoteAdmitted
                    }
                },
                vote.vote().round(),
            )),
            reducer::Event::QuorumCertificateReceived { certificate, .. } => Some((
                match certificate.phase() {
                    reducer::Phase::Prepare => wire::SumeragiV2ProgressTransition::PrepareQuorum,
                    reducer::Phase::Commit => wire::SumeragiV2ProgressTransition::CommitQuorum,
                },
                certificate.round(),
            )),
            reducer::Event::TimeoutVoteReceived { vote, .. } => Some((
                wire::SumeragiV2ProgressTransition::TimeoutVoteAdmitted,
                vote.vote().round(),
            )),
            reducer::Event::BodyAvailable { round, .. } => {
                Some((wire::SumeragiV2ProgressTransition::BodyAvailable, *round))
            }
            reducer::Event::BodyStored { round, .. } => {
                Some((wire::SumeragiV2ProgressTransition::BodyStored, *round))
            }
            reducer::Event::ValidationCompleted {
                round, valid: true, ..
            } => Some((wire::SumeragiV2ProgressTransition::BodyValidated, *round)),
            reducer::Event::ApplicationCompleted { .. } => {
                Some((wire::SumeragiV2ProgressTransition::Applied, current))
            }
            reducer::Event::Persisted { .. }
                if effects
                    .iter()
                    .any(|effect| matches!(effect, reducer::Effect::EnterView { .. })) =>
            {
                Some((
                    wire::SumeragiV2ProgressTransition::TimeoutCertificateInstalled,
                    current,
                ))
            }
            reducer::Event::Persisted { .. }
                if self.reducer.durable_state().decision().is_some() =>
            {
                Some((
                    wire::SumeragiV2ProgressTransition::DecisionPersisted,
                    self.reducer
                        .durable_state()
                        .decision()
                        .expect("guarded durable decision")
                        .round(),
                ))
            }
            reducer::Event::Persisted { .. }
                if self.reducer.durable_state().locked().is_some()
                    && effects.iter().any(|effect| {
                        matches!(
                            effect,
                            reducer::Effect::Sign {
                                message: reducer::SignableMessage::Vote(vote),
                                ..
                            } if vote.phase() == reducer::Phase::Commit
                        )
                    }) =>
            {
                Some((
                    wire::SumeragiV2ProgressTransition::LockInstalled,
                    self.reducer
                        .durable_state()
                        .locked()
                        .expect("guarded durable lock")
                        .round(),
                ))
            }
            reducer::Event::Signed { .. } => effects.iter().find_map(|effect| match effect {
                reducer::Effect::Broadcast(reducer::ConsensusMessageV2::Vote(vote)) => Some((
                    match vote.vote().phase() {
                        reducer::Phase::Prepare => {
                            wire::SumeragiV2ProgressTransition::PrepareVoteAdmitted
                        }
                        reducer::Phase::Commit => {
                            wire::SumeragiV2ProgressTransition::CommitVoteAdmitted
                        }
                    },
                    vote.vote().round(),
                )),
                reducer::Effect::Broadcast(reducer::ConsensusMessageV2::TimeoutVote(vote)) => {
                    Some((
                        wire::SumeragiV2ProgressTransition::TimeoutVoteAdmitted,
                        vote.vote().round(),
                    ))
                }
                _ => None,
            }),
            reducer::Event::TimeoutCertificateReceived { .. }
            | reducer::Event::TimeoutElapsed { .. }
            | reducer::Event::RetransmitElapsed { .. }
            | reducer::Event::Persisted { .. }
            | reducer::Event::ValidationCompleted { valid: false, .. }
            | reducer::Event::PersistenceFailed { .. } => None,
        };
        if let Some((transition, round)) = progress
            && progress_transition_is_public_at_view(transition, round.view(), current.view())
        {
            self.last_progress = Some((self.reducer.generation(), round, transition));
        }
    }
    fn log_body_progress(
        &self,
        event: &reducer::Event,
        disposition: reducer::StepDisposition,
        effect_count: usize,
    ) {
        let (stage, round, subject, valid) = match event {
            reducer::Event::ProposalReceived { proposal, .. } => {
                let proposal = proposal.proposal();
                (
                    "proposal_received",
                    proposal.round(),
                    proposal.manifest().subject(),
                    None,
                )
            }
            reducer::Event::BodyAvailable { round, subject, .. } => {
                ("body_available", *round, *subject, None)
            }
            reducer::Event::BodyStored { round, subject, .. } => {
                ("body_stored", *round, *subject, None)
            }
            reducer::Event::ValidationCompleted {
                round,
                subject,
                valid,
                ..
            } => ("validation_completed", *round, *subject, Some(*valid)),
            _ => return,
        };
        let current_tag = self.current_tag();
        iroha_logger::debug!(
            stage,
            round_height = round.height(),
            round_view = round.view(),
            subject = ?subject,
            ?valid,
            ?disposition,
            effect_count,
            current_height = current_tag.height(),
            current_view = current_tag.view(),
            deferred_completions = self.deferred_completions.len(),
            deferred_progress = self.deferred_progress_inputs.len(),
            deferred_normal = self.deferred_inputs.len(),
            "processed Sumeragi v2 body-progress reducer input"
        );
    }
    fn enqueue_deferred(
        &mut self,
        event: reducer::Event,
        retag_authenticated_ingress: bool,
        priority: DeferredPriority,
        admission: Option<IngressAdmission>,
        completion_evidence: Option<BodyPipelineCompletionEvidence>,
        authenticated_wire_identity: Option<Arc<[u8]>>,
    ) -> Result<Option<u128>, AdapterError> {
        if retag_authenticated_ingress && authenticated_wire_identity.is_none() {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        let protected_progress = admission.is_some_and(|admission| {
            admission.locked_commit_progress || admission.locked_reproposal_prepare_progress
        });
        let mut input = DeferredInput {
            admission_ordinal: 0,
            admission_capability: DeferredAdmissionCapability::pending(),
            event,
            completion_evidence,
            retag_authenticated_ingress,
            priority,
            protected_progress,
            admission,
            authenticated_wire_identity,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        };
        let progress_capacity = deferred_progress_capacity(self.wire_context.roster.len());
        let duplicate_ordinal = match priority {
            DeferredPriority::Completion => self
                .deferred_completions
                .iter()
                .find(|queued| *queued == &input),
            DeferredPriority::Progress => self
                .deferred_progress_inputs
                .iter()
                .find(|queued| *queued == &input),
            DeferredPriority::Normal => {
                self.deferred_inputs.iter().find(|queued| *queued == &input)
            }
        }
        .map(|queued| queued.admission_ordinal);
        if let Some(ordinal) = duplicate_ordinal {
            return Ok(Some(ordinal));
        }
        match priority {
            DeferredPriority::Completion => {
                // Adapter completions and local timer events are trusted;
                // untrusted network traffic cannot consume this reserved lane.
                // `contains` above bounds repeated retransmit ticks for one tag,
                // while the one-shot absolute timeout is never dropped merely
                // because the normal deferred lane is full.
                if self.deferred_completions.len() >= MAX_DEFERRED_INPUTS {
                    return Err(AdapterError::DeferredCompletionCapacityExceeded);
                }
            }
            DeferredPriority::Progress => {
                // The progress lane is partitioned before admission: one slot
                // per frozen validator is reserved independently for exact
                // locked-round Commit reconstruction, current locked-body
                // Prepare reproposal, and TimeoutVote messages, plus one slot
                // for each PrepareQC, CommitQC, and TC class.
                // Exact duplicates coalesce above; a distinct item for an
                // already-owned signer/class retries after fair service rather
                // than displacing admitted progress.
                let Some(owner) = deferred_progress_owner(&input) else {
                    return Ok(None);
                };
                let class = owner.class();
                let class_capacity = match class {
                    DeferredProgressClass::LockedCommitVote
                    | DeferredProgressClass::LockedReproposalPrepareVote
                    | DeferredProgressClass::TimeoutVote => self.wire_context.roster.len(),
                    DeferredProgressClass::PrepareCertificate
                    | DeferredProgressClass::CommitCertificate
                    | DeferredProgressClass::TimeoutCertificate => 1,
                };
                if self.deferred_progress_inputs.iter().any(|queued| {
                    deferred_progress_owner(queued)
                        .is_some_and(|queued_owner| queued_owner == owner)
                }) {
                    return Ok(None);
                }
                let class_len = self
                    .deferred_progress_inputs
                    .iter()
                    .filter(|queued| deferred_progress_class(queued) == Some(class))
                    .count();
                if class_len >= class_capacity
                    || self.deferred_progress_inputs.len() >= progress_capacity
                {
                    return Ok(None);
                }
            }
            DeferredPriority::Normal => {
                if self.deferred_inputs.len() >= MAX_DEFERRED_INPUTS {
                    return Ok(None);
                }
            }
        }
        input.admission_capability =
            self.mint_deferred_admission_ordinal(retag_authenticated_ingress)?;
        input.admission_ordinal = input.admission_capability.ordinal;
        let admission_ordinal = input.admission_ordinal;
        match priority {
            DeferredPriority::Completion => self.deferred_completions.push_back(input),
            DeferredPriority::Progress => self.deferred_progress_inputs.push_back(input),
            DeferredPriority::Normal => self.deferred_inputs.push_back(input),
        }
        Ok(Some(admission_ordinal))
    }
    fn mint_deferred_admission_ordinal(
        &mut self,
        authenticated_ingress: bool,
    ) -> Result<DeferredAdmissionCapability, AdapterError> {
        let origin = if authenticated_ingress {
            DeferredAdmissionOrigin::DirectAuthenticated
        } else {
            DeferredAdmissionOrigin::LocalOrCausal
        };
        match self.deferred_admission_ordinals.mint(origin) {
            Ok(ordinal) => Ok(ordinal),
            Err(error) => {
                self.fail_closed = true;
                Err(error)
            }
        }
    }
    /// Return whether one adapter-owned Busy-deferred input can cross the
    /// reducer boundary now.
    ///
    /// The serialized runtime gives this finite debt its own scheduling turn.
    /// Pending WAL or signing work must instead be cleared by its matching
    /// completion command, so reporting deferred work as ready while either
    /// fence is active would spin and starve that completion.
    pub(crate) fn deferred_work_is_serviceable(&self) -> bool {
        !self.fail_closed
            && self.replay_complete
            && self.reducer.pending_persistence_record().is_none()
            && self.reducer.awaiting_signature().is_none()
            && (!self.deferred_completions.is_empty()
                || !self.deferred_progress_inputs.is_empty()
                || !self.deferred_inputs.is_empty())
    }
    /// Return whether replay or a pending safety-WAL acknowledgement owns the
    /// only legal next reducer transition. Pacemaker ingress must remain
    /// queued until that exact asynchronous completion is delivered.
    pub(crate) fn pacemaker_escape_is_parked(&self) -> bool {
        !self.fail_closed
            && (!self.replay_complete || self.reducer.pending_persistence_record().is_some())
    }
    /// Return whether an active signature request is the sole reducer fence.
    /// Certified TC/CommitQC ingress may bypass only this state; persistence
    /// and replay fences always precede every network transition.
    pub(crate) fn signature_fence_is_active(&self) -> bool {
        !self.fail_closed
            && self.replay_complete
            && self.reducer.pending_persistence_record().is_none()
            && self.reducer.awaiting_signature().is_some()
    }
    /// Return the exact active signer owner used to scope runtime retry
    /// exclusions. A duplicate certified message leaves this identity
    /// unchanged, while consuming the signer or installing a successor
    /// signable changes it even when both tasks share one reducer tag.
    pub(crate) fn signature_fence_identity(
        &self,
    ) -> Option<(reducer::EventTag, reducer::SignableMessage)> {
        if !self.signature_fence_is_active() {
            return None;
        }
        self.reducer
            .awaiting_signature()
            .cloned()
            .map(|message| (self.reducer.current_tag(), message))
    }
    /// Return whether one exact runtime completion opens the active signing
    /// fence which currently makes older adapter-owned debt unserviceable.
    ///
    /// Safety-WAL acknowledgement is synchronous at this boundary, so signing
    /// is the only externally completed reducer fence. The preflight clone
    /// proves the callback still applies to the current reducer incarnation;
    /// the production effect executor has already verified the signature
    /// against its exact pending [`SignRequest`] and transferred that task's
    /// lifecycle owner. The runtime separately rejects an independently
    /// minted `SignatureCompleted` root from the dependency bypass. Stale and
    /// otherwise nonmatching completions remain ordinary FIFO work. A `true`
    /// result promises that dispatch consumes this signing fence rather than
    /// returning retryable or deferred work; the runtime fails closed if that
    /// contract is violated.
    pub(crate) fn completion_unblocks_deferred_fence(
        &self,
        tag: reducer::EventTag,
        command: &super::v2_runtime::AdapterCommand,
    ) -> bool {
        use super::v2_runtime::{
            AdapterCommand, RuntimeCommandAdmissionPreflight as AdmissionPreflight,
        };
        !self.fail_closed
            && self.replay_complete
            && tag == self.reducer.current_tag()
            && self.reducer.pending_persistence_record().is_none()
            && self.reducer.awaiting_signature().is_some()
            && matches!(command, AdapterCommand::SignatureCompleted(_))
            && self.preflight_runtime_command_admission(tag, command) == AdmissionPreflight::Admit
    }
    /// Return whether this exact queued command is forced to report `Busy` by
    /// the same active signing fence opened by
    /// [`Self::completion_unblocks_deferred_fence`].
    ///
    /// This is deliberately a proof, not a broad command-class hint. Internal
    /// callbacks must retain the current tag and survive reducer preflight,
    /// while queued authenticated ingress is deliberately retagged at
    /// dispatch and must therefore be classified against the current reducer
    /// incarnation regardless of its stored tag. Authenticated input must
    /// also have a fresh semantic-admission path and convert against the
    /// current registry. A duplicate, equivocation report, unsafe proposal,
    /// capacity terminal, stale view, malformed conversion, or independent
    /// signature callback therefore remains an ordinary ordered owner.
    pub(crate) fn command_is_blocked_by_deferred_fence(
        &self,
        tag: reducer::EventTag,
        command: &super::v2_runtime::AdapterCommand,
    ) -> bool {
        use super::v2_runtime::{
            AdapterCommand, RuntimeCommandAdmissionPreflight as AdmissionPreflight,
        };
        if self.fail_closed
            || !self.replay_complete
            || self.reducer.pending_persistence_record().is_some()
            || self.reducer.awaiting_signature().is_none()
        {
            return false;
        }
        match command {
            AdapterCommand::SignatureCompleted(_) => false,
            AdapterCommand::Authenticated(authenticated) => {
                self.authenticated_command_reaches_fenced_reducer(authenticated)
            }
            AdapterCommand::LocalProposalReady { .. }
            | AdapterCommand::BodyAvailable { .. }
            | AdapterCommand::BodyStored { .. }
            | AdapterCommand::ApplicationCompleted(_) => {
                tag == self.reducer.current_tag()
                    && self.preflight_runtime_command_admission(tag, command)
                        == AdmissionPreflight::Admit
            }
        }
    }
    /// Conservatively prove that authenticated ingress reaches the reducer's
    /// active signing fence in the current adapter state.
    ///
    /// A verified TC or CommitQC is deliberately excluded: those certified
    /// transitions supersede the fenced reducer incarnation. Every other
    /// non-`Signed` event which returns `true` is unconditionally `Busy`
    /// before its phase handler can run.
    fn authenticated_command_reaches_fenced_reducer(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> bool {
        let message = &authenticated.0;
        if message.validate_version().is_err() {
            return false;
        }
        let current_view = self.reducer.current_tag().view();
        let retained_vote_views = u64::try_from(self.wire_context.roster.len()).unwrap_or(u64::MAX);
        let oldest_retained_view = current_view.saturating_sub(retained_vote_views);
        let payload = &message.payload;
        let locked_commit_progress = match payload {
            wire::ConsensusMessageV2Payload::Vote(vote) => self.is_exact_locked_commit_vote(vote),
            _ => false,
        };
        let locked_reproposal_prepare_progress = match payload {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                self.is_exact_locked_reproposal_prepare_vote(vote)
            }
            _ => false,
        };
        let unsafe_proposal = if let wire::ConsensusMessageV2Payload::Proposal(proposal) = payload
            && let Some(locked) = self.reducer.durable_state().locked()
        {
            let Ok(locked_subject) = self.registry.subject(locked.subject()) else {
                return false;
            };
            !proposal_is_safe_for_lock(
                proposal,
                self.registry.round_to_wire(locked.round()),
                locked_subject,
            )
        } else {
            false
        };
        if unsafe_proposal {
            return false;
        }
        let semantic_key = match payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                if proposal.round.view != current_view {
                    return false;
                }
                Some(IngressSemanticKey::Proposal {
                    round: proposal.round,
                    proposer: proposal.proposer,
                })
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                if vote.round.view > current_view
                    || (vote.round.view < oldest_retained_view && !locked_commit_progress)
                {
                    return false;
                }
                Some(IngressSemanticKey::Vote {
                    round: vote.round,
                    phase: vote.phase,
                    signer: vote.signer,
                })
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                if !reducer::timeout_vote_view_is_admissible(current_view, vote.round.view) {
                    return false;
                }
                Some(IngressSemanticKey::TimeoutVote {
                    round: vote.round,
                    signer: vote.signer,
                })
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_) => None,
            wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => return false,
        };
        if let Some(key) = semantic_key {
            // Any existing semantic record can terminate as a duplicate or an
            // equivocation report before reaching the reducer. Be conservative
            // even when normal pruning would make a stale record removable.
            if self.ingress_equivocations.contains_key(&key) {
                return false;
            }
            let capacity_bypass = self.ingress_equivocations.len() >= MAX_INGRESS_SEMANTIC_KEYS;
            let protected_capacity_bypass = locked_commit_progress
                || locked_reproposal_prepare_progress
                || matches!(key, IngressSemanticKey::TimeoutVote { .. });
            if capacity_bypass && !protected_capacity_bypass {
                return false;
            }
        }
        // Authentication has already verified the envelope signature. Repeat
        // the registry conversion on a clone so conflicting identities or
        // commitments cannot be mislabeled as reducer-fenced work.
        let mut registry = self.registry.clone();
        match payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => registry
                .proposal_to_core(proposal, &self.wire_context)
                .is_ok(),
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                registry.vote_to_core(vote, &self.wire_context).is_ok()
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => registry
                .qc_to_core(certificate, &self.wire_context)
                .is_ok_and(|certificate| {
                    !reducer::Reducer::certified_progress_bypasses_signature_fence(
                        &reducer::Event::QuorumCertificateReceived {
                            tag: self.reducer.current_tag(),
                            certificate,
                        },
                    )
                }),
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => registry
                .timeout_vote_to_core(vote, &self.wire_context)
                .is_ok(),
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => registry
                .tc_to_core(certificate, &self.wire_context)
                .is_ok_and(|certificate| {
                    !reducer::Reducer::certified_progress_bypasses_signature_fence(
                        &reducer::Event::TimeoutCertificateReceived {
                            tag: self.reducer.current_tag(),
                            certificate,
                        },
                    )
                }),
            wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => false,
        }
    }
    /// Service at most one adapter-owned Busy-deferred reducer transition.
    ///
    /// Returning one macro-step preserves the executor's fixed retained-batch
    /// bound. Repeated serialized runtime turns decrease the finite deferred
    /// rank, while `pop_deferred_next` keeps the three classes round-robin.
    #[cfg(test)]
    pub(crate) fn drain_deferred(&mut self) -> Result<Vec<AdapterEffect>, AdapterError> {
        self.drain_deferred_with_evidence()
            .map(|selection| selection.map_or_else(Vec::new, |(effects, _)| effects))
    }
    /// Service one deferred transition and return its exact process-local
    /// ownership token with the resulting effects.
    ///
    /// `None` means no owner was serviceable. Production runtime code treats a
    /// `None` after observing [`Self::deferred_work_is_serviceable`] as a
    /// fail-closed source-fidelity violation.
    #[cfg(test)]
    pub(crate) fn drain_deferred_with_evidence(
        &mut self,
    ) -> Result<Option<(Vec<AdapterEffect>, DeferredServiceEvidence)>, AdapterError> {
        let eligible = self.all_deferred_admission_ordinals();
        self.drain_deferred_with_evidence_for_ordinals(&eligible)
    }
    /// Service one deferred transition from the exact target-relative set
    /// selected by the serialized runtime.
    ///
    /// The runtime first excludes post-cut physical replays, then applies
    /// logical minima within each retained predecessor set. The adapter owns
    /// class rotation only within the resulting exact set, so a later
    /// Completion, Progress, or Normal occurrence cannot overtake a frozen
    /// causal owner merely because it occupies the cursor's next class.
    #[cfg(test)]
    pub(crate) fn drain_deferred_with_evidence_for_ordinals(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<Option<(Vec<AdapterEffect>, DeferredServiceEvidence)>, AdapterError> {
        let Some((effects, evidence, producer_handoff)) =
            self.drain_deferred_with_handoff_for_ordinals(eligible)?
        else {
            return Ok(None);
        };
        if let Some(token) = producer_handoff {
            let handoff_evidence = self.producer_handoff_evidence(token, !effects.is_empty())?;
            self.acknowledge_producer_handoff(token, handoff_evidence)?;
        }
        Ok(Some((effects, evidence)))
    }
    /// Production deferred-service seam retaining an exact producer token
    /// until the serialized runtime installs the returned successor owner.
    pub(crate) fn drain_deferred_with_handoff_for_ordinals(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<
        Option<(
            Vec<AdapterEffect>,
            DeferredServiceEvidence,
            Option<ProducerContinuationHandoffToken>,
        )>,
        AdapterError,
    > {
        self.ensure_ingress()?;
        if !self.deferred_work_is_serviceable() {
            return Ok(None);
        }
        let active = self.all_deferred_admission_ordinals();
        if eligible.is_empty() || !eligible.is_subset(&active) {
            self.fail_closed = true;
            return Err(AdapterError::DeferredServiceOwnershipViolation);
        }
        let Some(selection) = self.pop_deferred_next_eligible(eligible)? else {
            return Ok(None);
        };
        if !selection.evidence.validate_exact()
            || !selection
                .evidence
                .matches_effective_event(&selection.input.event)
            || !selection
                .evidence
                .belongs_to(&self.deferred_admission_ordinals)
            || !selection
                .evidence
                .matches_eligible_admission_ordinals(eligible)
            || !self.deferred_authenticated_event_matches_wire(&selection.evidence)
            || !selection.evidence.claim_adapter_service_once()
        {
            self.fail_closed = true;
            return Err(AdapterError::DeferredServiceOwnershipViolation);
        }
        let deferred_ordinal = selection.evidence.admission_ordinal;
        let producer_continuation = self
            .deferred_producer_continuations
            .get(&deferred_ordinal)
            .cloned();
        let input = selection.input;
        let serviced_candidate = self.serviced_candidate(
            &input.event,
            input.priority,
            input.completion_evidence.as_ref(),
            input.authenticated_wire_identity.as_deref(),
        );
        if serviced_candidate.is_some_and(|(key, _, policy)| {
            policy == ServicedCandidatePolicy::Suppress
                && self.serviced_candidates.contains_key(&key)
        }) {
            if let Some(admission) = input.admission {
                self.record_ingress_delivery(admission);
            }
            let disposition = reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate);
            self.record_disposition(disposition);
            self.deferred_producer_continuations
                .remove(&deferred_ordinal);
            self.release_unrecorded_producer(producer_continuation)?;
            self.publish_status()?;
            self.log_body_progress(&input.event, disposition, 0);
            return Ok(Some((Vec::new(), selection.evidence, None)));
        }
        if let Err(error) =
            self.ensure_serviced_candidate_capacity_before_step(&input.event, serviced_candidate)
        {
            self.retain_failed_serviced_deferred_owner(input);
            return Err(error);
        }
        let event = input.event.clone();
        let observed_event = event.clone();
        let outcome = self.step_reducer(event)?;
        let disposition = outcome.disposition();
        self.record_reducer_outcome(&observed_event, disposition, outcome.effects());
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            // `Busy` has exactly the two fences excluded by
            // `deferred_work_is_serviceable`. A future reducer change must
            // extend that predicate deliberately; silently requeueing would
            // return `Advanced([])` forever without decreasing queue rank.
            return Err(self.fail_deferred_service_contract());
        }
        let effects = self.drive_effects(outcome.into_effects())?;
        let record_kind = serviced_candidate_record_kind(&observed_event, disposition);
        let serviced_candidate = record_kind.and(serviced_candidate);
        let durable_terminal_retirement =
            record_kind == Some(ServicedCandidateRecordKind::DurableTerminal);
        let durable_terminal_evidence =
            durable_terminal_retirement || input.completion_evidence.is_some();
        let producer_handoff = if record_kind.is_some() {
            self.record_serviced_candidate(
                serviced_candidate,
                durable_terminal_retirement,
                durable_terminal_evidence,
                producer_continuation,
            )
        } else {
            self.release_unrecorded_producer(producer_continuation)
                .map(|()| None)
        };
        let producer_handoff = match producer_handoff {
            Ok(token) => token,
            Err(error) => {
                self.retain_failed_serviced_deferred_owner(input);
                return Err(error);
            }
        };
        self.deferred_producer_continuations
            .remove(&deferred_ordinal);
        if let Some(admission) = input.admission {
            self.record_ingress_delivery(admission);
        }
        self.publish_status()?;
        self.log_body_progress(&observed_event, disposition, effects.len());
        Ok(Some((effects, selection.evidence, producer_handoff)))
    }
    fn retain_failed_serviced_deferred_owner(&mut self, input: DeferredInput) {
        match input.priority {
            DeferredPriority::Completion => self.deferred_completions.push_front(input),
            DeferredPriority::Progress => self.deferred_progress_inputs.push_front(input),
            DeferredPriority::Normal => self.deferred_inputs.push_front(input),
        }
    }
    fn deferred_authenticated_event_matches_wire(
        &self,
        evidence: &DeferredServiceEvidence,
    ) -> bool {
        let Some(identity) = evidence.authenticated_wire_identity.as_deref() else {
            return !evidence.is_authenticated_ingress();
        };
        let message = match &evidence.original_event {
            reducer::Event::ProposalReceived { proposal, .. } => {
                reducer::ConsensusMessageV2::Proposal(proposal.clone())
            }
            reducer::Event::VoteReceived { vote, .. } => {
                reducer::ConsensusMessageV2::Vote(vote.clone())
            }
            reducer::Event::QuorumCertificateReceived { certificate, .. } => {
                reducer::ConsensusMessageV2::QuorumCertificate(certificate.clone())
            }
            reducer::Event::TimeoutVoteReceived { vote, .. } => {
                reducer::ConsensusMessageV2::TimeoutVote(vote.clone())
            }
            reducer::Event::TimeoutCertificateReceived { certificate, .. } => {
                reducer::ConsensusMessageV2::TimeoutCertificate(certificate.clone())
            }
            _ => return false,
        };
        let mut registry = self.registry.clone();
        registry
            .message_to_wire(message, self.aggregator.as_ref())
            .is_ok_and(|message| message.encode().as_slice() == identity)
    }
    /// Fail closed when the deferred-service predicate and reducer Busy
    /// contract disagree.
    fn fail_deferred_service_contract(&mut self) -> AdapterError {
        self.fail_closed = true;
        AdapterError::DeferredServiceContractViolation
    }
    /// Snapshot every physically retained deferred owner by queue class.
    fn deferred_queue_lengths(&self) -> DeferredQueueLengths {
        DeferredQueueLengths {
            completion: u64::try_from(self.deferred_completions.len())
                .expect("bounded completion queue length fits u64"),
            progress: u64::try_from(self.deferred_progress_inputs.len())
                .expect("bounded progress queue length fits u64"),
            normal: u64::try_from(self.deferred_inputs.len())
                .expect("bounded normal queue length fits u64"),
        }
    }
    /// Snapshot only the lifecycle-minimal candidates the runtime authorized
    /// for this service turn.
    ///
    /// The runtime may deliberately exclude an older queue class whose
    /// physical lifecycle is not yet serviceable. This projection controls
    /// class rotation alongside the full physical queue snapshot, so an
    /// excluded owner cannot make a valid filtered selection prove the wrong
    /// class.
    fn eligible_deferred_queue_lengths(&self, eligible: &BTreeSet<u128>) -> DeferredQueueLengths {
        let count = |queue: &VecDeque<DeferredInput>| {
            u64::try_from(
                queue
                    .iter()
                    .filter(|input| eligible.contains(&input.admission_ordinal))
                    .count(),
            )
            .expect("bounded eligible deferred queue length fits u64")
        };
        DeferredQueueLengths {
            completion: count(&self.deferred_completions),
            progress: count(&self.deferred_progress_inputs),
            normal: count(&self.deferred_inputs),
        }
    }
    #[cfg(test)]
    fn pop_deferred_next(&mut self) -> Result<Option<DeferredServiceSelection>, AdapterError> {
        let eligible = self.all_deferred_admission_ordinals();
        self.pop_deferred_next_eligible(&eligible)
    }
    fn pop_deferred_next_eligible(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<Option<DeferredServiceSelection>, AdapterError> {
        let queue_lengths_before = self.deferred_queue_lengths();
        let eligible_queue_lengths_before = self.eligible_deferred_queue_lengths(eligible);
        let service_cursor_before = self.next_deferred_priority;
        for _ in 0..3 {
            let priority = self.next_deferred_priority;
            self.next_deferred_priority = self.next_deferred_priority.next();
            let selected = match priority {
                DeferredPriority::Completion => self
                    .deferred_completions
                    .iter()
                    .position(|input| eligible.contains(&input.admission_ordinal))
                    .and_then(|position| {
                        self.deferred_completions
                            .remove(position)
                            .map(|input| (position, input))
                    }),
                DeferredPriority::Progress => self
                    .deferred_progress_inputs
                    .iter()
                    .position(|input| eligible.contains(&input.admission_ordinal))
                    .and_then(|position| {
                        self.deferred_progress_inputs
                            .remove(position)
                            .map(|input| (position, input))
                    }),
                DeferredPriority::Normal => self
                    .deferred_inputs
                    .iter()
                    .position(|input| eligible.contains(&input.admission_ordinal))
                    .and_then(|position| {
                        self.deferred_inputs
                            .remove(position)
                            .map(|input| (position, input))
                    }),
            };
            let Some((selected_position, selected)) = selected else {
                continue;
            };
            for skipped_priority in [
                DeferredPriority::Completion,
                DeferredPriority::Progress,
                DeferredPriority::Normal,
            ] {
                if skipped_priority == priority {
                    continue;
                }
                let oldest = match skipped_priority {
                    DeferredPriority::Completion => self
                        .deferred_completions
                        .iter_mut()
                        .find(|input| eligible.contains(&input.admission_ordinal)),
                    DeferredPriority::Progress => self
                        .deferred_progress_inputs
                        .iter_mut()
                        .find(|input| eligible.contains(&input.admission_ordinal)),
                    DeferredPriority::Normal => self
                        .deferred_inputs
                        .iter_mut()
                        .find(|input| eligible.contains(&input.admission_ordinal)),
                };
                let Some(oldest) = oldest else {
                    continue;
                };
                let Some(next_debt) = oldest.eligible_skips.checked_add(1) else {
                    self.fail_closed = true;
                    return Err(AdapterError::DeferredServiceDebtOverflow);
                };
                oldest.eligible_skips = next_debt;
            }
            let mut input = selected;
            let original_event = input.event.clone();
            let original_admission = input.admission;
            let original_tag = deferred_event_tag(&original_event);
            let retag = if input.retag_authenticated_ingress {
                let current_tag = self.reducer.current_tag();
                input.event = input.event.retag_authenticated_ingress(current_tag);
                if let Some(admission) = &mut input.admission {
                    admission.consumer_tag = current_tag;
                }
                DeferredRetagRelation::AuthenticatedIngress {
                    from: original_tag,
                    to: current_tag,
                }
            } else {
                DeferredRetagRelation::Unchanged
            };
            let queue_lengths_after = self.deferred_queue_lengths();
            let mut evidence = DeferredServiceEvidence {
                admission_ordinal: input.admission_ordinal,
                priority,
                event_kind: deferred_event_kind(&original_event),
                original_tag,
                effective_tag: deferred_event_tag(&input.event),
                retag,
                protected_progress: input.protected_progress,
                eligible_skips_before: input.eligible_skips,
                eligible_skips_after: 0,
                queue_lengths_before,
                eligible_queue_lengths_before,
                queue_lengths_after,
                total_len_before: queue_lengths_before.total(),
                total_len_after: queue_lengths_after.total(),
                service_cursor_before,
                service_cursor_after: self.next_deferred_priority,
                projection_hash: Hash::new([]),
                original_event,
                effective_event: input.event.clone(),
                completion_evidence: input.completion_evidence.clone(),
                original_admission,
                effective_admission: input.admission,
                authenticated_wire_identity: input.authenticated_wire_identity.clone(),
                admission_capability: input.admission_capability.clone(),
                selection_seal: None,
            };
            evidence.projection_hash = deferred_service_projection_hash(&evidence);
            evidence.selection_seal = DeferredQueueSelectionSeal::mint(
                &self.deferred_admission_ordinals,
                eligible,
                queue_lengths_before,
                eligible_queue_lengths_before,
                queue_lengths_after,
                service_cursor_before,
                self.next_deferred_priority,
                priority,
                u64::try_from(selected_position).expect("bounded deferred queue position fits u64"),
                input.admission_ordinal,
                input.eligible_skips,
                evidence.projection_hash,
            );
            if evidence.selection_seal.is_none() {
                self.fail_closed = true;
                return Err(AdapterError::DeferredServiceOwnershipViolation);
            }
            return Ok(Some(DeferredServiceSelection { input, evidence }));
        }
        Ok(None)
    }
    fn publish_status(&mut self) -> Result<(), AdapterError> {
        #[cfg(test)]
        {
            self.status_publication_attempts = self.status_publication_attempts.saturating_add(1);
        }
        let status = self.status()?;
        if self.status_publication_enabled {
            super::status::set_v2_status(status);
        }
        Ok(())
    }
    /// Permanently close the adapter after an internal macro-step shape
    /// violates the reviewed reducer/continuation contract.
    fn fail_macro_step(&mut self, error: AdapterError) -> AdapterError {
        debug_assert!(matches!(
            &error,
            AdapterError::AdapterMacroStepBoundExceeded { .. }
        ));
        self.fail_closed = true;
        error
    }
    fn drive_effects(
        &mut self,
        effects: Vec<reducer::Effect>,
    ) -> Result<Vec<AdapterEffect>, AdapterError> {
        if self.pending_live_proposal_intent_sign.is_some() {
            self.fail_closed = true;
            return Err(AdapterError::LiveWalReplayCauseMismatch);
        }
        let initial_effects = effects.len();
        let mut persist_effects = 0usize;
        let mut persistence_class = None;
        for effect in &effects {
            if let reducer::Effect::Persist { entry, .. } = effect {
                persist_effects = persist_effects.saturating_add(1);
                persistence_class = Some(PersistenceMacroStepClass::from_record(entry.record()));
            }
        }
        let persistence_budget = persistence_class.map(PersistenceMacroStepClass::budget);
        let maximum_initial_effects = persistence_budget
            .map_or(MAX_ADAPTER_EFFECTS_PER_MACRO_STEP, |budget| {
                budget.initial_effects
            });
        let maximum_continuation_effects =
            persistence_budget.map_or(0, |budget| budget.continuation_effects);
        let maximum_flattened_effects = persistence_budget.map_or(
            MAX_ADAPTER_EFFECTS_PER_MACRO_STEP,
            PersistenceMacroStepBudget::flattened_effects,
        );
        if persist_effects > 1 || initial_effects > maximum_initial_effects {
            let error = AdapterError::AdapterMacroStepBoundExceeded {
                initial_effects,
                maximum_initial_effects,
                persist_effects,
                continuation_effects: 0,
                maximum_continuation_effects,
                maximum_flattened_effects,
                continuation_contains_persist: false,
            };
            return Err(self.fail_macro_step(error));
        }
        let mut pending = VecDeque::from(effects);
        let mut ready = Vec::new();
        let mut observed_continuation_effects = 0usize;
        let mut continuation_contains_persist = false;
        while let Some(effect) = pending.pop_front() {
            match effect {
                reducer::Effect::Persist { tag, entry } => {
                    let id = entry.id();
                    self.pending_persistence_id = Some(id.get());
                    if self.replay_complete {
                        if let Err(error) = self.publish_status() {
                            self.fail_closed = true;
                            return Err(error);
                        }
                    }
                    let payload = match self
                        .registry
                        .encode_wal_entry(&entry, self.aggregator.as_ref())
                    {
                        Ok(payload) => payload,
                        Err(error) => {
                            self.fail_closed = true;
                            return Err(error);
                        }
                    };
                    let receipt = match self.wal.append(&payload) {
                        Ok(receipt) => receipt,
                        Err(error) => {
                            self.fail_closed = true;
                            let _ = self
                                .reducer
                                .step(reducer::Event::PersistenceFailed { tag, id });
                            return Err(error.into());
                        }
                    };
                    let retained_frame_is_exact =
                        self.wal.recovered_records().last().is_some_and(|record| {
                            record.exactly_matches_receipt(receipt)
                                && record.payload() == payload.as_slice()
                        });
                    if receipt.sequence().checked_add(1) != Some(id.get())
                        || !retained_frame_is_exact
                    {
                        self.fail_closed = true;
                        return Err(AdapterError::WalFrameIdentityMismatch {
                            frame_sequence: receipt.sequence(),
                            persistence_id: id.get(),
                            frame_hash: receipt.frame_hash(),
                        });
                    }
                    if let reducer::WalRecord::ProposalIntent(proposal) = entry.record() {
                        let post_wal: Result<LiveProposalIntentWalSignHandoffV1, AdapterError> =
                            (|| {
                                let frame = self
                                    .wal
                                    .recovered_records()
                                    .last()
                                    .ok_or(AdapterError::LiveWalReplayCauseMismatch)?;
                                let wal_identity = LiveWalFrameIdentity::from_append_receipt(
                                    frame,
                                    receipt,
                                    id.get(),
                                )
                                .ok_or(AdapterError::LiveWalReplayCauseMismatch)?;
                                let effect = AdapterEffect::Sign {
                                    tag,
                                    request: SignRequest::Proposal(
                                        self.registry.unsigned_proposal_to_wire(
                                            proposal,
                                            self.aggregator.as_ref(),
                                        )?,
                                    ),
                                };
                                let pending =
                                    PendingRuntimeEffectBinding::from_exact_live_wal_append(
                                        &wal_identity,
                                        &effect,
                                    )
                                    .ok_or(AdapterError::LiveWalReplayCauseMismatch)?;
                                let persisted =
                                    SealedLiveWalPersistedEffectV1::from_exact_live_append(
                                        ExactLiveWalPersistedContinuationCause::PayloadFree {
                                            wal_identity,
                                            effect: effect.clone(),
                                            pending,
                                        },
                                    )
                                    .ok_or(AdapterError::LiveWalReplayCauseMismatch)?;
                                LiveProposalIntentWalSignHandoffV1::from_exact(effect, persisted)
                                    .ok_or(AdapterError::LiveWalReplayCauseMismatch)
                            })();
                        match post_wal {
                            Ok(handoff) => {
                                if self.pending_live_proposal_intent_sign.is_some() {
                                    self.fail_closed = true;
                                    return Err(AdapterError::LiveWalReplayCauseMismatch);
                                }
                                self.pending_live_proposal_intent_sign = Some(Box::new(handoff));
                            }
                            Err(error) => {
                                self.fail_closed = true;
                                return Err(error);
                            }
                        }
                    }
                    let pending_decision_apply =
                        if let reducer::WalRecord::Decision(certificate) = entry.record() {
                            if self.pending_live_decision_apply.is_some() {
                                self.fail_closed = true;
                                return Err(AdapterError::LiveWalReplayCauseMismatch);
                            }
                            let Some(frame) = self.wal.recovered_records().last() else {
                                self.fail_closed = true;
                                return Err(AdapterError::LiveWalReplayCauseMismatch);
                            };
                            let Some(wal_identity) =
                                LiveWalFrameIdentity::from_append_receipt(frame, receipt, id.get())
                            else {
                                self.fail_closed = true;
                                return Err(AdapterError::LiveWalReplayCauseMismatch);
                            };
                            let apply = AdapterEffect::Apply {
                                tag,
                                subject: self.registry.subject(certificate.subject())?,
                                certificate: self
                                    .registry
                                    .qc_to_wire(certificate, self.aggregator.as_ref())?,
                            };
                            Some((wal_identity, apply, certificate.clone()))
                        } else {
                            None
                        };
                    self.pending_persistence_id = None;
                    let persisted = reducer::Event::Persisted { tag, id };
                    let continuation = match self.step_reducer(persisted.clone()) {
                        Ok(continuation) => continuation,
                        Err(error) => {
                            // The physical WAL is now ahead of memory. Only a
                            // clean reopen/replay may reconcile that state.
                            self.fail_closed = true;
                            return Err(error);
                        }
                    };
                    self.prune_ingress_records();
                    self.reclaim_serviced_candidates()?;
                    self.record_reducer_outcome(
                        &persisted,
                        continuation.disposition(),
                        continuation.effects(),
                    );
                    let continuation = continuation.into_effects();
                    observed_continuation_effects = continuation.len();
                    continuation_contains_persist = continuation
                        .iter()
                        .any(|effect| matches!(effect, reducer::Effect::Persist { .. }));
                    let flattened_effects = initial_effects
                        .saturating_sub(1)
                        .saturating_add(observed_continuation_effects);
                    if observed_continuation_effects > maximum_continuation_effects
                        || continuation_contains_persist
                        || flattened_effects > maximum_flattened_effects
                        || flattened_effects > MAX_ADAPTER_EFFECTS_PER_MACRO_STEP
                    {
                        let error = AdapterError::AdapterMacroStepBoundExceeded {
                            initial_effects,
                            maximum_initial_effects,
                            persist_effects,
                            continuation_effects: observed_continuation_effects,
                            maximum_continuation_effects,
                            maximum_flattened_effects,
                            continuation_contains_persist,
                        };
                        return Err(self.fail_macro_step(error));
                    }
                    if let Some((wal_identity, apply, decision)) = pending_decision_apply {
                        let mut direct_apply_count = 0usize;
                        let mut direct_apply_is_exact = true;
                        for effect in &continuation {
                            if let reducer::Effect::Apply {
                                tag: apply_tag,
                                subject,
                                certificate,
                            } = effect
                            {
                                direct_apply_count = direct_apply_count.saturating_add(1);
                                direct_apply_is_exact &= *apply_tag == tag
                                    && *subject == decision.subject()
                                    && certificate == &decision;
                            }
                        }
                        if direct_apply_count > 1
                            || (direct_apply_count == 1 && !direct_apply_is_exact)
                        {
                            self.fail_closed = true;
                            return Err(AdapterError::LiveWalReplayCauseMismatch);
                        }
                        if direct_apply_count == 0 {
                            // The body is not yet validated, so its source-only
                            // Decision-WAL seal must wait for the exact Ready
                            // Validate completion to bind the durable frame and
                            // predecessor-derived Apply owner. When this same
                            // persisted continuation emits the exact Apply,
                            // its authenticated Decision owner is already the
                            // complete direct handoff and no deferred seal may
                            // remain stranded in the adapter.
                            let Some(sealed) =
                                SealedLiveWalPersistedEffectV1::from_exact_live_append(
                                    ExactLiveWalPersistedContinuationCause::Apply {
                                        wal_identity,
                                        effect: apply,
                                    },
                                )
                            else {
                                self.fail_closed = true;
                                return Err(AdapterError::LiveWalReplayCauseMismatch);
                            };
                            self.pending_live_decision_apply = Some(sealed);
                        }
                    }
                    reducer::prepend_causal_continuation(&mut pending, continuation);
                }
                effect => match self.convert_effect(effect) {
                    Ok(effect) => ready.push(effect),
                    Err(error) => {
                        self.fail_closed = true;
                        return Err(error);
                    }
                },
            }
        }
        if ready.len() > MAX_ADAPTER_EFFECTS_PER_MACRO_STEP {
            let error = AdapterError::AdapterMacroStepBoundExceeded {
                initial_effects,
                maximum_initial_effects,
                persist_effects,
                continuation_effects: observed_continuation_effects,
                maximum_continuation_effects,
                maximum_flattened_effects,
                continuation_contains_persist,
            };
            return Err(self.fail_macro_step(error));
        }
        Ok(ready)
    }
    fn convert_effect(&mut self, effect: reducer::Effect) -> Result<AdapterEffect, AdapterError> {
        match effect {
            reducer::Effect::Persist { .. } => {
                unreachable!("persistence effects are consumed by drive_effects")
            }
            reducer::Effect::FetchBody {
                tag,
                round,
                subject,
                manifest,
                certified_sources,
                certificate,
            } => Ok(AdapterEffect::FetchBody {
                tag,
                round: self.registry.round_to_wire(round),
                subject: self.registry.subject(subject)?,
                manifest: manifest
                    .map(|manifest| self.registry.manifest_to_wire(round, &manifest))
                    .transpose()?,
                certified_sources: certified_sources
                    .into_iter()
                    .map(|validator| self.registry.peer(validator))
                    .collect::<Result<_, _>>()?,
                certificate: certificate
                    .map(|certificate| {
                        self.registry
                            .qc_to_wire(&certificate, self.aggregator.as_ref())
                    })
                    .transpose()?,
            }),
            reducer::Effect::StoreBody {
                tag,
                round,
                subject,
            } => Ok(AdapterEffect::StoreBody {
                tag,
                round: self.registry.round_to_wire(round),
                subject: self.registry.subject(subject)?,
            }),
            reducer::Effect::ValidateBody {
                tag,
                round,
                subject,
            } => Ok(AdapterEffect::ValidateBody {
                tag,
                round: self.registry.round_to_wire(round),
                subject: self.registry.subject(subject)?,
            }),
            reducer::Effect::Sign { tag, message } => {
                let request = match message {
                    reducer::SignableMessage::Proposal(proposal) => SignRequest::Proposal(
                        self.registry
                            .unsigned_proposal_to_wire(&proposal, self.aggregator.as_ref())?,
                    ),
                    reducer::SignableMessage::Vote(vote) => {
                        SignRequest::Vote(self.registry.unsigned_vote_to_wire(vote)?)
                    }
                    reducer::SignableMessage::TimeoutVote(vote) => SignRequest::TimeoutVote(
                        self.registry
                            .unsigned_timeout_vote_to_wire(&vote, self.aggregator.as_ref())?,
                    ),
                };
                Ok(AdapterEffect::Sign { tag, request })
            }
            reducer::Effect::Broadcast(message) => Ok(AdapterEffect::Broadcast(
                self.registry
                    .message_to_wire(message, self.aggregator.as_ref())?,
            )),
            reducer::Effect::Apply {
                tag,
                subject,
                certificate,
            } => Ok(AdapterEffect::Apply {
                tag,
                subject: self.registry.subject(subject)?,
                certificate: self
                    .registry
                    .qc_to_wire(&certificate, self.aggregator.as_ref())?,
            }),
            reducer::Effect::EnterView {
                tag,
                certificate,
                protected_lock,
            } => {
                // Consume the lock selected by the reducer transition itself.
                // Preserve the full authenticated QC so downstream identity
                // and lifecycle classification cannot lose its proposal round
                // or execution commitment.
                let wire_protected_lock = protected_lock
                    .as_ref()
                    .map(|locked| self.registry.qc_to_wire(locked, self.aggregator.as_ref()))
                    .transpose()?;
                self.active_subject = protected_lock
                    .as_ref()
                    .map(|locked| (locked.round(), locked.subject()));
                Ok(AdapterEffect::EnterView {
                    tag,
                    certificate: self
                        .registry
                        .tc_to_wire(&certificate, self.aggregator.as_ref())?,
                    protected_lock: wire_protected_lock,
                })
            }
            reducer::Effect::ReportEquivocation { evidence } => {
                let evidence = match evidence {
                    reducer::EquivocationEvidence::Proposal { first, second } => {
                        let first = self
                            .registry
                            .signed_proposal_to_wire(&first, self.aggregator.as_ref())?;
                        let second = self
                            .registry
                            .signed_proposal_to_wire(&second, self.aggregator.as_ref())?;
                        AdapterEquivocationEvidence::proposal(first, second)
                    }
                    reducer::EquivocationEvidence::Vote { first, second } => {
                        let first = self.registry.signed_vote_to_wire(&first)?;
                        let second = self.registry.signed_vote_to_wire(&second)?;
                        AdapterEquivocationEvidence::vote(first, second)
                    }
                    reducer::EquivocationEvidence::Timeout { first, second } => {
                        let first = self
                            .registry
                            .signed_timeout_vote_to_wire(&first, self.aggregator.as_ref())?;
                        let second = self
                            .registry
                            .signed_timeout_vote_to_wire(&second, self.aggregator.as_ref())?;
                        AdapterEquivocationEvidence::timeout_vote(first, second)
                    }
                };
                Ok(AdapterEffect::ReportEquivocation { evidence })
            }
            reducer::Effect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => Ok(AdapterEffect::ReportInvalidCertifiedBody {
                subject: self.registry.subject(subject)?,
                certificate: self
                    .registry
                    .qc_to_wire(&certificate, self.aggregator.as_ref())?,
            }),
        }
    }
}
const ALL_IGNORE_REASONS: [(reducer::IgnoreReason, wire::SumeragiV2IgnoreReason); 12] = [
    (
        reducer::IgnoreReason::WrongHeight,
        wire::SumeragiV2IgnoreReason::WrongHeight,
    ),
    (
        reducer::IgnoreReason::WrongView,
        wire::SumeragiV2IgnoreReason::WrongView,
    ),
    (
        reducer::IgnoreReason::StaleGeneration,
        wire::SumeragiV2IgnoreReason::StaleGeneration,
    ),
    (
        reducer::IgnoreReason::Busy,
        wire::SumeragiV2IgnoreReason::Busy,
    ),
    (
        reducer::IgnoreReason::Duplicate,
        wire::SumeragiV2IgnoreReason::Duplicate,
    ),
    (
        reducer::IgnoreReason::NoMatchingWork,
        wire::SumeragiV2IgnoreReason::NoMatchingWork,
    ),
    (
        reducer::IgnoreReason::Observer,
        wire::SumeragiV2IgnoreReason::Observer,
    ),
    (
        reducer::IgnoreReason::ViewClosed,
        wire::SumeragiV2IgnoreReason::ViewClosed,
    ),
    (
        reducer::IgnoreReason::AlreadyDecided,
        wire::SumeragiV2IgnoreReason::AlreadyDecided,
    ),
    (
        reducer::IgnoreReason::RecoveryPending,
        wire::SumeragiV2IgnoreReason::RecoveryPending,
    ),
    (
        reducer::IgnoreReason::IrrelevantView,
        wire::SumeragiV2IgnoreReason::IrrelevantView,
    ),
    (
        reducer::IgnoreReason::UnsafeProposal,
        wire::SumeragiV2IgnoreReason::UnsafeProposal,
    ),
];
const fn outbound_stage_rank(stage: wire::SumeragiV2OutboundIntentStage) -> u8 {
    match stage {
        wire::SumeragiV2OutboundIntentStage::PendingPersistence => 0,
        wire::SumeragiV2OutboundIntentStage::PendingSignature => 1,
        wire::SumeragiV2OutboundIntentStage::Queued => 2,
        wire::SumeragiV2OutboundIntentStage::Sent => 3,
    }
}
const fn progress_transition_is_public_at_view(
    transition: wire::SumeragiV2ProgressTransition,
    progress_view: u64,
    current_view: u64,
) -> bool {
    progress_view <= current_view
        || matches!(
            transition,
            wire::SumeragiV2ProgressTransition::CommitQuorum
                | wire::SumeragiV2ProgressTransition::DecisionPersisted
        )
}
fn bounded_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
fn duration_ms(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
fn queue_status(
    queue: wire::SumeragiV2QueueKind,
    depth: usize,
    capacity: usize,
    oldest_age: Option<std::time::Duration>,
    service_debt: u64,
) -> wire::SumeragiV2QueueStatus {
    wire::SumeragiV2QueueStatus {
        queue,
        depth: bounded_u32(depth),
        capacity: bounded_u32(capacity),
        oldest_age_ms: oldest_age.map(duration_ms),
        service_debt,
    }
}
fn deferred_queue_status(
    queue: wire::SumeragiV2QueueKind,
    inputs: &VecDeque<DeferredInput>,
    capacity: usize,
    now: Instant,
) -> wire::SumeragiV2QueueStatus {
    let oldest_age = inputs
        .iter()
        .map(|input| input.admitted_at)
        .min()
        .map(|oldest| now.saturating_duration_since(oldest));
    let service_debt = inputs
        .iter()
        .map(|input| input.eligible_skips)
        .max()
        .unwrap_or_default();
    queue_status(queue, inputs.len(), capacity, oldest_age, service_debt)
}
#[cfg(test)]
fn progress_rank(event: &reducer::Event) -> u8 {
    match event {
        reducer::Event::QuorumCertificateReceived { certificate, .. }
            if certificate.phase() == reducer::Phase::Commit =>
        {
            3
        }
        reducer::Event::TimeoutCertificateReceived { .. } => 2,
        reducer::Event::QuorumCertificateReceived { .. } => 1,
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::LocalProposalReady { .. }
        | reducer::Event::ProposalReceived { .. }
        | reducer::Event::VoteReceived { .. }
        | reducer::Event::TimeoutVoteReceived { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. }
        | reducer::Event::BodyAvailable { .. }
        | reducer::Event::BodyStored { .. }
        | reducer::Event::ValidationCompleted { .. }
        | reducer::Event::Persisted { .. }
        | reducer::Event::PersistenceFailed { .. }
        | reducer::Event::Signed { .. }
        | reducer::Event::ApplicationCompleted { .. } => 0,
    }
}
#[derive(Clone, Debug, Decode, Encode)]
struct WalEnvelopeV2 {
    protocol_version: u16,
    persistence_id: u64,
    record: WalRecordV2,
}
#[derive(Clone, Debug, Decode, Encode)]
enum WalRecordV2 {
    ProposalIntent(wire::Proposal),
    PrepareIntent(wire::Vote),
    ObservePrepare(wire::QuorumCertificate),
    LockAndCommit {
        prepare: wire::QuorumCertificate,
        vote: wire::Vote,
    },
    TimeoutIntent(wire::TimeoutVote),
    InstallTimeout(wire::TimeoutCertificate),
    Decision(wire::QuorumCertificate),
}
#[derive(Clone, Default)]
struct WireRegistry {
    wire_context: Option<wire::HeightContext>,
    context_id: Option<wire::HeightContextId>,
    peers: Vec<PeerId>,
    validators: BTreeMap<reducer::ValidatorId, wire::ValidatorIndex>,
    subjects: BTreeMap<reducer::Subject, wire::BlockSubject>,
    manifests: BTreeMap<(reducer::Round, reducer::Subject), wire::PayloadManifest>,
    execution_commitments: BTreeMap<(reducer::Round, reducer::Subject), wire::ExecutionCommitment>,
    certificates: BTreeMap<reducer::CertificateRef, wire::QuorumCertificate>,
    proposals: BTreeMap<(reducer::Round, reducer::Subject), wire::Proposal>,
}
include!("v2_wire_registry_and_authentication.rs");
fn verify_timeout_certificate(
    context: &wire::HeightContext,
    certificate: &wire::TimeoutCertificate,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    certificate.validate(context)?;
    for group in &certificate.groups {
        if let Some(highest) = &group.highest_prepare_qc {
            verify_quorum_certificate(context, highest, proofs_of_possession)?;
        }
        let signer = group
            .signers
            .first()
            .copied()
            .ok_or(wire::ValidationError::EmptyTimeoutGroup)?;
        let preimage = wire::TimeoutVote {
            round: certificate.round,
            highest_prepare_qc: group.highest_prepare_qc.clone(),
            signer,
            signature: Vec::new(),
        }
        .signature_preimage();
        verify_aggregate_signature(
            context,
            &group.signers,
            &group.aggregate_signature,
            &preimage,
            proofs_of_possession,
        )?;
    }
    Ok(())
}
fn verify_aggregate_signature(
    context: &wire::HeightContext,
    signers: &[wire::ValidatorIndex],
    aggregate_signature: &[u8],
    preimage: &[u8],
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    let mut public_keys = Vec::with_capacity(signers.len());
    let mut pops = Vec::with_capacity(signers.len());
    for signer in signers {
        let index = usize::try_from(*signer)
            .ok()
            .filter(|index| *index < context.roster.len() && *index < proofs_of_possession.len())
            .ok_or(AdapterError::ValidatorIndexOutOfRange(*signer))?;
        public_keys.push(context.roster[index].validator.public_key());
        pops.push(proofs_of_possession[index].as_slice());
    }
    #[cfg(feature = "bls")]
    {
        iroha_crypto::bls_normal_verify_preaggregated_same_message(
            preimage,
            aggregate_signature,
            &public_keys,
            &pops,
        )
        .map_err(|error| AdapterError::Cryptography(error.to_string()))
    }
    #[cfg(not(feature = "bls"))]
    {
        let _ = (public_keys, pops, aggregate_signature, preimage);
        Err(AdapterError::Cryptography(
            "the iroha_core `bls` feature is required by Sumeragi v2".to_owned(),
        ))
    }
}
fn validator_token(index: wire::ValidatorIndex) -> reducer::ValidatorId {
    let mut bytes = [0_u8; 32];
    bytes[28..].copy_from_slice(&index.to_be_bytes());
    reducer::ValidatorId::new(bytes)
}
fn context_id(id: wire::HeightContextId) -> reducer::ContextId {
    reducer::ContextId::new(*id.0.as_ref())
}
fn aggregate_token(signature: &[u8]) -> reducer::OpaqueSignature {
    let mut token = Vec::with_capacity(AGGREGATE_TOKEN_PREFIX.len() + signature.len());
    token.extend_from_slice(AGGREGATE_TOKEN_PREFIX);
    token.extend_from_slice(signature);
    reducer::OpaqueSignature::new(token)
}
fn aggregate_core_shares(
    shares: &[reducer::SignatureShare],
    aggregator: &dyn SignatureAggregator,
) -> Result<Vec<u8>, AdapterError> {
    let signatures = shares
        .iter()
        .map(|share| share.signature().as_bytes())
        .collect::<Vec<_>>();
    if let Some(first) = signatures.first()
        && let Some(aggregate) = first.strip_prefix(AGGREGATE_TOKEN_PREFIX)
    {
        if signatures
            .iter()
            .all(|signature| signature.strip_prefix(AGGREGATE_TOKEN_PREFIX) == Some(aggregate))
        {
            return Ok(aggregate.to_vec());
        }
        return Err(AdapterError::SignatureAggregation(
            "verified aggregate tokens disagree within one certificate".to_owned(),
        ));
    }
    if signatures
        .iter()
        .any(|signature| signature.starts_with(AGGREGATE_TOKEN_PREFIX))
    {
        return Err(AdapterError::SignatureAggregation(
            "verified aggregate tokens cannot be mixed with signature shares".to_owned(),
        ));
    }
    aggregator
        .aggregate(&signatures)
        .map_err(AdapterError::SignatureAggregation)
}
#[cfg(test)]
mod tests {
    include!("tests/v2_adapter_leader_wire_consumer.rs");
    include!("tests/v2_adapter_main_00.rs");
    include!("tests/v2_adapter_main_01.rs");
    include!("tests/v2_adapter_main_02.rs");
    include!("tests/v2_adapter_main_03.rs");
    include!("tests/v2_adapter_main_04.rs");

    /// Open one genuine recovered Decision Apply owner for cross-lineage tests.
    #[cfg(feature = "bls")]
    pub(in crate::sumeragi) fn recovered_decision_apply_owner_for_lineage_test(
        marker: u8,
    ) -> (
        super::super::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
        tempfile::TempDir,
        tempfile::TempDir,
    ) {
        let local_signer =
            iroha_crypto::KeyPair::try_from_seed(vec![1; 32], iroha_crypto::Algorithm::BlsNormal)
                .expect("deterministic recovered Decision Apply lineage fixture signer");
        let safety = tempfile::TempDir::new()
            .expect("temporary recovered Decision Apply lineage fixture WAL");
        let storage = tempfile::TempDir::new()
            .expect("temporary recovered Decision Apply lineage fixture stores");
        let (startup, body_store) = write_decision_startup_with_body_marker(
            &safety,
            &storage.path().join("body"),
            marker,
            DecisionBodyMarkerFixture::Validated,
        );
        let authenticated = startup
            .authenticate_final_wal_startup_authority()
            .unwrap_or_else(|(error, _)| {
                panic!("authenticate recovered Decision Apply lineage fixture: {error}")
            });
        let body_store = body_store
            .into_revalidated_startup()
            .expect("seal recovered Decision Apply lineage fixture body store");
        let owner = authenticated
            .open_production_lifecycle_owner_v1_with_store_for_test(
                &lifecycle_owner_config(),
                4,
                &storage.path().join("ledger"),
                &storage.path().join("serve"),
                body_store,
                &local_signer,
            )
            .unwrap_or_else(|error| {
                panic!("open recovered Decision Apply lineage fixture: {error}")
            });
        (owner, safety, storage)
    }
}
#[cfg(all(test, feature = "bls"))]
pub(in crate::sumeragi) use tests::recovered_decision_apply_owner_for_lineage_test;
