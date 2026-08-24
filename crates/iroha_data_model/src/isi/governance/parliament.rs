//! Canonical instruction surface for attempt-based SORA Parliament governance.
//!
//! The two instructions in this module deliberately separate immutable proposal
//! admission from reducer transitions.  Core derives block-local heights and
//! verifies authority, world-state bindings, sortition proofs, timed-OVN proofs,
//! and threshold-beacon material before applying a transition.  Consequently,
//! callers cannot supply an execution height through this wire surface.

use std::{cmp::Ordering, vec::Vec};

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    governance::types::{
        AssignmentId, BallotAttemptId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId,
        BodyInstanceId, DeliberationPhaseV1, GovernanceAttemptId, GovernanceAttemptStatusV1,
        GovernanceAttemptV1, GovernanceExpectedHeadV1, GovernanceStageV1, ParliamentBody,
        ProposalKind, RiskTierV1, SortitionRequestId, SortitionRequestV1, TleKeySessionId,
        TleSessionId,
    },
    seal,
};

/// Create one retryable Parliament attempt for exact immutable proposal content.
///
/// The instruction carries no caller-selected attempt identifier, status, stage,
/// policy version, effect hash, compare-and-set head, or body pipeline.  Core
/// derives those consensus bindings from the proposal and committed world state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct CreateParliamentGovernanceAttemptV1 {
    /// Exact typed proposal whose fingerprint is shared by every retry.
    pub proposal: ProposalKind,
    /// Zero-based end-to-end retry sequence for this proposal content.
    pub attempt_sequence: u32,
}

impl CreateParliamentGovernanceAttemptV1 {
    /// Stable path-independent identifier used by the instruction registry.
    pub const WIRE_ID: &'static str = "iroha.governance.parliament.attempt.create.v1";

    /// Derive the immutable proposal-content identifier.
    #[must_use]
    pub fn proposal_content_id(&self) -> crate::governance::types::ProposalContentId {
        crate::governance::types::ProposalContentId::derive_v1(&self.proposal)
    }

    /// Derive the only valid attempt identifier for this proposal and retry.
    #[must_use]
    pub fn governance_attempt_id(&self) -> GovernanceAttemptId {
        GovernanceAttemptId::derive_v1(self.proposal_content_id(), self.attempt_sequence)
    }

    /// Build the canonical initial reducer snapshot using a policy-derived risk tier.
    ///
    /// Core remains responsible for deriving and freezing every other immutable
    /// reducer binding from committed state.
    #[must_use]
    pub fn canonical_attempt(&self, risk_tier: RiskTierV1) -> GovernanceAttemptV1 {
        GovernanceAttemptV1 {
            id: self.governance_attempt_id(),
            proposal_content_id: self.proposal_content_id(),
            sequence: self.attempt_sequence,
            risk_tier,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        }
    }
}

impl PartialOrd for CreateParliamentGovernanceAttemptV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.encode().cmp(&other.encode()))
    }
}

impl seal::Instruction for CreateParliamentGovernanceAttemptV1 {}

/// Payload for a monotonic risk escalation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentEscalateRiskV1 {
    /// Strictly nondecreasing policy-derived risk tier.
    pub target: RiskTierV1,
}

/// Payload registering one immutable candidate snapshot for future sortition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentRegisterSortitionRequestV1 {
    /// Zero-based retry sequence for this body election.
    pub sequence: u32,
    /// Complete canonical future-pulse request.
    pub request: SortitionRequestV1,
    /// Strictly ordered complete account snapshot committed by `candidate_root`.
    pub candidate_snapshot: Vec<AccountId>,
}

/// Payload consuming one finalized threshold-beacon pulse batch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentConsumeSortitionPulseBatchV1 {
    /// Strictly ordered complete request identifiers for the pulse slot.
    pub request_ids: Vec<SortitionRequestId>,
    /// Threshold-beacon key session producing the pulse.
    pub beacon_session_id: BeaconSessionId,
    /// Exact finalized height committed by every request in the batch.
    pub pulse_height: u64,
    /// Content identifier of the finalized pulse.
    pub pulse_id: BeaconPulseId,
}

/// Payload beginning invitation acceptance after a deterministic draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentBeginInvitationAcceptanceV1 {
    /// Election attempt whose finalized-pulse draw completed.
    pub election_attempt_id: BodyElectionAttemptId,
}

/// Payload terminally recording that an election formed no roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentFailBodyElectionNoRosterV1 {
    /// Election attempt that failed.
    pub election_attempt_id: BodyElectionAttemptId,
}

/// A candidate's response to one canonical Parliament invitation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[repr(u8)]
pub enum ParliamentInvitationDecisionV1 {
    /// Accept the offered Parliament assignment.
    #[codec(index = 0)]
    Accept,
    /// Decline the offered Parliament assignment.
    #[codec(index = 1)]
    Decline,
}

/// Payload recording one authority-bound invitation response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentRecordInvitationResponseV1 {
    /// Election attempt whose ranked invitation is being answered.
    pub election_attempt_id: BodyElectionAttemptId,
    /// Parliament body copied from and rechecked against the election request.
    pub body: ParliamentBody,
    /// Accept or decline decision by the transaction authority.
    ///
    /// The member identity and assignment identifier are deliberately absent:
    /// Core derives both from the authenticated transaction authority.
    pub decision: ParliamentInvitationDecisionV1,
}

/// Payload triggering consensus-owned sealing of a canonical Parliament roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentSealBodyRosterV1 {
    /// Election whose ranked, authority-bound responses determine the roster.
    ///
    /// Core derives the exact assignments, roster root, and body-instance ID;
    /// none can be selected by the transition submitter.
    pub election_attempt_id: BodyElectionAttemptId,
}

/// Payload advancing one sealed body by exactly one deliberation phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentAdvanceBodyPhaseV1 {
    /// Body instance being advanced.
    pub body_instance_id: BodyInstanceId,
    /// Exact next deliberation phase.
    pub target: DeliberationPhaseV1,
}

/// Payload excluding one absent assignment from the current attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentRecordAttemptAbsenceV1 {
    /// Body instance whose member was absent.
    pub body_instance_id: BodyInstanceId,
    /// Canonical assignment excluded from this attempt only.
    pub assignment_id: AssignmentId,
}

/// Payload finalizing one public nonbinding Parliament finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentFinalizePublicFindingV1 {
    /// Body instance contributing the finding.
    pub body_instance_id: BodyInstanceId,
    /// Root of the complete evidence, deliberation, and dissent record.
    pub result_root: [u8; 32],
}

/// Payload registering a fresh private timed-OVN ballot attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentRegisterBallotAttemptV1 {
    /// Binding body instance receiving the private ballot.
    pub body_instance_id: BodyInstanceId,
    /// Identifier derived from the body instance and retry sequence.
    pub ballot_attempt_id: BallotAttemptId,
    /// Zero-based ballot retry sequence within the body instance.
    pub sequence: u32,
    /// Dedicated threshold timelock-encryption session.
    pub tle_session_id: TleSessionId,
    /// Long-lived threshold-BLS key session dedicated to TLE release signatures.
    pub tle_key_session_id: TleKeySessionId,
    /// Threshold-beacon key session committed for release timing.
    pub release_beacon_session_id: BeaconSessionId,
    /// Strictly future finalized height committed for release.
    pub release_height: u64,
}

/// Payload closing proof-validated timed-OVN registration.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentCloseBallotRegistrationV1 {
    /// Ballot attempt whose registration closes.
    pub ballot_attempt_id: BallotAttemptId,
    /// Exact canonical 3,624-byte timed-OVN registration records.
    ///
    /// Core decodes and verifies every record, preserves the supplied canonical
    /// order, and derives the registration root and count. The active
    /// governance configuration must enforce the V1 cap of 1,000 records.
    pub registration_records: Vec<Vec<u8>>,
}

/// Payload freezing the exact nonempty survivor subset.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentFreezeBallotSurvivorsV1 {
    /// Ballot attempt whose survivor set becomes immutable.
    pub ballot_attempt_id: BallotAttemptId,
    /// Strictly ordered nonempty participant hashes forming a roster subsequence.
    ///
    /// Core derives the survivor corpus root, count, and the suite-specific
    /// no-post-freeze-recovery sentinel from the proof-validated registration
    /// corpus. The active governance configuration must enforce the V1 cap of
    /// 1,000 participants.
    pub survivor_participant_hashes: Vec<[u8; 32]>,
}

/// Payload freezing the exact timed-OVN ciphertext and one-hot-proof corpus.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentFreezeTimedOvnCorpusV1 {
    /// Ballot attempt whose corpus becomes immutable.
    pub ballot_attempt_id: BallotAttemptId,
    /// Exact canonical 2,858-byte timed-OVN ballot records in survivor order.
    ///
    /// Core verifies every proof and exact survivor coverage, then derives all
    /// corpus roots, counts, and aggregate commitments. The active governance
    /// configuration must enforce the V1 cap of 1,000 records.
    pub ballot_records: Vec<Vec<u8>>,
}

/// Payload consuming one finalized release pulse for a complete ballot batch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentBeginBallotOpeningBatchV1 {
    /// Strictly ordered complete ballot attempts for the release slot.
    pub ballot_attempt_ids: Vec<BallotAttemptId>,
    /// Threshold-beacon key session committed by every ballot.
    pub release_beacon_session_id: BeaconSessionId,
    /// Exact committed release height.
    pub release_height: u64,
    /// Content identifier of the finalized release pulse.
    pub pulse_id: BeaconPulseId,
}

/// Payload terminally recording a cryptographic ballot protocol failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentFailBallotNoResultV1 {
    /// Ballot attempt that failed.
    pub ballot_attempt_id: BallotAttemptId,
    /// Nonzero root of deterministic failure evidence.
    ///
    /// Core must authorize this consensus-origin transition and must not accept
    /// an arbitrary transaction submitter's failure assertion.
    pub failure_root: [u8; 32],
}

/// Canonical public final threshold release record for one future TLE identity.
///
/// This schema DTO mirrors Core's verified release record without making the
/// data model depend on non-schema cryptographic runtime types. Core must
/// reconstruct the exact future identity and verify every field and pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentTleFinalReleaseSignatureV1 {
    /// Long-lived TLE key session producing the final signature.
    pub key_session_id: TleKeySessionId,
    /// SHA-256 of the exact Core-reconstructed future release identity.
    pub identity_digest: [u8; 32],
    /// Canonical standard BLS12-381 G1 threshold release signature.
    pub signature: [u8; 48],
}

/// Payload finalizing an aggregate-only timed-OVN opening.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentFinalizeOpenedBallotV1 {
    /// Ballot attempt whose complete survivor aggregate is opened.
    pub ballot_attempt_id: BallotAttemptId,
    /// Exact public threshold release record verified and replayed by Core.
    pub final_release: ParliamentTleFinalReleaseSignatureV1,
}

/// Payload constructing and freezing the automatic governance certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentConstructCertificateV1 {
    /// Exact deterministic height at which enactment is due.
    pub enact_at_height: u64,
}

/// Payload recording that a competing compare-and-set head won first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentMarkSupersededV1 {
    /// Different committed head observed when execution became due.
    pub observed_head: GovernanceExpectedHeadV1,
}

/// Payload recording deterministic execution failure of the certified effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ParliamentMarkExecutionFailedV1 {
    /// Exact certified effect preimage hash.
    pub effect_preimage_hash: [u8; 32],
    /// Nonzero root of deterministic execution-failure evidence.
    ///
    /// Core must authorize this consensus-origin transition and must not accept
    /// an arbitrary transaction submitter's failure assertion.
    pub failure_root: [u8; 32],
}

/// One closed, versioned transition accepted by the Parliament attempt reducer.
///
/// Heights representing when a transition executes are intentionally absent:
/// Core supplies the containing block height.  Future sortition and release
/// heights remain explicit because they are immutable precommitments.
#[expect(
    clippy::large_enum_variant,
    reason = "closed transition variants retain their canonical direct Norito payloads"
)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub enum ParliamentLifecycleTransitionV1 {
    /// Escalate the attempt's risk tier before Policy Jury sortition is frozen.
    #[codec(index = 0)]
    EscalateRisk(ParliamentEscalateRiskV1),
    /// Finish qualification and enter the first required Parliament body.
    #[codec(index = 1)]
    CompleteQualification,
    /// Register one immutable candidate snapshot for a future beacon pulse.
    #[codec(index = 2)]
    RegisterSortitionRequest(ParliamentRegisterSortitionRequestV1),
    /// Consume a finalized threshold-beacon pulse for a complete request batch.
    #[codec(index = 3)]
    ConsumeSortitionPulseBatch(ParliamentConsumeSortitionPulseBatchV1),
    /// Derive the deterministic draw plan and begin invitation acceptance.
    #[codec(index = 4)]
    BeginInvitationAcceptance(ParliamentBeginInvitationAcceptanceV1),
    /// Terminally record that an election could not form a nonempty roster.
    #[codec(index = 5)]
    FailBodyElectionNoRoster(ParliamentFailBodyElectionNoRosterV1),
    /// Seal a nonempty canonical roster into a body instance.
    #[codec(index = 6)]
    SealBodyRoster(ParliamentSealBodyRosterV1),
    /// Advance one sealed body by exactly one deliberation phase.
    #[codec(index = 7)]
    AdvanceBodyPhase(ParliamentAdvanceBodyPhaseV1),
    /// Exclude one absent assignment without changing the original-seat quorum.
    #[codec(index = 8)]
    RecordAttemptAbsence(ParliamentRecordAttemptAbsenceV1),
    /// Finalize a public nonbinding body finding.
    #[codec(index = 9)]
    FinalizePublicFinding(ParliamentFinalizePublicFindingV1),
    /// Register a fresh private timed-OVN ballot attempt.
    #[codec(index = 10)]
    RegisterBallotAttempt(ParliamentRegisterBallotAttemptV1),
    /// Close proof-validated OVN registration from its exact canonical corpus.
    #[codec(index = 11)]
    CloseBallotRegistration(ParliamentCloseBallotRegistrationV1),
    /// Freeze the nonempty survivor roster before accepting ballots.
    #[codec(index = 12)]
    FreezeBallotSurvivors(ParliamentFreezeBallotSurvivorsV1),
    /// Freeze the complete timed-OVN ciphertext and one-hot-proof corpus.
    #[codec(index = 13)]
    FreezeTimedOvnCorpus(ParliamentFreezeTimedOvnCorpusV1),
    /// Consume one finalized release pulse for a complete ballot batch.
    #[codec(index = 14)]
    BeginBallotOpeningBatch(ParliamentBeginBallotOpeningBatchV1),
    /// Terminally record a cryptographic ballot protocol failure.
    #[codec(index = 15)]
    FailBallotNoResult(ParliamentFailBallotNoResultV1),
    /// Finalize an aggregate-only timed-OVN opening and binding-body result.
    #[codec(index = 16)]
    FinalizeOpenedBallot(ParliamentFinalizeOpenedBallotV1),
    /// Construct and freeze the complete automatic governance certificate.
    #[codec(index = 17)]
    ConstructCertificate(ParliamentConstructCertificateV1),
    /// Record successful deterministic execution of a due certificate.
    #[codec(index = 18)]
    MarkEnacted,
    /// Record that a competing compare-and-set head superseded the certificate.
    #[codec(index = 19)]
    MarkSuperseded(ParliamentMarkSupersededV1),
    /// Record deterministic failure while executing the exact certified effect.
    #[codec(index = 20)]
    MarkExecutionFailed(ParliamentMarkExecutionFailedV1),
    /// Record one authenticated candidate's invitation response.
    #[codec(index = 21)]
    RecordInvitationResponse(ParliamentRecordInvitationResponseV1),
}

/// Bounded audit classification for a Parliament lifecycle transition.
///
/// Indices exactly mirror [`ParliamentLifecycleTransitionV1`] and never carry
/// registration or ballot corpora.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[repr(u8)]
pub enum ParliamentLifecycleTransitionKindV1 {
    /// Risk escalation.
    #[codec(index = 0)]
    EscalateRisk,
    /// Qualification completion.
    #[codec(index = 1)]
    CompleteQualification,
    /// Sortition-request registration.
    #[codec(index = 2)]
    RegisterSortitionRequest,
    /// Sortition-pulse batch consumption.
    #[codec(index = 3)]
    ConsumeSortitionPulseBatch,
    /// Invitation-acceptance start.
    #[codec(index = 4)]
    BeginInvitationAcceptance,
    /// Terminal election failure without a roster.
    #[codec(index = 5)]
    FailBodyElectionNoRoster,
    /// Canonical body-roster sealing.
    #[codec(index = 6)]
    SealBodyRoster,
    /// Body deliberation phase advancement.
    #[codec(index = 7)]
    AdvanceBodyPhase,
    /// Attempt-scoped absence recording.
    #[codec(index = 8)]
    RecordAttemptAbsence,
    /// Public finding finalization.
    #[codec(index = 9)]
    FinalizePublicFinding,
    /// Ballot-attempt registration.
    #[codec(index = 10)]
    RegisterBallotAttempt,
    /// Timed-OVN registration closure.
    #[codec(index = 11)]
    CloseBallotRegistration,
    /// Survivor-roster freezing.
    #[codec(index = 12)]
    FreezeBallotSurvivors,
    /// Timed-OVN ballot corpus freezing.
    #[codec(index = 13)]
    FreezeTimedOvnCorpus,
    /// Release-pulse batch consumption.
    #[codec(index = 14)]
    BeginBallotOpeningBatch,
    /// Terminal ballot failure without a result.
    #[codec(index = 15)]
    FailBallotNoResult,
    /// Aggregate-only ballot finalization.
    #[codec(index = 16)]
    FinalizeOpenedBallot,
    /// Governance-certificate construction.
    #[codec(index = 17)]
    ConstructCertificate,
    /// Successful certified enactment.
    #[codec(index = 18)]
    MarkEnacted,
    /// Compare-and-set supersession.
    #[codec(index = 19)]
    MarkSuperseded,
    /// Certified-effect execution failure.
    #[codec(index = 20)]
    MarkExecutionFailed,
    /// Authority-bound invitation response.
    #[codec(index = 21)]
    RecordInvitationResponse,
}

/// Domain separating the digest of an exact Parliament lifecycle transition.
pub const PARLIAMENT_LIFECYCLE_TRANSITION_DIGEST_V1: &[u8] =
    b"iroha.governance.parliament.lifecycle_transition.digest.v1";

impl ParliamentLifecycleTransitionV1 {
    /// Return the bounded audit classification for this transition.
    #[must_use]
    pub const fn kind(&self) -> ParliamentLifecycleTransitionKindV1 {
        match self {
            Self::EscalateRisk(_) => ParliamentLifecycleTransitionKindV1::EscalateRisk,
            Self::CompleteQualification => {
                ParliamentLifecycleTransitionKindV1::CompleteQualification
            }
            Self::RegisterSortitionRequest(_) => {
                ParliamentLifecycleTransitionKindV1::RegisterSortitionRequest
            }
            Self::ConsumeSortitionPulseBatch(_) => {
                ParliamentLifecycleTransitionKindV1::ConsumeSortitionPulseBatch
            }
            Self::BeginInvitationAcceptance(_) => {
                ParliamentLifecycleTransitionKindV1::BeginInvitationAcceptance
            }
            Self::FailBodyElectionNoRoster(_) => {
                ParliamentLifecycleTransitionKindV1::FailBodyElectionNoRoster
            }
            Self::SealBodyRoster(_) => ParliamentLifecycleTransitionKindV1::SealBodyRoster,
            Self::AdvanceBodyPhase(_) => ParliamentLifecycleTransitionKindV1::AdvanceBodyPhase,
            Self::RecordAttemptAbsence(_) => {
                ParliamentLifecycleTransitionKindV1::RecordAttemptAbsence
            }
            Self::FinalizePublicFinding(_) => {
                ParliamentLifecycleTransitionKindV1::FinalizePublicFinding
            }
            Self::RegisterBallotAttempt(_) => {
                ParliamentLifecycleTransitionKindV1::RegisterBallotAttempt
            }
            Self::CloseBallotRegistration(_) => {
                ParliamentLifecycleTransitionKindV1::CloseBallotRegistration
            }
            Self::FreezeBallotSurvivors(_) => {
                ParliamentLifecycleTransitionKindV1::FreezeBallotSurvivors
            }
            Self::FreezeTimedOvnCorpus(_) => {
                ParliamentLifecycleTransitionKindV1::FreezeTimedOvnCorpus
            }
            Self::BeginBallotOpeningBatch(_) => {
                ParliamentLifecycleTransitionKindV1::BeginBallotOpeningBatch
            }
            Self::FailBallotNoResult(_) => ParliamentLifecycleTransitionKindV1::FailBallotNoResult,
            Self::FinalizeOpenedBallot(_) => {
                ParliamentLifecycleTransitionKindV1::FinalizeOpenedBallot
            }
            Self::ConstructCertificate(_) => {
                ParliamentLifecycleTransitionKindV1::ConstructCertificate
            }
            Self::MarkEnacted => ParliamentLifecycleTransitionKindV1::MarkEnacted,
            Self::MarkSuperseded(_) => ParliamentLifecycleTransitionKindV1::MarkSuperseded,
            Self::MarkExecutionFailed(_) => {
                ParliamentLifecycleTransitionKindV1::MarkExecutionFailed
            }
            Self::RecordInvitationResponse(_) => {
                ParliamentLifecycleTransitionKindV1::RecordInvitationResponse
            }
        }
    }

    /// Derive a domain-separated digest of the exact transition and evidence.
    #[must_use]
    pub fn digest_v1(&self) -> [u8; 32] {
        crate::governance_fingerprint::fingerprint(PARLIAMENT_LIFECYCLE_TRANSITION_DIGEST_V1, self)
    }
}

/// Submit one transition for an existing Parliament governance attempt.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct SubmitParliamentLifecycleTransitionV1 {
    /// Attempt whose reducer must consume the transition.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Exact closed transition payload.
    pub transition: ParliamentLifecycleTransitionV1,
}

impl SubmitParliamentLifecycleTransitionV1 {
    /// Stable path-independent identifier used by the instruction registry.
    pub const WIRE_ID: &'static str = "iroha.governance.parliament.transition.submit.v1";
}

impl seal::Instruction for SubmitParliamentLifecycleTransitionV1 {}

fn decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for CreateParliamentGovernanceAttemptV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return crate::isi::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0;
        let proposal = crate::isi::decode_aos_canonical_field::<ProposalKind>(
            crate::isi::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let attempt_sequence = crate::isi::decode_aos_canonical_field::<u32>(
            crate::isi::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                proposal,
                attempt_sequence,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SubmitParliamentLifecycleTransitionV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return crate::isi::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0;
        let governance_attempt_id = crate::isi::decode_aos_canonical_field::<GovernanceAttemptId>(
            crate::isi::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let transition = crate::isi::decode_aos_canonical_field::<ParliamentLifecycleTransitionV1>(
            crate::isi::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                governance_attempt_id,
                transition,
            },
            offset,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        governance::types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBody,
            ParliamentSeatAssignmentV1, ProposalContentId, parliament_candidate_root_v1,
            parliament_roster_root_v1,
        },
        isi::test_support::{assert_registry_decodes, assert_slice_roundtrip},
        smart_contract::ContractAddress,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    fn proposal() -> ProposalKind {
        ProposalKind::DeployContract(DeployContractProposal {
            contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse::<ContractAddress>()
                .expect("parse Parliament instruction fixture contract address"),
            code_hash_hex: ContractCodeHash::new([0x11; 32]),
            abi_hash_hex: ContractAbiHash::new([0x22; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        })
    }

    fn transition(
        transition: ParliamentLifecycleTransitionV1,
    ) -> SubmitParliamentLifecycleTransitionV1 {
        SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: GovernanceAttemptId::new([0x33; 32]),
            transition,
        }
    }

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked Parliament instruction fixture account");
        AccountId::new(key_pair.public_key().clone())
    }

    #[test]
    fn attempt_creation_derives_only_canonical_identifiers_and_initial_state() {
        let instruction = CreateParliamentGovernanceAttemptV1 {
            proposal: proposal(),
            attempt_sequence: 3,
        };
        let proposal_content_id = ProposalContentId::derive_v1(&instruction.proposal);
        assert_eq!(instruction.proposal_content_id(), proposal_content_id);
        assert_eq!(
            instruction.governance_attempt_id(),
            GovernanceAttemptId::derive_v1(proposal_content_id, 3)
        );
        let attempt = instruction.canonical_attempt(RiskTierV1::Constitutional);
        assert!(attempt.has_canonical_id());
        assert_eq!(attempt.stage, GovernanceStageV1::Qualification);
        assert_eq!(attempt.status, GovernanceAttemptStatusV1::Active);
        assert_eq!(attempt.risk_tier, RiskTierV1::Constitutional);
        assert_slice_roundtrip(instruction);
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one table verifies every closed transition variant remains decodable"
    )]
    fn lifecycle_transition_instruction_roundtrips_every_variant() {
        let beacon_session_id = BeaconSessionId::new([0x44; 32]);
        let pulse_id = BeaconPulseId::new([0x45; 32]);
        let tle_key_session_id = TleKeySessionId::new([0x47; 32]);
        let governance_attempt_id = GovernanceAttemptId::new([0x33; 32]);
        let candidate_snapshot = vec![account(1)];
        let election_attempt_id = BodyElectionAttemptId::derive_v1(
            governance_attempt_id,
            ParliamentBody::RulesCommittee,
            0,
        );
        let request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            ParliamentBody::RulesCommittee,
            parliament_candidate_root_v1(
                governance_attempt_id,
                ParliamentBody::RulesCommittee,
                &candidate_snapshot,
            ),
            1,
            1,
            10,
            20,
            beacon_session_id,
            None,
        )
        .expect("construct canonical Parliament instruction sortition fixture");
        let seated_member = account(2);
        let assignments = vec![ParliamentSeatAssignmentV1 {
            assignment_id: AssignmentId::derive_v1(election_attempt_id, &seated_member),
            member: seated_member,
        }];
        let assignment_id = assignments[0].assignment_id;
        let roster_root = parliament_roster_root_v1(election_attempt_id, &assignments);
        let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
        let ballot_attempt_id = BallotAttemptId::derive_v1(body_instance_id, 0);
        let tle_session_id =
            TleSessionId::derive_v1(ballot_attempt_id, tle_key_session_id, beacon_session_id, 40);
        let variants = vec![
            ParliamentLifecycleTransitionV1::EscalateRisk(ParliamentEscalateRiskV1 {
                target: RiskTierV1::Constitutional,
            }),
            ParliamentLifecycleTransitionV1::CompleteQualification,
            ParliamentLifecycleTransitionV1::RegisterSortitionRequest(
                ParliamentRegisterSortitionRequestV1 {
                    sequence: 0,
                    request,
                    candidate_snapshot,
                },
            ),
            ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(
                ParliamentConsumeSortitionPulseBatchV1 {
                    request_ids: vec![request.id],
                    beacon_session_id,
                    pulse_height: 20,
                    pulse_id,
                },
            ),
            ParliamentLifecycleTransitionV1::BeginInvitationAcceptance(
                ParliamentBeginInvitationAcceptanceV1 {
                    election_attempt_id,
                },
            ),
            ParliamentLifecycleTransitionV1::FailBodyElectionNoRoster(
                ParliamentFailBodyElectionNoRosterV1 {
                    election_attempt_id,
                },
            ),
            ParliamentLifecycleTransitionV1::SealBodyRoster(ParliamentSealBodyRosterV1 {
                election_attempt_id,
            }),
            ParliamentLifecycleTransitionV1::AdvanceBodyPhase(ParliamentAdvanceBodyPhaseV1 {
                body_instance_id,
                target: DeliberationPhaseV1::Evidence,
            }),
            ParliamentLifecycleTransitionV1::RecordAttemptAbsence(
                ParliamentRecordAttemptAbsenceV1 {
                    body_instance_id,
                    assignment_id,
                },
            ),
            ParliamentLifecycleTransitionV1::FinalizePublicFinding(
                ParliamentFinalizePublicFindingV1 {
                    body_instance_id,
                    result_root: [0x53; 32],
                },
            ),
            ParliamentLifecycleTransitionV1::RegisterBallotAttempt(
                ParliamentRegisterBallotAttemptV1 {
                    body_instance_id,
                    ballot_attempt_id,
                    sequence: 0,
                    tle_session_id,
                    tle_key_session_id,
                    release_beacon_session_id: beacon_session_id,
                    release_height: 40,
                },
            ),
            ParliamentLifecycleTransitionV1::CloseBallotRegistration(
                ParliamentCloseBallotRegistrationV1 {
                    ballot_attempt_id,
                    registration_records: vec![vec![0x54; 3_624]],
                },
            ),
            ParliamentLifecycleTransitionV1::FreezeBallotSurvivors(
                ParliamentFreezeBallotSurvivorsV1 {
                    ballot_attempt_id,
                    survivor_participant_hashes: vec![[0x56; 32]],
                },
            ),
            ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(
                ParliamentFreezeTimedOvnCorpusV1 {
                    ballot_attempt_id,
                    ballot_records: vec![vec![0x58; 2_858]],
                },
            ),
            ParliamentLifecycleTransitionV1::BeginBallotOpeningBatch(
                ParliamentBeginBallotOpeningBatchV1 {
                    ballot_attempt_ids: vec![ballot_attempt_id],
                    release_beacon_session_id: beacon_session_id,
                    release_height: 40,
                    pulse_id,
                },
            ),
            ParliamentLifecycleTransitionV1::FailBallotNoResult(ParliamentFailBallotNoResultV1 {
                ballot_attempt_id,
                failure_root: [0x5A; 32],
            }),
            ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(
                ParliamentFinalizeOpenedBallotV1 {
                    ballot_attempt_id,
                    final_release: ParliamentTleFinalReleaseSignatureV1 {
                        key_session_id: tle_key_session_id,
                        identity_digest: [0x5B; 32],
                        signature: [0x5C; 48],
                    },
                },
            ),
            ParliamentLifecycleTransitionV1::ConstructCertificate(
                ParliamentConstructCertificateV1 {
                    enact_at_height: 50,
                },
            ),
            ParliamentLifecycleTransitionV1::MarkEnacted,
            ParliamentLifecycleTransitionV1::MarkSuperseded(ParliamentMarkSupersededV1 {
                observed_head: GovernanceExpectedHeadV1::Absent(
                    crate::governance::types::GovernanceExpectedHeadAbsentV1 {
                        subject_id: [0x5D; 32],
                    },
                ),
            }),
            ParliamentLifecycleTransitionV1::MarkExecutionFailed(ParliamentMarkExecutionFailedV1 {
                effect_preimage_hash: [0x5E; 32],
                failure_root: [0x5F; 32],
            }),
            ParliamentLifecycleTransitionV1::RecordInvitationResponse(
                ParliamentRecordInvitationResponseV1 {
                    election_attempt_id,
                    body: ParliamentBody::RulesCommittee,
                    decision: ParliamentInvitationDecisionV1::Accept,
                },
            ),
        ];
        for variant in variants {
            let kind = variant.kind();
            assert_eq!(kind.encode()[0], variant.encode()[0]);
            assert_ne!(variant.digest_v1(), [0; 32]);
            assert_slice_roundtrip(transition(variant));
        }
    }

    #[test]
    fn default_registry_decodes_parliament_instruction_wire_ids() {
        let registry = crate::isi::registry::default();
        assert_registry_decodes(
            &registry,
            CreateParliamentGovernanceAttemptV1::WIRE_ID,
            CreateParliamentGovernanceAttemptV1 {
                proposal: proposal(),
                attempt_sequence: 0,
            },
        );
        assert_registry_decodes(
            &registry,
            SubmitParliamentLifecycleTransitionV1::WIRE_ID,
            transition(ParliamentLifecycleTransitionV1::CompleteQualification),
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "each replayable private-evidence field has an explicit digest mutation case"
    )]
    fn lifecycle_digest_is_domain_separated_and_commits_to_all_private_evidence() {
        let ballot_attempt_id = BallotAttemptId::new([0x61; 32]);
        let domain_probe = ParliamentLifecycleTransitionV1::CloseBallotRegistration(
            ParliamentCloseBallotRegistrationV1 {
                ballot_attempt_id,
                registration_records: vec![vec![0x62; 3_624]],
            },
        );
        assert_eq!(
            domain_probe.digest_v1(),
            crate::governance_fingerprint::fingerprint(
                PARLIAMENT_LIFECYCLE_TRANSITION_DIGEST_V1,
                &domain_probe,
            )
        );
        assert_ne!(
            domain_probe.digest_v1(),
            crate::governance_fingerprint::fingerprint(
                b"iroha.governance.parliament.lifecycle_transition.other.v1",
                &domain_probe,
            )
        );

        let final_release =
            |key_byte, digest_byte, signature_byte| ParliamentTleFinalReleaseSignatureV1 {
                key_session_id: TleKeySessionId::new([key_byte; 32]),
                identity_digest: [digest_byte; 32],
                signature: [signature_byte; 48],
            };
        let pairs = vec![
            (
                ParliamentLifecycleTransitionV1::CloseBallotRegistration(
                    ParliamentCloseBallotRegistrationV1 {
                        ballot_attempt_id,
                        registration_records: vec![vec![0x62; 3_624]],
                    },
                ),
                ParliamentLifecycleTransitionV1::CloseBallotRegistration(
                    ParliamentCloseBallotRegistrationV1 {
                        ballot_attempt_id,
                        registration_records: vec![vec![0x63; 3_624]],
                    },
                ),
            ),
            (
                ParliamentLifecycleTransitionV1::FreezeBallotSurvivors(
                    ParliamentFreezeBallotSurvivorsV1 {
                        ballot_attempt_id,
                        survivor_participant_hashes: vec![[0x64; 32]],
                    },
                ),
                ParliamentLifecycleTransitionV1::FreezeBallotSurvivors(
                    ParliamentFreezeBallotSurvivorsV1 {
                        ballot_attempt_id,
                        survivor_participant_hashes: vec![[0x65; 32]],
                    },
                ),
            ),
            (
                ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(
                    ParliamentFreezeTimedOvnCorpusV1 {
                        ballot_attempt_id,
                        ballot_records: vec![vec![0x66; 2_858]],
                    },
                ),
                ParliamentLifecycleTransitionV1::FreezeTimedOvnCorpus(
                    ParliamentFreezeTimedOvnCorpusV1 {
                        ballot_attempt_id,
                        ballot_records: vec![vec![0x67; 2_858]],
                    },
                ),
            ),
            (
                ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(
                    ParliamentFinalizeOpenedBallotV1 {
                        ballot_attempt_id,
                        final_release: final_release(0x68, 0x69, 0x6A),
                    },
                ),
                ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(
                    ParliamentFinalizeOpenedBallotV1 {
                        ballot_attempt_id,
                        final_release: final_release(0x68, 0x69, 0x6B),
                    },
                ),
            ),
            (
                ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(
                    ParliamentFinalizeOpenedBallotV1 {
                        ballot_attempt_id,
                        final_release: final_release(0x68, 0x69, 0x6A),
                    },
                ),
                ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(
                    ParliamentFinalizeOpenedBallotV1 {
                        ballot_attempt_id,
                        final_release: final_release(0x68, 0x6C, 0x6A),
                    },
                ),
            ),
            (
                ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(
                    ParliamentFinalizeOpenedBallotV1 {
                        ballot_attempt_id,
                        final_release: final_release(0x68, 0x69, 0x6A),
                    },
                ),
                ParliamentLifecycleTransitionV1::FinalizeOpenedBallot(
                    ParliamentFinalizeOpenedBallotV1 {
                        ballot_attempt_id,
                        final_release: final_release(0x6D, 0x69, 0x6A),
                    },
                ),
            ),
        ];
        for (first, second) in pairs {
            assert_eq!(first.kind(), second.kind());
            assert_ne!(first.digest_v1(), second.digest_v1());
        }
    }

    #[test]
    fn slice_decoder_rejects_trailing_bytes() {
        let instruction = transition(ParliamentLifecycleTransitionV1::CompleteQualification);
        let mut encoded = instruction.encode();
        encoded.push(0);
        assert!(
            SubmitParliamentLifecycleTransitionV1::decode_from_slice(&encoded).is_err(),
            "instruction decoder must fully consume the canonical payload"
        );
    }
}
