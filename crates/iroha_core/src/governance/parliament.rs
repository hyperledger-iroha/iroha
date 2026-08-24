//! Deterministic, fail-closed reducer for one SORA Parliament governance attempt.
//!
//! The reducer owns the consensus-relevant lifecycle links between an immutable
//! proposal, future-beacon sortition, sealed body instances, private OVN ballot
//! attempts, certification, and enactment.  Cryptographic verification happens
//! before a transition is submitted; this module makes the verified bindings
//! immutable and rejects replay, stage skipping, and cross-attempt substitution.
//!
//! There is deliberately no plaintext ballot or manual-opening transition.  A
//! failed pulse, TLE opening, or proof produces `NoResult`; retry requires a
//! fresh ballot attempt and a fresh TLE session.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use iroha_config::parameters::actual::{Governance, ParliamentTimedOvn};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::types::{
        AssignmentId, BallotAttemptId, BallotAttemptStatusV1, BeaconPulseId, BeaconSessionId,
        BodyElectionAttemptId, BodyElectionAttemptStatusV1, BodyInstanceId, BodyInstanceStatusV1,
        DeliberationPhaseV1, GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceAttemptV1,
        GovernanceCertificateV1, GovernanceExpectedHeadV1, GovernanceStageV1,
        ParliamentAggregateOutcomeV1, ParliamentAggregateTallyV1, ParliamentBallotAttemptV1,
        ParliamentBallotCertificateBindingV1, ParliamentBallotFailureKindV1, ParliamentBody,
        ParliamentBodyCertificateBindingV1, ParliamentBodyInstanceV1,
        ParliamentSeatAssignmentV1, ProposalContentId, RiskTierV1, SortitionRequestId,
        SortitionRequestV1, TleKeySessionId, TleSessionId,
        MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1, MAX_PARLIAMENT_BALLOT_RETRIES_V1,
        parliament_assignment_plan_root_v1, parliament_ballot_result_root_v1,
        parliament_candidate_root_v1, parliament_roster_root_v1,
    },
};
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

use super::draw::{body_committee_size, derive_attempt_body_plan_v1};

/// A reducible entity named by [`ParliamentReducerErrorV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParliamentReducerEntityV1 {
    /// The end-to-end governance attempt.
    GovernanceAttempt,
    /// An immutable future-pulse sortition request.
    SortitionRequest,
    /// A retryable body-election attempt.
    BodyElection,
    /// A sealed Parliament body instance.
    BodyInstance,
    /// A retryable private ballot attempt.
    BallotAttempt,
    /// The automatic governance certificate.
    Certificate,
}

/// Fail-closed errors returned by the Parliament attempt reducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParliamentReducerErrorV1 {
    /// An operation named a different end-to-end attempt.
    AttemptBindingMismatch,
    /// A certificate named different immutable proposal content.
    ProposalBindingMismatch,
    /// The attempt is terminal or is not active for the requested operation.
    AttemptNotActive,
    /// The requested risk tier is below the already accepted tier.
    RiskDowngrade,
    /// Repeating the current risk tier is a replay, not an escalation.
    RiskEscalationReplay,
    /// Risk became immutable when Policy Jury sortition was requested.
    RiskTierLocked,
    /// The supplied required-body pipeline is not canonical V1 order.
    InvalidRequiredBodyPipeline,
    /// The operation was submitted at the wrong governance stage.
    WrongGovernanceStage {
        /// Stage required by the operation.
        expected: GovernanceStageV1,
        /// Stage currently occupied by the attempt.
        actual: GovernanceStageV1,
    },
    /// An object named a body other than the current required body.
    WrongParliamentBody,
    /// An entity identifier is zero or already registered.
    DuplicateOrZeroIdentifier(ParliamentReducerEntityV1),
    /// The named entity does not exist in this attempt.
    UnknownEntity(ParliamentReducerEntityV1),
    /// The entity is not in the one lifecycle state accepted by the operation.
    InvalidLifecycleTransition(ParliamentReducerEntityV1),
    /// A retry sequence was not exactly zero or the predecessor plus one.
    RetrySequenceMismatch,
    /// A request disagreed with its governance- or election-attempt binding.
    ImmutableBindingMismatch,
    /// A commitment root was zero.
    ZeroCommitmentRoot,
    /// A later transition supplied a root different from the frozen root.
    CommitmentRootMismatch,
    /// A future pulse did not match the immutable request.
    PulseBindingMismatch,
    /// A beacon pulse identifier or session-height slot was already consumed.
    BeaconPulseAlreadyConsumed,
    /// A TLE session was already bound to another ballot attempt.
    TleSessionAlreadyConsumed,
    /// The roster was empty, oversized, non-canonical, or internally duplicated.
    InvalidRoster,
    /// The stored assignment plan does not match the deterministic future-beacon draw.
    InvalidAssignmentPlan,
    /// The responding account was not selected as a primary or alternate.
    UnknownInvitation,
    /// The selected account already submitted its immutable response.
    InvitationResponseReplay,
    /// Invitation acceptance was opened with an invalid block-height window.
    InvalidInvitationWindow,
    /// An invitation response arrived after the immutable response window.
    InvitationWindowClosed,
    /// A roster or no-roster result was requested before the response window closed.
    InvitationWindowStillOpen,
    /// A candidate snapshot was empty, noncanonical, or disagreed with its committed root/count.
    InvalidCandidateSnapshot,
    /// The Confirmation Jury reused a Policy Jury member.
    ConfirmationJuryNotFresh,
    /// The transition used a public finding for a binding body, or vice versa.
    DecisionModeMismatch,
    /// A ballot count exceeded a frozen registration, survivor, or seat bound.
    InvalidBallotCount,
    /// A later ballot transition attempted to change the accepted corpus.
    AcceptedCorpusMutation,
    /// The release height was not strictly after the timed seal height.
    ReleaseHeightNotFuture,
    /// The immutable timed-ballot phase schedule is invalid or changed mid-attempt.
    InvalidBallotSchedule,
    /// A consensus transition did not execute at its immutable phase boundary.
    WrongBallotPhaseHeight,
    /// The requested retry exceeds the policy frozen for this lifecycle.
    BallotRetryLimitExceeded,
    /// The no-result reason does not match the ballot phase or its deadline.
    BallotFailureKindMismatch,
    /// Opening was requested before the committed release height.
    ReleaseHeightNotReached,
    /// The aggregate tally was malformed.
    InvalidTally,
    /// A final opening did not cover the complete frozen survivor set.
    IncompleteOpening,
    /// A required final body binding is absent or inconsistent.
    IncompleteCertificate,
    /// Certification or enactment heights violate their strict ordering.
    InvalidCertificateHeight,
    /// A supplied certificate or effect binding differs from reducer state.
    CertificateBindingMismatch,
    /// Supersession was reported without an actual compare-and-set head change.
    ExpectedHeadUnchanged,
}

impl fmt::Display for ParliamentReducerErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttemptBindingMismatch => f.write_str("governance attempt binding mismatch"),
            Self::ProposalBindingMismatch => f.write_str("proposal content binding mismatch"),
            Self::AttemptNotActive => f.write_str("governance attempt is not active"),
            Self::RiskDowngrade => f.write_str("governance risk may only escalate"),
            Self::RiskEscalationReplay => f.write_str("risk escalation replays the current tier"),
            Self::RiskTierLocked => {
                f.write_str("governance risk is locked by Policy Jury sortition")
            }
            Self::InvalidRequiredBodyPipeline => {
                f.write_str("required Parliament body pipeline is not canonical")
            }
            Self::WrongGovernanceStage { expected, actual } => {
                write!(
                    f,
                    "expected governance stage {expected:?}, found {actual:?}"
                )
            }
            Self::WrongParliamentBody => f.write_str("wrong Parliament body for current stage"),
            Self::DuplicateOrZeroIdentifier(entity) => {
                write!(f, "duplicate or zero {entity:?} identifier")
            }
            Self::UnknownEntity(entity) => write!(f, "unknown {entity:?}"),
            Self::InvalidLifecycleTransition(entity) => {
                write!(f, "invalid or replayed {entity:?} lifecycle transition")
            }
            Self::RetrySequenceMismatch => f.write_str("retry sequence mismatch"),
            Self::ImmutableBindingMismatch => f.write_str("immutable binding mismatch"),
            Self::ZeroCommitmentRoot => f.write_str("commitment root must not be zero"),
            Self::CommitmentRootMismatch => f.write_str("frozen commitment root mismatch"),
            Self::PulseBindingMismatch => f.write_str("future beacon pulse binding mismatch"),
            Self::BeaconPulseAlreadyConsumed => f.write_str("beacon pulse already consumed"),
            Self::TleSessionAlreadyConsumed => f.write_str("TLE session already consumed"),
            Self::InvalidRoster => f.write_str("invalid or non-canonical Parliament roster"),
            Self::InvalidAssignmentPlan => {
                f.write_str("invalid deterministic Parliament assignment plan")
            }
            Self::UnknownInvitation => f.write_str("account has no invitation in this election"),
            Self::InvitationResponseReplay => {
                f.write_str("Parliament invitation response already recorded")
            }
            Self::InvalidInvitationWindow => {
                f.write_str("invalid Parliament invitation response window")
            }
            Self::InvitationWindowClosed => {
                f.write_str("Parliament invitation response window is closed")
            }
            Self::InvitationWindowStillOpen => {
                f.write_str("Parliament invitation response window is still open")
            }
            Self::InvalidCandidateSnapshot => {
                f.write_str("invalid or mismatched Parliament candidate snapshot")
            }
            Self::ConfirmationJuryNotFresh => {
                f.write_str("Confirmation Jury is not disjoint from Policy Jury")
            }
            Self::DecisionModeMismatch => f.write_str("body decision mode mismatch"),
            Self::InvalidBallotCount => f.write_str("ballot count exceeds a frozen bound"),
            Self::AcceptedCorpusMutation => f.write_str("accepted ballot corpus is immutable"),
            Self::ReleaseHeightNotFuture => {
                f.write_str("ballot release height must be strictly future")
            }
            Self::InvalidBallotSchedule => f.write_str("invalid timed-ballot phase schedule"),
            Self::WrongBallotPhaseHeight => {
                f.write_str("timed-ballot transition is outside its immutable phase boundary")
            }
            Self::BallotRetryLimitExceeded => {
                f.write_str("private ballot retry limit exceeded")
            }
            Self::BallotFailureKindMismatch => {
                f.write_str("private ballot failure reason does not match its phase or deadline")
            }
            Self::ReleaseHeightNotReached => f.write_str("ballot release height not reached"),
            Self::InvalidTally => f.write_str("invalid private-ballot aggregate tally"),
            Self::IncompleteOpening => f.write_str("opening does not cover every survivor"),
            Self::IncompleteCertificate => f.write_str("governance certificate is incomplete"),
            Self::InvalidCertificateHeight => {
                f.write_str("invalid certification or enactment height")
            }
            Self::CertificateBindingMismatch => {
                f.write_str("governance certificate binding mismatch")
            }
            Self::ExpectedHeadUnchanged => {
                f.write_str("supersession requires a changed compare-and-set head")
            }
        }
    }
}

impl std::error::Error for ParliamentReducerErrorV1 {}

/// Whether a body emits a public nonbinding finding or a private binding ballot.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(tag = "mode", content = "details", deny_unknown_fields)]
pub enum ParliamentDecisionModeV1 {
    /// Public evidence and deliberation end in a nonbinding finding.
    PublicFinding,
    /// The body must use the private OVN/TLE ballot lifecycle.
    HiddenBindingBallot,
}

/// One body required by the attempt's immutable policy pipeline.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct RequiredParliamentBodyV1 {
    /// Body role that must be completed.
    pub body: ParliamentBody,
    /// Decision protocol required for the body.
    pub decision_mode: ParliamentDecisionModeV1,
}

/// The single reducer object allowed to consume a finalized beacon pulse.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, JsonSerialize, JsonDeserialize,
)]
#[norito(tag = "consumer", content = "bindings", deny_unknown_fields)]
enum ParliamentPulseConsumerV1 {
    /// A canonical simultaneous future-pulse sortition batch.
    SortitionBatch(Vec<SortitionRequestId>),
    /// A canonical simultaneous timed private-ballot opening batch.
    BallotBatch(Vec<BallotAttemptId>),
}

/// One globally unique threshold-beacon output slot.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
)]
struct ParliamentPulseSlotV1 {
    beacon_session_id: BeaconSessionId,
    height: u64,
}

impl ParliamentPulseSlotV1 {
    const fn new(beacon_session_id: BeaconSessionId, height: u64) -> Self {
        Self {
            beacon_session_id,
            height,
        }
    }
}

/// Reducer-owned state for one body-election attempt.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
pub struct ParliamentElectionStateV1 {
    attempt: iroha_data_model::governance::types::BodyElectionAttemptV1,
    candidate_snapshot: Vec<AccountId>,
    pulse_id: Option<BeaconPulseId>,
    pulse_output: Option<[u8; 32]>,
    assignment_root: Option<[u8; 32]>,
    primary_assignments: Vec<ParliamentSeatAssignmentV1>,
    alternate_assignments: Vec<ParliamentSeatAssignmentV1>,
    cross_body_assignment_cap: Option<u32>,
    invitation_opened_at_height: Option<u64>,
    invitation_close_height: Option<u64>,
    accepted_assignments: BTreeSet<AssignmentId>,
    declined_assignments: BTreeSet<AssignmentId>,
}

impl ParliamentElectionStateV1 {
    /// Return the canonical data-model election snapshot.
    #[must_use]
    pub const fn attempt(&self) -> &iroha_data_model::governance::types::BodyElectionAttemptV1 {
        &self.attempt
    }

    /// Return the exact canonically ordered candidate snapshot.
    #[must_use]
    pub fn candidate_snapshot(&self) -> &[AccountId] {
        &self.candidate_snapshot
    }

    /// Return the consumed pulse identifier, when drawing has begun.
    #[must_use]
    pub const fn pulse_id(&self) -> Option<BeaconPulseId> {
        self.pulse_id
    }

    /// Return the exact finalized beacon output used by the deterministic draw.
    #[must_use]
    pub const fn pulse_output(&self) -> Option<[u8; 32]> {
        self.pulse_output
    }

    /// Return the frozen assignment root, when invitations have begun.
    #[must_use]
    pub const fn assignment_root(&self) -> Option<[u8; 32]> {
        self.assignment_root
    }

    /// Return primary assignments in deterministic invitation rank order.
    #[must_use]
    pub fn primary_assignments(&self) -> &[ParliamentSeatAssignmentV1] {
        &self.primary_assignments
    }

    /// Return alternate assignments in deterministic replacement rank order.
    #[must_use]
    pub fn alternate_assignments(&self) -> &[ParliamentSeatAssignmentV1] {
        &self.alternate_assignments
    }

    /// Return the cross-body concentration cap used by the simultaneous draw.
    #[must_use]
    pub const fn cross_body_assignment_cap(&self) -> Option<u32> {
        self.cross_body_assignment_cap
    }

    /// Return the last height at which an invited citizen may respond.
    #[must_use]
    pub const fn invitation_close_height(&self) -> Option<u64> {
        self.invitation_close_height
    }

    /// Return immutable accepted assignment identifiers.
    #[must_use]
    pub const fn accepted_assignments(&self) -> &BTreeSet<AssignmentId> {
        &self.accepted_assignments
    }

    /// Return immutable declined assignment identifiers.
    #[must_use]
    pub const fn declined_assignments(&self) -> &BTreeSet<AssignmentId> {
        &self.declined_assignments
    }
}

/// Reducer-owned state for one sealed body instance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
pub struct ParliamentBodyStateV1 {
    instance: ParliamentBodyInstanceV1,
    roster_root: [u8; 32],
    assignments: Vec<ParliamentSeatAssignmentV1>,
    excluded_assignments: BTreeSet<AssignmentId>,
    result_root: Option<[u8; 32]>,
    result_height: Option<u64>,
    ballot_binding: Option<ParliamentBallotCertificateBindingV1>,
}

impl ParliamentBodyStateV1 {
    /// Return the canonical body-instance snapshot.
    #[must_use]
    pub const fn instance(&self) -> &ParliamentBodyInstanceV1 {
        &self.instance
    }

    /// Return the immutable ordered roster root.
    #[must_use]
    pub const fn roster_root(&self) -> [u8; 32] {
        self.roster_root
    }

    /// Return the canonical sealed seat assignments.
    #[must_use]
    pub fn assignments(&self) -> &[ParliamentSeatAssignmentV1] {
        &self.assignments
    }

    /// Return attempt-local absence exclusions.
    #[must_use]
    pub const fn excluded_assignments(&self) -> &BTreeSet<AssignmentId> {
        &self.excluded_assignments
    }

    /// Return the final public result root, when the body has completed.
    #[must_use]
    pub const fn result_root(&self) -> Option<[u8; 32]> {
        self.result_root
    }

    /// Return the height at which the body result became immutable.
    #[must_use]
    pub const fn result_height(&self) -> Option<u64> {
        self.result_height
    }
}

/// Reducer-owned transcript bindings for one private ballot attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
pub struct ParliamentBallotStateV1 {
    attempt: ParliamentBallotAttemptV1,
    registration_root: Option<[u8; 32]>,
    registered_voters: Option<u32>,
    corpus_root: Option<[u8; 32]>,
    accepted_ballots: Option<u32>,
    dropout_root: Option<[u8; 32]>,
    survivor_root: Option<[u8; 32]>,
    survivors: Option<u32>,
    no_recovery_root: Option<[u8; 32]>,
    tle_session_id: Option<TleSessionId>,
    tle_key_session_id: Option<TleKeySessionId>,
    release_beacon_session_id: Option<BeaconSessionId>,
    registered_at_height: u64,
    registration_phase_blocks: u64,
    survivor_freeze_phase_blocks: u64,
    commitment_phase_blocks: u64,
    release_delay_blocks: u64,
    max_ballot_retries: u32,
    max_corpus_entries: u32,
    registration_close_height: u64,
    survivor_freeze_height: u64,
    commitment_close_height: u64,
    registration_closed_at_height: Option<u64>,
    survivors_frozen_at_height: Option<u64>,
    commitment_closed_at_height: Option<u64>,
    timed_commitment_root: Option<[u8; 32]>,
    release_height: Option<u64>,
    release_pulse_id: Option<BeaconPulseId>,
    opening_height: Option<u64>,
    opening_root: Option<[u8; 32]>,
    tally: Option<ParliamentAggregateTallyV1>,
    outcome: Option<ParliamentAggregateOutcomeV1>,
    failure_root: Option<[u8; 32]>,
    failure_kind: Option<ParliamentBallotFailureKindV1>,
    failure_height: Option<u64>,
}

impl ParliamentBallotStateV1 {
    /// Return the canonical ballot-attempt snapshot.
    #[must_use]
    pub const fn attempt(&self) -> &ParliamentBallotAttemptV1 {
        &self.attempt
    }

    /// Return the frozen accepted corpus root, when commitments have closed.
    #[must_use]
    pub const fn corpus_root(&self) -> Option<[u8; 32]> {
        self.corpus_root
    }

    /// Return the accepted ballot count, when commitments have closed.
    #[must_use]
    pub const fn accepted_ballots(&self) -> Option<u32> {
        self.accepted_ballots
    }

    /// Return the dedicated TLE session, once the timed seal is complete.
    #[must_use]
    pub const fn tle_session_id(&self) -> Option<TleSessionId> {
        self.tle_session_id
    }

    /// Return the committed release height, once the timed seal is complete.
    #[must_use]
    pub const fn release_height(&self) -> Option<u64> {
        self.release_height
    }
}

/// Deterministic aggregate state for one immutable proposal attempt.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
pub struct ParliamentAttemptStateV1 {
    attempt: GovernanceAttemptV1,
    policy_version: u64,
    effect_preimage_hash: [u8; 32],
    expected_head: GovernanceExpectedHeadV1,
    required_bodies: Vec<RequiredParliamentBodyV1>,
    risk_locked: bool,
    elections: BTreeMap<BodyElectionAttemptId, ParliamentElectionStateV1>,
    active_elections: BTreeMap<ParliamentBody, BodyElectionAttemptId>,
    bodies: BTreeMap<BodyInstanceId, ParliamentBodyStateV1>,
    active_bodies: BTreeMap<ParliamentBody, BodyInstanceId>,
    ballots: BTreeMap<BallotAttemptId, ParliamentBallotStateV1>,
    active_ballots: BTreeMap<BodyInstanceId, BallotAttemptId>,
    body_bindings: BTreeMap<ParliamentBody, ParliamentBodyCertificateBindingV1>,
    used_pulse_ids: BTreeMap<BeaconPulseId, ParliamentPulseConsumerV1>,
    used_pulse_slots: BTreeMap<ParliamentPulseSlotV1, BeaconPulseId>,
    used_tle_sessions: BTreeMap<TleSessionId, BallotAttemptId>,
    certificate: Option<GovernanceCertificateV1>,
    terminal_height: Option<u64>,
    superseding_head: Option<GovernanceExpectedHeadV1>,
    execution_failure_root: Option<[u8; 32]>,
}

fn root_is_zero(root: &[u8; 32]) -> bool {
    root.iter().all(|byte| *byte == 0)
}

fn timed_ballot_schedule(
    registered_at_height: u64,
    policy: ParliamentTimedOvn,
) -> Result<(u64, u64, u64, u64), ParliamentReducerErrorV1> {
    if registered_at_height == 0
        || policy.registration_phase_blocks == 0
        || policy.survivor_freeze_phase_blocks == 0
        || policy.commitment_phase_blocks == 0
        || policy.release_delay_blocks == 0
        || policy.max_ballot_retries > MAX_PARLIAMENT_BALLOT_RETRIES_V1
        || !(1..=MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1)
            .contains(&policy.max_corpus_entries)
    {
        return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
    }
    let registration_close_height = registered_at_height
        .checked_add(policy.registration_phase_blocks)
        .ok_or(ParliamentReducerErrorV1::InvalidBallotSchedule)?;
    let survivor_freeze_height = registration_close_height
        .checked_add(policy.survivor_freeze_phase_blocks)
        .ok_or(ParliamentReducerErrorV1::InvalidBallotSchedule)?;
    let commitment_close_height = survivor_freeze_height
        .checked_add(policy.commitment_phase_blocks)
        .ok_or(ParliamentReducerErrorV1::InvalidBallotSchedule)?;
    let release_height = commitment_close_height
        .checked_add(policy.release_delay_blocks)
        .ok_or(ParliamentReducerErrorV1::InvalidBallotSchedule)?;
    Ok((
        registration_close_height,
        survivor_freeze_height,
        commitment_close_height,
        release_height,
    ))
}

fn ballot_policy_matches(ballot: &ParliamentBallotStateV1, policy: ParliamentTimedOvn) -> bool {
    ballot.registration_phase_blocks == policy.registration_phase_blocks
        && ballot.survivor_freeze_phase_blocks == policy.survivor_freeze_phase_blocks
        && ballot.commitment_phase_blocks == policy.commitment_phase_blocks
        && ballot.release_delay_blocks == policy.release_delay_blocks
        && ballot.max_ballot_retries == policy.max_ballot_retries
        && ballot.max_corpus_entries == policy.max_corpus_entries
}

fn accepted_roster(
    election: &ParliamentElectionStateV1,
) -> Result<Vec<ParliamentSeatAssignmentV1>, ParliamentReducerErrorV1> {
    let target = usize::try_from(election.attempt.request.target_seats)
        .map_err(|_| ParliamentReducerErrorV1::InvalidRoster)?;
    let mut assignments: Vec<_> = election
        .primary_assignments
        .iter()
        .chain(&election.alternate_assignments)
        .filter(|assignment| {
            election
                .accepted_assignments
                .contains(&assignment.assignment_id)
        })
        .take(target)
        .cloned()
        .collect();
    assignments.sort_unstable_by_key(|assignment| assignment.assignment_id);
    Ok(assignments)
}

fn stage_for_body(body: ParliamentBody) -> GovernanceStageV1 {
    match body {
        ParliamentBody::RulesCommittee => GovernanceStageV1::Rules,
        ParliamentBody::AgendaCouncil => GovernanceStageV1::Agenda,
        ParliamentBody::InterestPanel => GovernanceStageV1::Interest,
        ParliamentBody::ReviewPanel => GovernanceStageV1::Review,
        ParliamentBody::CoordinationCouncil => GovernanceStageV1::Coordination,
        ParliamentBody::MpcCommittee => GovernanceStageV1::Mpc,
        ParliamentBody::FmaCommittee => GovernanceStageV1::Fma,
        ParliamentBody::OversightCommittee => GovernanceStageV1::Oversight,
        ParliamentBody::PolicyJury => GovernanceStageV1::PolicyJury,
        ParliamentBody::ConfirmationJury => GovernanceStageV1::ConfirmationJury,
    }
}

fn next_deliberation_phase(phase: DeliberationPhaseV1) -> Option<DeliberationPhaseV1> {
    match phase {
        DeliberationPhaseV1::Orientation => Some(DeliberationPhaseV1::Evidence),
        DeliberationPhaseV1::Evidence => Some(DeliberationPhaseV1::Questions),
        DeliberationPhaseV1::Questions => Some(DeliberationPhaseV1::Responses),
        DeliberationPhaseV1::Responses => Some(DeliberationPhaseV1::Deliberation),
        DeliberationPhaseV1::Deliberation => Some(DeliberationPhaseV1::Reflection),
        DeliberationPhaseV1::Reflection => Some(DeliberationPhaseV1::Vote),
        DeliberationPhaseV1::Vote => None,
    }
}

fn required_pipeline_is_canonical(required: &[RequiredParliamentBodyV1]) -> bool {
    if required.is_empty()
        || required.last().map(|entry| entry.body) != Some(ParliamentBody::PolicyJury)
    {
        return false;
    }
    let mut previous_stage = None;
    for entry in required {
        if entry.body == ParliamentBody::ConfirmationJury {
            return false;
        }
        let stage = stage_for_body(entry.body);
        if previous_stage.is_some_and(|previous| previous >= stage) {
            return false;
        }
        if entry.body == ParliamentBody::PolicyJury
            && entry.decision_mode != ParliamentDecisionModeV1::HiddenBindingBallot
        {
            return false;
        }
        previous_stage = Some(stage);
    }
    true
}

fn persisted_pipeline_is_canonical(required: &[RequiredParliamentBodyV1]) -> bool {
    match required.last() {
        Some(RequiredParliamentBodyV1 {
            body: ParliamentBody::ConfirmationJury,
            decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
        }) => required_pipeline_is_canonical(&required[..required.len() - 1]),
        _ => required_pipeline_is_canonical(required),
    }
}

impl ParliamentAttemptStateV1 {
    /// Construct a reducer for one immutable governance attempt.
    ///
    /// The initial snapshot must be active at `Qualification`. The required
    /// bodies must be strictly ordered by the V1 pipeline, end in a private
    /// Policy Jury, and omit the dynamically required Confirmation Jury.
    ///
    /// # Errors
    /// Returns an error for zero immutable bindings, a noninitial attempt, or a
    /// noncanonical required-body pipeline.
    pub fn try_new(
        attempt: GovernanceAttemptV1,
        policy_version: u64,
        effect_preimage_hash: [u8; 32],
        expected_head: GovernanceExpectedHeadV1,
        required_bodies: Vec<RequiredParliamentBodyV1>,
    ) -> Result<Self, ParliamentReducerErrorV1> {
        if attempt.id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(ParliamentReducerErrorV1::DuplicateOrZeroIdentifier(
                ParliamentReducerEntityV1::GovernanceAttempt,
            ));
        }
        if !attempt.has_canonical_id() {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        if attempt
            .proposal_content_id
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
            || policy_version == 0
            || root_is_zero(&effect_preimage_hash)
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        if attempt.status != GovernanceAttemptStatusV1::Active
            || attempt.stage != GovernanceStageV1::Qualification
        {
            return Err(ParliamentReducerErrorV1::AttemptNotActive);
        }
        if !required_pipeline_is_canonical(&required_bodies) {
            return Err(ParliamentReducerErrorV1::InvalidRequiredBodyPipeline);
        }
        Ok(Self {
            attempt,
            policy_version,
            effect_preimage_hash,
            expected_head,
            required_bodies,
            risk_locked: false,
            elections: BTreeMap::new(),
            active_elections: BTreeMap::new(),
            bodies: BTreeMap::new(),
            active_bodies: BTreeMap::new(),
            ballots: BTreeMap::new(),
            active_ballots: BTreeMap::new(),
            body_bindings: BTreeMap::new(),
            used_pulse_ids: BTreeMap::new(),
            used_pulse_slots: BTreeMap::new(),
            used_tle_sessions: BTreeMap::new(),
            certificate: None,
            terminal_height: None,
            superseding_head: None,
            execution_failure_root: None,
        })
    }

    /// Return the canonical governance-attempt snapshot.
    #[must_use]
    pub const fn attempt(&self) -> &GovernanceAttemptV1 {
        &self.attempt
    }

    /// Return the immutable proposal content identifier.
    #[must_use]
    pub const fn proposal_content_id(&self) -> ProposalContentId {
        self.attempt.proposal_content_id
    }

    /// Return the policy version frozen for the attempt.
    #[must_use]
    pub const fn policy_version(&self) -> u64 {
        self.policy_version
    }

    /// Return the ordered body requirements, including dynamic confirmation.
    #[must_use]
    pub fn required_bodies(&self) -> &[RequiredParliamentBodyV1] {
        &self.required_bodies
    }

    /// Return a registered body-election attempt.
    #[must_use]
    pub fn election(&self, id: &BodyElectionAttemptId) -> Option<&ParliamentElectionStateV1> {
        self.elections.get(id)
    }

    /// Return a sealed body instance.
    #[must_use]
    pub fn body(&self, id: &BodyInstanceId) -> Option<&ParliamentBodyStateV1> {
        self.bodies.get(id)
    }

    /// Return a private ballot attempt.
    #[must_use]
    pub fn ballot(&self, id: &BallotAttemptId) -> Option<&ParliamentBallotStateV1> {
        self.ballots.get(id)
    }

    /// Return the constructed certificate after certification.
    #[must_use]
    pub const fn certificate(&self) -> Option<&GovernanceCertificateV1> {
        self.certificate.as_ref()
    }

    /// Return the committed height of a terminal enactment outcome.
    #[must_use]
    pub const fn terminal_height(&self) -> Option<u64> {
        self.terminal_height
    }

    /// Return the observed compare-and-set head that superseded this certificate.
    #[must_use]
    pub const fn superseding_head(&self) -> Option<GovernanceExpectedHeadV1> {
        self.superseding_head
    }

    /// Return the deterministic execution-failure transcript root, when present.
    #[must_use]
    pub const fn execution_failure_root(&self) -> Option<[u8; 32]> {
        self.execution_failure_root
    }

    fn ensure_attempt(
        &self,
        governance_attempt_id: GovernanceAttemptId,
    ) -> Result<(), ParliamentReducerErrorV1> {
        if governance_attempt_id != self.attempt.id {
            return Err(ParliamentReducerErrorV1::AttemptBindingMismatch);
        }
        Ok(())
    }

    fn ensure_active(
        &self,
        governance_attempt_id: GovernanceAttemptId,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_attempt(governance_attempt_id)?;
        if self.attempt.status != GovernanceAttemptStatusV1::Active {
            return Err(ParliamentReducerErrorV1::AttemptNotActive);
        }
        Ok(())
    }

    fn ensure_stage(&self, expected: GovernanceStageV1) -> Result<(), ParliamentReducerErrorV1> {
        if self.attempt.stage != expected {
            return Err(ParliamentReducerErrorV1::WrongGovernanceStage {
                expected,
                actual: self.attempt.stage,
            });
        }
        Ok(())
    }

    fn requirement_for_body(
        &self,
        body: ParliamentBody,
    ) -> Result<RequiredParliamentBodyV1, ParliamentReducerErrorV1> {
        self.required_bodies
            .iter()
            .copied()
            .find(|entry| entry.body == body)
            .ok_or(ParliamentReducerErrorV1::WrongParliamentBody)
    }

    fn ensure_current_body(
        &self,
        body: ParliamentBody,
    ) -> Result<RequiredParliamentBodyV1, ParliamentReducerErrorV1> {
        let requirement = self.requirement_for_body(body)?;
        self.ensure_stage(stage_for_body(body))?;
        Ok(requirement)
    }

    fn ensure_draw_eligible_body(
        &self,
        body: ParliamentBody,
    ) -> Result<RequiredParliamentBodyV1, ParliamentReducerErrorV1> {
        let requirement = self.requirement_for_body(body)?;
        let body_stage = stage_for_body(body);
        if self.attempt.stage != GovernanceStageV1::Qualification && self.attempt.stage > body_stage
        {
            return Err(ParliamentReducerErrorV1::WrongGovernanceStage {
                expected: body_stage,
                actual: self.attempt.stage,
            });
        }
        if self.body_bindings.contains_key(&body) {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        Ok(requirement)
    }

    fn advance_after_body(&mut self, body: ParliamentBody) -> Result<(), ParliamentReducerErrorV1> {
        let index = self
            .required_bodies
            .iter()
            .position(|entry| entry.body == body)
            .ok_or(ParliamentReducerErrorV1::WrongParliamentBody)?;
        self.attempt.stage = self
            .required_bodies
            .get(index + 1)
            .map_or(GovernanceStageV1::Certification, |next| {
                stage_for_body(next.body)
            });
        Ok(())
    }

    /// Escalate the risk tier before Policy Jury sortition locks it.
    ///
    /// # Errors
    /// Returns an error for a downgrade, replay, terminal attempt, or escalation
    /// after the first Policy Jury request.
    pub fn escalate_risk(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        target: RiskTierV1,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if self.risk_locked {
            return Err(ParliamentReducerErrorV1::RiskTierLocked);
        }
        if target == self.attempt.risk_tier {
            return Err(ParliamentReducerErrorV1::RiskEscalationReplay);
        }
        if !self.attempt.risk_tier.can_escalate_to(target) {
            return Err(ParliamentReducerErrorV1::RiskDowngrade);
        }
        self.attempt.risk_tier = target;
        Ok(())
    }

    /// Finish qualification and enter the first policy-required body stage.
    ///
    /// # Errors
    /// Returns an error for a wrong attempt, replay, terminal state, or skipped stage.
    pub fn complete_qualification(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        self.ensure_stage(GovernanceStageV1::Qualification)?;
        let first = self
            .required_bodies
            .first()
            .ok_or(ParliamentReducerErrorV1::InvalidRequiredBodyPipeline)?;
        self.attempt.stage = stage_for_body(first.body);
        Ok(())
    }
}

impl ParliamentAttemptStateV1 {
    /// Register an immutable body-election attempt and future-pulse request.
    ///
    /// A retry is accepted only after the prior election reached `NoRoster`,
    /// uses sequence `previous + 1`, and supersedes that prior attempt.
    ///
    /// # Errors
    /// Returns an error for wrong bindings, invalid request bounds, duplicate
    /// identifiers, wrong stage/body, pulse reuse, or a noncanonical retry.
    pub fn register_sortition_request(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        sequence: u32,
        request: SortitionRequestV1,
        candidate_snapshot: Vec<AccountId>,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        self.ensure_draw_eligible_body(request.body)?;
        if request.governance_attempt_id != governance_attempt_id {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        if request.id.as_bytes().iter().all(|byte| *byte == 0)
            || request
                .body_election_attempt_id
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(ParliamentReducerErrorV1::DuplicateOrZeroIdentifier(
                ParliamentReducerEntityV1::SortitionRequest,
            ));
        }
        if root_is_zero(&request.candidate_root) {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        if candidate_snapshot.is_empty()
            || !candidate_snapshot.windows(2).all(|pair| pair[0] < pair[1])
            || u32::try_from(candidate_snapshot.len()).ok() != Some(request.candidate_count)
            || request.candidate_root
                != parliament_candidate_root_v1(
                    governance_attempt_id,
                    request.body,
                    &candidate_snapshot,
                )
        {
            return Err(ParliamentReducerErrorV1::InvalidCandidateSnapshot);
        }
        if self
            .elections
            .values()
            .any(|election| election.attempt.request.id == request.id)
            || self
                .elections
                .contains_key(&request.body_election_attempt_id)
        {
            return Err(ParliamentReducerErrorV1::DuplicateOrZeroIdentifier(
                ParliamentReducerEntityV1::BodyElection,
            ));
        }
        let last_consumed = self
            .used_pulse_slots
            .keys()
            .filter_map(|slot| {
                (slot.beacon_session_id == request.beacon_session_id).then_some(slot.height)
            })
            .max();
        request
            .validate(last_consumed)
            .map_err(|_| ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
        if request.body == ParliamentBody::ConfirmationJury {
            let policy_result_height = self
                .body_bindings
                .get(&ParliamentBody::PolicyJury)
                .map(|binding| binding.result_height)
                .ok_or(ParliamentReducerErrorV1::ConfirmationJuryNotFresh)?;
            if request.request_height <= policy_result_height {
                return Err(ParliamentReducerErrorV1::ConfirmationJuryNotFresh);
            }
            let policy_members: BTreeSet<_> = self
                .bodies
                .values()
                .find(|body| body.instance.body == ParliamentBody::PolicyJury)
                .ok_or(ParliamentReducerErrorV1::ConfirmationJuryNotFresh)?
                .assignments
                .iter()
                .map(|assignment| assignment.member.clone())
                .collect();
            if candidate_snapshot
                .iter()
                .any(|candidate| policy_members.contains(candidate))
            {
                return Err(ParliamentReducerErrorV1::ConfirmationJuryNotFresh);
            }
        }
        if self
            .used_pulse_slots
            .contains_key(&ParliamentPulseSlotV1::new(
                request.beacon_session_id,
                request.pulse_height,
            ))
        {
            return Err(ParliamentReducerErrorV1::BeaconPulseAlreadyConsumed);
        }

        let predecessor = self.active_elections.get(&request.body).copied();
        match predecessor {
            None if sequence != 0 => {
                return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
            }
            Some(previous_id) => {
                let previous = self.elections.get(&previous_id).ok_or(
                    ParliamentReducerErrorV1::UnknownEntity(
                        ParliamentReducerEntityV1::BodyElection,
                    ),
                )?;
                if previous.attempt.status != BodyElectionAttemptStatusV1::NoRoster {
                    return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                        ParliamentReducerEntityV1::BodyElection,
                    ));
                }
                if sequence != previous.attempt.sequence.saturating_add(1) {
                    return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
                }
            }
            None => {}
        }

        let attempt = iroha_data_model::governance::types::BodyElectionAttemptV1::try_new(
            request.body_election_attempt_id,
            governance_attempt_id,
            sequence,
            request,
            BodyElectionAttemptStatusV1::AwaitingPulse,
        )
        .map_err(|_| ParliamentReducerErrorV1::ImmutableBindingMismatch)?;

        if let Some(previous_id) = predecessor {
            self.elections
                .get_mut(&previous_id)
                .expect("predecessor checked above")
                .attempt
                .status = BodyElectionAttemptStatusV1::Superseded;
        }
        if request.body == ParliamentBody::PolicyJury {
            self.risk_locked = true;
        }
        self.active_elections
            .insert(request.body, request.body_election_attempt_id);
        self.elections.insert(
            request.body_election_attempt_id,
            ParliamentElectionStateV1 {
                attempt,
                candidate_snapshot,
                pulse_id: None,
                pulse_output: None,
                assignment_root: None,
                primary_assignments: Vec::new(),
                alternate_assignments: Vec::new(),
                cross_body_assignment_cap: None,
                invitation_opened_at_height: None,
                invitation_close_height: None,
                accepted_assignments: BTreeSet::new(),
                declined_assignments: BTreeSet::new(),
            },
        );
        Ok(())
    }

    /// Consume one finalized future pulse and derive its complete assignment plans.
    ///
    /// # Errors
    /// Returns an error for replay, a wrong request/session/height binding, a
    /// duplicate pulse identifier or session-height slot, or a wrong attempt.
    pub fn consume_sortition_pulse_batch(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        request_ids: Vec<SortitionRequestId>,
        beacon_session_id: BeaconSessionId,
        pulse_height: u64,
        pulse_id: BeaconPulseId,
        pulse_output: [u8; 32],
        network_id: &NetworkId,
        governance: &Governance,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if pulse_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(ParliamentReducerErrorV1::DuplicateOrZeroIdentifier(
                ParliamentReducerEntityV1::SortitionRequest,
            ));
        }
        if request_ids.is_empty() || !request_ids.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
        }
        let expected_request_ids: Vec<_> = self
            .elections
            .values()
            .filter(|election| {
                election.attempt.status == BodyElectionAttemptStatusV1::AwaitingPulse
                    && election.attempt.request.beacon_session_id == beacon_session_id
                    && election.attempt.request.pulse_height == pulse_height
            })
            .map(|election| election.attempt.request.id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if request_ids != expected_request_ids {
            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
        }
        let mut election_ids = Vec::with_capacity(request_ids.len());
        let mut bodies = Vec::with_capacity(request_ids.len());
        let mut shared_candidate_snapshot: Option<Vec<AccountId>> = None;
        for request_id in &request_ids {
            let (election_id, election) = self
                .elections
                .iter()
                .find(|(_, state)| state.attempt.request.id == *request_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::SortitionRequest,
                ))?;
            let request = election.attempt.request;
            self.ensure_draw_eligible_body(request.body)?;
            if election.attempt.status != BodyElectionAttemptStatusV1::AwaitingPulse {
                return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                    ParliamentReducerEntityV1::BodyElection,
                ));
            }
            if request.governance_attempt_id != governance_attempt_id
                || request.beacon_session_id != beacon_session_id
                || request.pulse_height != pulse_height
            {
                return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
            }
            let configured_target = u32::try_from(body_committee_size(governance, request.body))
                .map_err(|_| ParliamentReducerErrorV1::InvalidAssignmentPlan)?;
            if request.target_seats != configured_target {
                return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
            }
            match &shared_candidate_snapshot {
                None => shared_candidate_snapshot = Some(election.candidate_snapshot.clone()),
                Some(expected) if expected == &election.candidate_snapshot => {}
                Some(_) => return Err(ParliamentReducerErrorV1::InvalidCandidateSnapshot),
            }
            election_ids.push(*election_id);
            bodies.push(request.body);
        }
        if self.used_pulse_ids.contains_key(&pulse_id)
            || self
                .used_pulse_slots
                .contains_key(&ParliamentPulseSlotV1::new(beacon_session_id, pulse_height))
        {
            return Err(ParliamentReducerErrorV1::BeaconPulseAlreadyConsumed);
        }
        bodies.sort_unstable();
        bodies.dedup();
        if bodies.len() != election_ids.len() {
            return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
        }
        let candidate_snapshot = shared_candidate_snapshot
            .as_deref()
            .ok_or(ParliamentReducerErrorV1::InvalidCandidateSnapshot)?;
        let plan = derive_attempt_body_plan_v1(
            governance,
            network_id,
            pulse_height,
            &pulse_output,
            candidate_snapshot,
            &bodies,
        );
        if plan.assignment_cap == 0 || plan.bodies.rosters.len() != bodies.len() {
            return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
        }

        struct DerivedElectionPlan {
            election_id: BodyElectionAttemptId,
            primary: Vec<ParliamentSeatAssignmentV1>,
            alternates: Vec<ParliamentSeatAssignmentV1>,
            assignment_root: [u8; 32],
        }
        let mut derived = Vec::with_capacity(election_ids.len());
        for election_id in &election_ids {
            let election = self
                .elections
                .get(election_id)
                .expect("election id came from this map");
            let request = election.attempt.request;
            let roster = plan
                .bodies
                .rosters
                .get(&request.body)
                .ok_or(ParliamentReducerErrorV1::InvalidAssignmentPlan)?;
            if roster.body != request.body
                || roster.epoch != pulse_height
                || roster.candidate_count != request.candidate_count
                || roster.members.is_empty()
                || u32::try_from(roster.members.len()).ok()
                    != Some(request.target_seats.min(request.candidate_count))
            {
                return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
            }
            let primary: Vec<_> = roster
                .members
                .iter()
                .cloned()
                .map(|member| ParliamentSeatAssignmentV1 {
                    assignment_id: AssignmentId::derive_v1(*election_id, &member),
                    member,
                })
                .collect();
            let alternates: Vec<_> = roster
                .alternates
                .iter()
                .cloned()
                .map(|member| ParliamentSeatAssignmentV1 {
                    assignment_id: AssignmentId::derive_v1(*election_id, &member),
                    member,
                })
                .collect();
            let invited: BTreeSet<_> = primary
                .iter()
                .chain(&alternates)
                .map(|assignment| assignment.member.clone())
                .collect();
            if invited.len() != primary.len() + alternates.len() {
                return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
            }
            let assignment_root = parliament_assignment_plan_root_v1(
                *election_id,
                &primary,
                &alternates,
                plan.assignment_cap,
            );
            if root_is_zero(&assignment_root) {
                return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
            }
            derived.push(DerivedElectionPlan {
                election_id: *election_id,
                primary,
                alternates,
                assignment_root,
            });
        }

        for derived_election in derived {
            let election = self
                .elections
                .get_mut(&derived_election.election_id)
                .expect("election id came from this map");
            election.attempt.status = BodyElectionAttemptStatusV1::Drawing;
            election.pulse_id = Some(pulse_id);
            election.pulse_output = Some(pulse_output);
            election.assignment_root = Some(derived_election.assignment_root);
            election.primary_assignments = derived_election.primary;
            election.alternate_assignments = derived_election.alternates;
            election.cross_body_assignment_cap = Some(plan.assignment_cap);
        }
        self.used_pulse_ids.insert(
            pulse_id,
            ParliamentPulseConsumerV1::SortitionBatch(request_ids),
        );
        self.used_pulse_slots.insert(
            ParliamentPulseSlotV1::new(beacon_session_id, pulse_height),
            pulse_id,
        );
        Ok(())
    }

    /// Open the immutable block-height window for invitation responses.
    ///
    /// # Errors
    /// Returns an error for a wrong attempt, unknown election, invalid height
    /// window, or any transition other than `Drawing -> AcceptingInvitations`.
    pub fn begin_invitation_acceptance(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        election_attempt_id: BodyElectionAttemptId,
        opened_at_height: u64,
        response_phase_blocks: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let invitation_close_height = opened_at_height
            .checked_add(
                response_phase_blocks
                    .checked_sub(1)
                    .ok_or(ParliamentReducerErrorV1::InvalidInvitationWindow)?,
            )
            .ok_or(ParliamentReducerErrorV1::InvalidInvitationWindow)?;
        let election = self.elections.get(&election_attempt_id).ok_or(
            ParliamentReducerErrorV1::UnknownEntity(ParliamentReducerEntityV1::BodyElection),
        )?;
        self.ensure_draw_eligible_body(election.attempt.request.body)?;
        if election.attempt.governance_attempt_id != governance_attempt_id {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        if election.attempt.status != BodyElectionAttemptStatusV1::Drawing
            || election.pulse_id.is_none()
            || election.pulse_output.is_none()
            || election.assignment_root.is_none()
            || election.primary_assignments.is_empty()
            || election.cross_body_assignment_cap.is_none()
            || opened_at_height < election.attempt.request.pulse_height
        {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyElection,
            ));
        }
        let election = self
            .elections
            .get_mut(&election_attempt_id)
            .expect("election checked above");
        election.invitation_opened_at_height = Some(opened_at_height);
        election.invitation_close_height = Some(invitation_close_height);
        election.attempt.status = BodyElectionAttemptStatusV1::AcceptingInvitations;
        Ok(())
    }

    /// Record one selected citizen's immutable invitation decision.
    ///
    /// The transaction authority is passed as `member`; callers cannot choose
    /// another assignment identifier. Both primaries and alternates respond up
    /// front so the final roster is a pure function of the ranked draw and the
    /// response transcript.
    ///
    /// # Errors
    /// Returns an error for a wrong attempt, unknown invitation, replay, wrong
    /// lifecycle, or a response after the committed close height.
    pub fn record_invitation_response(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        election_attempt_id: BodyElectionAttemptId,
        member: &AccountId,
        accept: bool,
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let election = self.elections.get(&election_attempt_id).ok_or(
            ParliamentReducerErrorV1::UnknownEntity(ParliamentReducerEntityV1::BodyElection),
        )?;
        self.ensure_draw_eligible_body(election.attempt.request.body)?;
        if election.attempt.governance_attempt_id != governance_attempt_id
            || election.attempt.status != BodyElectionAttemptStatusV1::AcceptingInvitations
        {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyElection,
            ));
        }
        let opened_at_height = election
            .invitation_opened_at_height
            .ok_or(ParliamentReducerErrorV1::InvalidInvitationWindow)?;
        let close_height = election
            .invitation_close_height
            .ok_or(ParliamentReducerErrorV1::InvalidInvitationWindow)?;
        if current_height < opened_at_height {
            return Err(ParliamentReducerErrorV1::InvalidInvitationWindow);
        }
        if current_height > close_height {
            return Err(ParliamentReducerErrorV1::InvitationWindowClosed);
        }
        let assignment_id = election
            .primary_assignments
            .iter()
            .chain(&election.alternate_assignments)
            .find_map(|assignment| {
                (&assignment.member == member).then_some(assignment.assignment_id)
            })
            .ok_or(ParliamentReducerErrorV1::UnknownInvitation)?;
        if election.accepted_assignments.contains(&assignment_id)
            || election.declined_assignments.contains(&assignment_id)
        {
            return Err(ParliamentReducerErrorV1::InvitationResponseReplay);
        }
        let election = self
            .elections
            .get_mut(&election_attempt_id)
            .expect("election checked above");
        if accept {
            election.accepted_assignments.insert(assignment_id);
        } else {
            election.declined_assignments.insert(assignment_id);
        }
        Ok(())
    }

    /// Mark an election unable to form any nonempty eligible roster.
    ///
    /// # Errors
    /// Returns an error for a wrong attempt, unknown election, or replay from a
    /// state other than drawing or invitation acceptance.
    pub fn fail_body_election_no_roster(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        election_attempt_id: BodyElectionAttemptId,
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let election = self.elections.get(&election_attempt_id).ok_or(
            ParliamentReducerErrorV1::UnknownEntity(ParliamentReducerEntityV1::BodyElection),
        )?;
        self.ensure_draw_eligible_body(election.attempt.request.body)?;
        if election.attempt.status != BodyElectionAttemptStatusV1::AcceptingInvitations {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyElection,
            ));
        }
        if election
            .invitation_close_height
            .is_none_or(|close_height| current_height <= close_height)
        {
            return Err(ParliamentReducerErrorV1::InvitationWindowStillOpen);
        }
        if !accepted_roster(election)?.is_empty() {
            return Err(ParliamentReducerErrorV1::InvalidRoster);
        }
        self.elections
            .get_mut(&election_attempt_id)
            .expect("election checked above")
            .attempt
            .status = BodyElectionAttemptStatusV1::NoRoster;
        Ok(())
    }

    /// Seal a nonempty canonical roster into a new body instance.
    ///
    /// Confirmation members must be disjoint from the completed Policy Jury.
    /// The sealed seat count becomes the immutable quorum denominator; later
    /// absence never changes it.
    ///
    /// # Errors
    /// Returns an error for wrong bindings or lifecycle, a malformed roster,
    /// duplicate identifiers, zero roots, or nonfresh confirmation membership.
    pub fn seal_body_roster(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        election_attempt_id: BodyElectionAttemptId,
        current_height: u64,
    ) -> Result<BodyInstanceId, ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let election = self.elections.get(&election_attempt_id).ok_or(
            ParliamentReducerErrorV1::UnknownEntity(ParliamentReducerEntityV1::BodyElection),
        )?;
        let request = election.attempt.request;
        self.ensure_draw_eligible_body(request.body)?;
        if election.attempt.governance_attempt_id != governance_attempt_id
            || election.attempt.status != BodyElectionAttemptStatusV1::AcceptingInvitations
            || election.assignment_root.is_none()
            || self.active_elections.get(&request.body) != Some(&election_attempt_id)
        {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyElection,
            ));
        }
        if election
            .invitation_close_height
            .is_none_or(|close_height| current_height <= close_height)
        {
            return Err(ParliamentReducerErrorV1::InvitationWindowStillOpen);
        }
        let assignments = accepted_roster(election)?;
        let assignment_count = u32::try_from(assignments.len())
            .map_err(|_| ParliamentReducerErrorV1::InvalidRoster)?;
        if assignment_count == 0 || assignment_count > request.target_seats {
            return Err(ParliamentReducerErrorV1::InvalidRoster);
        }
        if assignments.iter().any(|seat| {
            seat.assignment_id != AssignmentId::derive_v1(election_attempt_id, &seat.member)
        }) {
            return Err(ParliamentReducerErrorV1::InvalidRoster);
        }
        let roster_root = parliament_roster_root_v1(election_attempt_id, &assignments);
        let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
        if root_is_zero(&roster_root)
            || body_instance_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.bodies.contains_key(&body_instance_id)
        {
            return Err(ParliamentReducerErrorV1::DuplicateOrZeroIdentifier(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        let unique_members: BTreeSet<_> =
            assignments.iter().map(|seat| seat.member.clone()).collect();
        if unique_members.len() != assignments.len() {
            return Err(ParliamentReducerErrorV1::InvalidRoster);
        }
        if request.body == ParliamentBody::ConfirmationJury {
            let policy_members: BTreeSet<_> = self
                .bodies
                .values()
                .find(|body| body.instance.body == ParliamentBody::PolicyJury)
                .ok_or(ParliamentReducerErrorV1::ConfirmationJuryNotFresh)?
                .assignments
                .iter()
                .map(|seat| seat.member.clone())
                .collect();
            if !unique_members.is_disjoint(&policy_members) {
                return Err(ParliamentReducerErrorV1::ConfirmationJuryNotFresh);
            }
        }

        let body = ParliamentBodyInstanceV1 {
            id: body_instance_id,
            governance_attempt_id,
            election_attempt_id,
            body: request.body,
            target_seats: request.target_seats,
            original_seats: assignment_count,
            status: BodyInstanceStatusV1::RosterSealed,
        };
        self.elections
            .get_mut(&election_attempt_id)
            .expect("election checked above")
            .attempt
            .status = BodyElectionAttemptStatusV1::Sealed;
        self.active_bodies.insert(request.body, body_instance_id);
        self.bodies.insert(
            body_instance_id,
            ParliamentBodyStateV1 {
                instance: body,
                roster_root,
                assignments,
                excluded_assignments: BTreeSet::new(),
                result_root: None,
                result_height: None,
                ballot_binding: None,
            },
        );
        Ok(body_instance_id)
    }
}

impl ParliamentAttemptStateV1 {
    /// Advance a sealed body by exactly one deliberation phase.
    ///
    /// Public-finding bodies stop at reflection. Binding bodies alone may enter
    /// `Vote`, which can only be followed by private ballot registration.
    ///
    /// # Errors
    /// Returns an error for a wrong attempt/body/stage, an unknown instance,
    /// replay, phase skipping, phase reversal, or a decision-mode mismatch.
    pub fn advance_body_phase(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        target: DeliberationPhaseV1,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        let requirement = self.ensure_current_body(body.instance.body)?;
        if body.instance.governance_attempt_id != governance_attempt_id
            || self.active_bodies.get(&body.instance.body) != Some(&body_instance_id)
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        let expected = match body.instance.status {
            BodyInstanceStatusV1::RosterSealed => DeliberationPhaseV1::Orientation,
            BodyInstanceStatusV1::Deliberating(current) => next_deliberation_phase(current).ok_or(
                ParliamentReducerErrorV1::InvalidLifecycleTransition(
                    ParliamentReducerEntityV1::BodyInstance,
                ),
            )?,
            _ => {
                return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                    ParliamentReducerEntityV1::BodyInstance,
                ));
            }
        };
        if target != expected {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        if target == DeliberationPhaseV1::Vote
            && requirement.decision_mode != ParliamentDecisionModeV1::HiddenBindingBallot
        {
            return Err(ParliamentReducerErrorV1::DecisionModeMismatch);
        }
        self.bodies
            .get_mut(&body_instance_id)
            .expect("body checked above")
            .instance
            .status = BodyInstanceStatusV1::Deliberating(target);
        Ok(())
    }

    /// Exclude an absent seat from this attempt without changing its quorum denominator.
    ///
    /// The reducer records no slash, cooldown, or future-selection penalty. The
    /// same assignment cannot be excluded twice, and an exclusion cannot be
    /// introduced after balloting starts.
    ///
    /// # Errors
    /// Returns an error for wrong bindings, an unknown assignment, replay, or a
    /// body that has already entered or completed balloting.
    pub fn record_attempt_absence(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        assignment_id: AssignmentId,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        self.ensure_current_body(body.instance.body)?;
        if body.instance.governance_attempt_id != governance_attempt_id
            || !body
                .assignments
                .iter()
                .any(|seat| seat.assignment_id == assignment_id)
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        if !matches!(
            body.instance.status,
            BodyInstanceStatusV1::RosterSealed | BodyInstanceStatusV1::Deliberating(_)
        ) {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        let body = self
            .bodies
            .get_mut(&body_instance_id)
            .expect("body checked above");
        if !body.excluded_assignments.insert(assignment_id) {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        Ok(())
    }

    fn build_ballot_binding(
        &self,
        ballot_attempt_id: BallotAttemptId,
    ) -> Result<ParliamentBallotCertificateBindingV1, ParliamentReducerErrorV1> {
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
        if ballot.attempt.status != BallotAttemptStatusV1::Finalized {
            return Err(ParliamentReducerErrorV1::IncompleteCertificate);
        }
        Ok(ParliamentBallotCertificateBindingV1 {
            ballot_attempt_id,
            ballot_attempt_sequence: ballot.attempt.sequence,
            tle_session_id: ballot
                .tle_session_id
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            tle_key_session_id: ballot
                .tle_key_session_id
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            registration_root: ballot
                .registration_root
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            dropout_root: ballot
                .dropout_root
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            survivor_root: ballot
                .survivor_root
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            corpus_root: ballot
                .corpus_root
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            no_recovery_root: ballot
                .no_recovery_root
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            timed_commitment_root: ballot
                .timed_commitment_root
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            release_beacon_session_id: ballot
                .release_beacon_session_id
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            registered_at_height: ballot.registered_at_height,
            registration_close_height: ballot.registration_close_height,
            survivor_freeze_height: ballot.survivor_freeze_height,
            commitment_close_height: ballot.commitment_close_height,
            registration_closed_at_height: ballot
                .registration_closed_at_height
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            survivors_frozen_at_height: ballot
                .survivors_frozen_at_height
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            commitment_closed_at_height: ballot
                .commitment_closed_at_height
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            max_ballot_retries: ballot.max_ballot_retries,
            max_corpus_entries: ballot.max_corpus_entries,
            release_height: ballot
                .release_height
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            release_pulse_id: ballot
                .release_pulse_id
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            opening_height: ballot
                .opening_height
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            opening_root: ballot
                .opening_root
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            tally: ballot
                .tally
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
            outcome: ballot
                .outcome
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?,
        })
    }

    fn build_body_binding(
        &self,
        body_instance_id: BodyInstanceId,
    ) -> Result<ParliamentBodyCertificateBindingV1, ParliamentReducerErrorV1> {
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        let election = self
            .elections
            .get(&body.instance.election_attempt_id)
            .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                ParliamentReducerEntityV1::BodyElection,
            ))?;
        if election.attempt.status != BodyElectionAttemptStatusV1::Sealed
            || election.attempt.governance_attempt_id != self.attempt.id
            || election.attempt.request.governance_attempt_id != self.attempt.id
            || election.attempt.request.body != body.instance.body
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        let beacon_pulse_id = election
            .pulse_id
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let assignment_root = election
            .assignment_root
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let result_root = body
            .result_root
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let result_height = body
            .result_height
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let ballot = self
            .active_ballots
            .get(&body_instance_id)
            .copied()
            .map(|ballot_id| self.build_ballot_binding(ballot_id))
            .transpose()?;
        if body.ballot_binding != ballot {
            return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
        }
        Ok(ParliamentBodyCertificateBindingV1 {
            body_instance_id,
            election_attempt_id: body.instance.election_attempt_id,
            election_attempt_sequence: election.attempt.sequence,
            sortition_request_id: election.attempt.request.id,
            sortition_request: election.attempt.request,
            body: body.instance.body,
            beacon_session_id: election.attempt.request.beacon_session_id,
            beacon_pulse_id,
            roster_root: body.roster_root,
            assignment_root,
            result_root,
            result_height,
            ballot,
        })
    }

    /// Finalize a public, nonbinding body finding after reflection.
    ///
    /// # Errors
    /// Returns an error for a binding body, wrong stage/bindings, zero result
    /// root, replay, or a body that has not completed reflection.
    pub fn finalize_public_finding(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        result_root: [u8; 32],
        result_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if root_is_zero(&result_root) {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        let body_role = body.instance.body;
        let requirement = self.ensure_current_body(body_role)?;
        if requirement.decision_mode != ParliamentDecisionModeV1::PublicFinding {
            return Err(ParliamentReducerErrorV1::DecisionModeMismatch);
        }
        if body.instance.governance_attempt_id != governance_attempt_id
            || body.instance.status
                != BodyInstanceStatusV1::Deliberating(DeliberationPhaseV1::Reflection)
        {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        let election = self
            .elections
            .get(&body.instance.election_attempt_id)
            .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                ParliamentReducerEntityV1::BodyElection,
            ))?;
        if result_height <= election.attempt.request.pulse_height {
            return Err(ParliamentReducerErrorV1::InvalidCertificateHeight);
        }
        {
            let body = self
                .bodies
                .get_mut(&body_instance_id)
                .expect("body checked above");
            body.instance.status = BodyInstanceStatusV1::Approved;
            body.result_root = Some(result_root);
            body.result_height = Some(result_height);
        }
        let binding = self.build_body_binding(body_instance_id)?;
        if self.body_bindings.insert(body_role, binding).is_some() {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        self.advance_after_body(body_role)
    }
}

impl ParliamentAttemptStateV1 {
    /// Register a fresh private OVN ballot attempt for a body at `Vote`.
    ///
    /// A retry is accepted only after the preceding ballot reached `NoResult`.
    /// The old attempt is superseded and the new attempt must use the exact next
    /// sequence. No plaintext ballot input exists in this reducer API.
    ///
    /// # Errors
    /// Returns an error for a public-finding body, wrong stage/bindings, duplicate
    /// identifier, sequence mismatch, or an old ballot not in `NoResult`.
    #[expect(
        clippy::too_many_arguments,
        reason = "the ballot-specific TLE identity and target are immutable at registration"
    )]
    pub fn register_ballot_attempt(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        ballot_attempt_id: BallotAttemptId,
        sequence: u32,
        tle_session_id: TleSessionId,
        tle_key_session_id: TleKeySessionId,
        release_beacon_session_id: BeaconSessionId,
        registered_at_height: u64,
        timed_ovn_policy: ParliamentTimedOvn,
        release_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if ballot_attempt_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.ballots.contains_key(&ballot_attempt_id)
        {
            return Err(ParliamentReducerErrorV1::DuplicateOrZeroIdentifier(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        if tle_session_id.as_bytes().iter().all(|byte| *byte == 0)
            || tle_key_session_id.as_bytes().iter().all(|byte| *byte == 0)
            || release_beacon_session_id
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(ParliamentReducerErrorV1::DuplicateOrZeroIdentifier(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        let (
            registration_close_height,
            survivor_freeze_height,
            commitment_close_height,
            expected_release_height,
        ) = timed_ballot_schedule(registered_at_height, timed_ovn_policy)?;
        if release_height != expected_release_height {
            return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
        }
        if sequence > timed_ovn_policy.max_ballot_retries {
            return Err(ParliamentReducerErrorV1::BallotRetryLimitExceeded);
        }
        if self
            .ballots
            .values()
            .next()
            .is_some_and(|ballot| !ballot_policy_matches(ballot, timed_ovn_policy))
        {
            return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
        }
        if self.used_tle_sessions.contains_key(&tle_session_id) {
            return Err(ParliamentReducerErrorV1::TleSessionAlreadyConsumed);
        }
        if ballot_attempt_id != BallotAttemptId::derive_v1(body_instance_id, sequence)
            || tle_session_id
                != TleSessionId::derive_v1(
                    ballot_attempt_id,
                    tle_key_session_id,
                    release_beacon_session_id,
                    release_height,
                )
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        if self
            .used_pulse_slots
            .contains_key(&ParliamentPulseSlotV1::new(
                release_beacon_session_id,
                release_height,
            ))
        {
            return Err(ParliamentReducerErrorV1::BeaconPulseAlreadyConsumed);
        }
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        let body_role = body.instance.body;
        let requirement = self.ensure_current_body(body_role)?;
        if requirement.decision_mode != ParliamentDecisionModeV1::HiddenBindingBallot {
            return Err(ParliamentReducerErrorV1::DecisionModeMismatch);
        }
        if body.instance.governance_attempt_id != governance_attempt_id {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        let original_seats = body.instance.original_seats;
        let predecessor = self.active_ballots.get(&body_instance_id).copied();
        match predecessor {
            None => {
                if sequence != 0
                    || body.instance.status
                        != BodyInstanceStatusV1::Deliberating(DeliberationPhaseV1::Vote)
                {
                    return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
                }
            }
            Some(previous_id) => {
                let previous = self.ballots.get(&previous_id).ok_or(
                    ParliamentReducerErrorV1::UnknownEntity(
                        ParliamentReducerEntityV1::BallotAttempt,
                    ),
                )?;
                if previous.attempt.status != BallotAttemptStatusV1::NoResult
                    || body.instance.status != BodyInstanceStatusV1::NoResult
                {
                    return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                        ParliamentReducerEntityV1::BallotAttempt,
                    ));
                }
                if sequence != previous.attempt.sequence.saturating_add(1) {
                    return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
                }
            }
        }
        if let Some(previous_id) = predecessor {
            self.ballots
                .get_mut(&previous_id)
                .expect("predecessor checked above")
                .attempt
                .status = BallotAttemptStatusV1::Superseded;
        }
        self.bodies
            .get_mut(&body_instance_id)
            .expect("body checked above")
            .instance
            .status = BodyInstanceStatusV1::Balloting;
        self.active_ballots
            .insert(body_instance_id, ballot_attempt_id);
        self.ballots.insert(
            ballot_attempt_id,
            ParliamentBallotStateV1 {
                attempt: ParliamentBallotAttemptV1 {
                    id: ballot_attempt_id,
                    body_instance_id,
                    sequence,
                    original_seats,
                    status: BallotAttemptStatusV1::Registration,
                },
                registration_root: None,
                registered_voters: None,
                corpus_root: None,
                accepted_ballots: None,
                dropout_root: None,
                survivor_root: None,
                survivors: None,
                no_recovery_root: None,
                tle_session_id: Some(tle_session_id),
                tle_key_session_id: Some(tle_key_session_id),
                release_beacon_session_id: Some(release_beacon_session_id),
                registered_at_height,
                registration_phase_blocks: timed_ovn_policy.registration_phase_blocks,
                survivor_freeze_phase_blocks: timed_ovn_policy.survivor_freeze_phase_blocks,
                commitment_phase_blocks: timed_ovn_policy.commitment_phase_blocks,
                release_delay_blocks: timed_ovn_policy.release_delay_blocks,
                max_ballot_retries: timed_ovn_policy.max_ballot_retries,
                max_corpus_entries: timed_ovn_policy.max_corpus_entries,
                registration_close_height,
                survivor_freeze_height,
                commitment_close_height,
                registration_closed_at_height: None,
                survivors_frozen_at_height: None,
                commitment_closed_at_height: None,
                timed_commitment_root: None,
                release_height: Some(release_height),
                release_pulse_id: None,
                opening_height: None,
                opening_root: None,
                tally: None,
                outcome: None,
                failure_root: None,
                failure_kind: None,
                failure_height: None,
            },
        );
        self.used_tle_sessions
            .insert(tle_session_id, ballot_attempt_id);
        Ok(())
    }

    /// Close private registration and enter canonical survivor freezing.
    ///
    /// # Errors
    /// Returns an error for replay, zero root, wrong attempt, or a registered
    /// voter count exceeding nonabsent seats. The original-seat quorum is unchanged.
    pub fn close_ballot_registration(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        registration_root: [u8; 32],
        registered_voters: u32,
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if root_is_zero(&registration_root) {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
        if ballot.attempt.status != BallotAttemptStatusV1::Registration {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        if current_height != ballot.registration_close_height {
            return Err(ParliamentReducerErrorV1::WrongBallotPhaseHeight);
        }
        let body = self.bodies.get(&ballot.attempt.body_instance_id).ok_or(
            ParliamentReducerErrorV1::UnknownEntity(ParliamentReducerEntityV1::BodyInstance),
        )?;
        if body.instance.governance_attempt_id != governance_attempt_id
            || self.active_ballots.get(&body.instance.id) != Some(&ballot_attempt_id)
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        let excluded = u32::try_from(body.excluded_assignments.len())
            .map_err(|_| ParliamentReducerErrorV1::InvalidBallotCount)?;
        let eligible = ballot.attempt.original_seats.saturating_sub(excluded);
        if registered_voters > eligible || registered_voters > ballot.max_corpus_entries {
            return Err(ParliamentReducerErrorV1::InvalidBallotCount);
        }
        let ballot = self
            .ballots
            .get_mut(&ballot_attempt_id)
            .expect("ballot checked above");
        ballot.registration_root = Some(registration_root);
        ballot.registered_voters = Some(registered_voters);
        ballot.registration_closed_at_height = Some(current_height);
        ballot.attempt.status = BallotAttemptStatusV1::SurvivorFreeze;
        Ok(())
    }

    /// Freeze the canonical nonempty survivor roster before accepting any ballot.
    ///
    /// # Errors
    /// Returns an error for replay, zero roots, wrong attempt, an empty survivor
    /// set, or a survivor count exceeding the frozen registration.
    pub fn freeze_ballot_survivors(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        dropout_root: [u8; 32],
        survivor_root: [u8; 32],
        survivors: u32,
        no_recovery_root: [u8; 32],
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if root_is_zero(&dropout_root)
            || root_is_zero(&survivor_root)
            || root_is_zero(&no_recovery_root)
        {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
        if ballot.attempt.status != BallotAttemptStatusV1::SurvivorFreeze {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        if current_height != ballot.survivor_freeze_height
            || ballot.registration_closed_at_height != Some(ballot.registration_close_height)
        {
            return Err(ParliamentReducerErrorV1::WrongBallotPhaseHeight);
        }
        let registered = ballot
            .registered_voters
            .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
        if survivors == 0 || survivors > registered || survivors > ballot.max_corpus_entries {
            return Err(ParliamentReducerErrorV1::InvalidBallotCount);
        }
        let ballot = self
            .ballots
            .get_mut(&ballot_attempt_id)
            .expect("ballot checked above");
        ballot.dropout_root = Some(dropout_root);
        ballot.survivor_root = Some(survivor_root);
        ballot.survivors = Some(survivors);
        ballot.no_recovery_root = Some(no_recovery_root);
        ballot.survivors_frozen_at_height = Some(current_height);
        ballot.attempt.status = BallotAttemptStatusV1::TimedCommitment;
        Ok(())
    }

    /// Freeze the complete intrinsic timed-OVN ciphertext and one-hot-proof corpus.
    ///
    /// # Errors
    /// Returns an error for replay, survivor-root mutation, a missing survivor
    /// ballot, zero roots, unknown ballot, or wrong attempt.
    pub fn freeze_timed_ovn_corpus(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        corpus_root: [u8; 32],
        survivor_root: [u8; 32],
        accepted_ballots: u32,
        timed_commitment_root: [u8; 32],
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if root_is_zero(&corpus_root)
            || root_is_zero(&survivor_root)
            || root_is_zero(&timed_commitment_root)
        {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
        if ballot.attempt.status != BallotAttemptStatusV1::TimedCommitment {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        if current_height != ballot.commitment_close_height
            || ballot.survivors_frozen_at_height != Some(ballot.survivor_freeze_height)
        {
            return Err(ParliamentReducerErrorV1::WrongBallotPhaseHeight);
        }
        if ballot.survivor_root != Some(survivor_root) {
            return Err(ParliamentReducerErrorV1::AcceptedCorpusMutation);
        }
        if ballot.survivors != Some(accepted_ballots)
            || accepted_ballots > ballot.max_corpus_entries
        {
            return Err(ParliamentReducerErrorV1::InvalidBallotCount);
        }
        let ballot = self
            .ballots
            .get_mut(&ballot_attempt_id)
            .expect("ballot checked above");
        ballot.corpus_root = Some(corpus_root);
        ballot.accepted_ballots = Some(accepted_ballots);
        ballot.timed_commitment_root = Some(timed_commitment_root);
        ballot.commitment_closed_at_height = Some(current_height);
        ballot.attempt.status = BallotAttemptStatusV1::AwaitingRelease;
        Ok(())
    }

    /// Consume one finalized pulse for an exact canonical batch of timed openings.
    ///
    /// All awaiting ballots for the supplied session-height slot must be listed
    /// in strict identifier order. This permits legitimate simultaneous opening
    /// while rejecting subset, later, and cross-batch pulse reuse.
    ///
    /// # Errors
    /// Returns an error for an incomplete/noncanonical batch, early release,
    /// wrong binding, pulse reuse, wrong attempt, or replay.
    pub fn begin_ballot_opening_batch(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_ids: Vec<BallotAttemptId>,
        release_beacon_session_id: BeaconSessionId,
        release_height: u64,
        at_height: u64,
        pulse_id: BeaconPulseId,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if ballot_attempt_ids.is_empty()
            || !ballot_attempt_ids.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
        }
        if pulse_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(ParliamentReducerErrorV1::DuplicateOrZeroIdentifier(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        if at_height < release_height {
            return Err(ParliamentReducerErrorV1::ReleaseHeightNotReached);
        }
        let expected: Vec<_> = self
            .ballots
            .values()
            .filter(|ballot| {
                ballot.attempt.status == BallotAttemptStatusV1::AwaitingRelease
                    && ballot.release_beacon_session_id == Some(release_beacon_session_id)
                    && ballot.release_height == Some(release_height)
            })
            .map(|ballot| ballot.attempt.id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if ballot_attempt_ids != expected {
            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
        }
        for ballot_id in &ballot_attempt_ids {
            let ballot =
                self.ballots
                    .get(ballot_id)
                    .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                        ParliamentReducerEntityV1::BallotAttempt,
                    ))?;
            let body = self.bodies.get(&ballot.attempt.body_instance_id).ok_or(
                ParliamentReducerErrorV1::UnknownEntity(ParliamentReducerEntityV1::BodyInstance),
            )?;
            if body.instance.governance_attempt_id != governance_attempt_id
                || ballot.attempt.status != BallotAttemptStatusV1::AwaitingRelease
                || ballot.release_beacon_session_id != Some(release_beacon_session_id)
                || ballot.release_height != Some(release_height)
                || ballot.commitment_closed_at_height != Some(ballot.commitment_close_height)
            {
                return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
            }
        }
        if self.used_pulse_ids.contains_key(&pulse_id)
            || self
                .used_pulse_slots
                .contains_key(&ParliamentPulseSlotV1::new(
                    release_beacon_session_id,
                    release_height,
                ))
        {
            return Err(ParliamentReducerErrorV1::BeaconPulseAlreadyConsumed);
        }
        for ballot_id in &ballot_attempt_ids {
            let ballot = self
                .ballots
                .get_mut(ballot_id)
                .expect("ballot batch checked above");
            ballot.release_pulse_id = Some(pulse_id);
            ballot.opening_height = Some(at_height);
            ballot.attempt.status = BallotAttemptStatusV1::Opening;
        }
        self.used_pulse_ids.insert(
            pulse_id,
            ParliamentPulseConsumerV1::BallotBatch(ballot_attempt_ids),
        );
        self.used_pulse_slots.insert(
            ParliamentPulseSlotV1::new(release_beacon_session_id, release_height),
            pulse_id,
        );
        Ok(())
    }

    /// Mark a cryptographic ballot protocol failure as `NoResult`.
    ///
    /// There is no manual or plaintext fallback. A retry must register a fresh
    /// ballot attempt and, if it reaches timed sealing, a fresh TLE session.
    ///
    /// # Errors
    /// Returns an error for a zero failure root, unknown/wrong attempt, or a
    /// terminal/replayed ballot transition.
    pub fn fail_ballot_no_result(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        failure_kind: ParliamentBallotFailureKindV1,
        failure_root: [u8; 32],
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if root_is_zero(&failure_root) {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
        let failure_matches_phase = match (ballot.attempt.status, failure_kind) {
            (
                BallotAttemptStatusV1::Registration,
                ParliamentBallotFailureKindV1::RegistrationDeadlineExpired,
            ) => current_height > ballot.registration_close_height,
            (
                BallotAttemptStatusV1::SurvivorFreeze,
                ParliamentBallotFailureKindV1::SurvivorDeadlineExpired,
            ) => current_height > ballot.survivor_freeze_height,
            (
                BallotAttemptStatusV1::TimedCommitment,
                ParliamentBallotFailureKindV1::CommitmentDeadlineExpired,
            ) => current_height > ballot.commitment_close_height,
            (
                BallotAttemptStatusV1::AwaitingRelease,
                ParliamentBallotFailureKindV1::ReleasePulseUnavailable,
            ) => ballot
                .release_height
                .is_some_and(|release_height| current_height > release_height),
            (
                BallotAttemptStatusV1::Opening,
                ParliamentBallotFailureKindV1::AggregateOpeningFailed,
            ) => ballot
                .opening_height
                .is_some_and(|opening_height| current_height >= opening_height),
            _ => false,
        };
        if !failure_matches_phase {
            return Err(ParliamentReducerErrorV1::BallotFailureKindMismatch);
        }
        if matches!(
            ballot.attempt.status,
            BallotAttemptStatusV1::Finalized
                | BallotAttemptStatusV1::NoResult
                | BallotAttemptStatusV1::Superseded
        ) {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        let body_id = ballot.attempt.body_instance_id;
        let body = self
            .bodies
            .get(&body_id)
            .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                ParliamentReducerEntityV1::BodyInstance,
            ))?;
        if body.instance.governance_attempt_id != governance_attempt_id
            || self.active_ballots.get(&body_id) != Some(&ballot_attempt_id)
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        let ballot = self
            .ballots
            .get_mut(&ballot_attempt_id)
            .expect("ballot checked above");
        ballot.failure_root = Some(failure_root);
        ballot.failure_kind = Some(failure_kind);
        ballot.failure_height = Some(current_height);
        ballot.attempt.status = BallotAttemptStatusV1::NoResult;
        self.bodies
            .get_mut(&body_id)
            .expect("body checked above")
            .instance
            .status = BodyInstanceStatusV1::NoResult;
        Ok(())
    }
}

impl ParliamentAttemptStateV1 {
    /// Finalize a cryptographically opened aggregate and its body result.
    ///
    /// The accepted corpus root/count, recovery root, TLE session, original-seat
    /// denominator, and complete survivor opening are rechecked. An approved
    /// Policy Jury with a strictly sub-five-percent decisive margin dynamically
    /// requires a fresh, disjoint Confirmation Jury. Exactly five percent does
    /// not trigger confirmation.
    ///
    /// # Errors
    /// Returns an error for replay, wrong bindings, a mutated corpus, incomplete
    /// opening, malformed tally, zero roots, or wrong attempt/stage.
    #[expect(
        clippy::too_many_arguments,
        reason = "every final private-ballot binding is rechecked explicitly"
    )]
    pub fn finalize_opened_ballot(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        corpus_root: [u8; 32],
        no_recovery_root: [u8; 32],
        tle_session_id: TleSessionId,
        opening_root: [u8; 32],
        opened_survivors: u32,
        tally: ParliamentAggregateTallyV1,
        result_height: u64,
    ) -> Result<ParliamentAggregateOutcomeV1, ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if root_is_zero(&opening_root) {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
        if ballot.attempt.status != BallotAttemptStatusV1::Opening
            || ballot.release_pulse_id.is_none()
            || ballot.opening_height.is_none()
        {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        if ballot.corpus_root != Some(corpus_root)
            || ballot.no_recovery_root != Some(no_recovery_root)
            || ballot.tle_session_id != Some(tle_session_id)
            || tally.accepted_ballots != ballot.accepted_ballots.unwrap_or(u32::MAX)
        {
            return Err(ParliamentReducerErrorV1::AcceptedCorpusMutation);
        }
        if tally.original_seats != ballot.attempt.original_seats {
            return Err(ParliamentReducerErrorV1::InvalidBallotCount);
        }
        if ballot.survivors != Some(opened_survivors) {
            return Err(ParliamentReducerErrorV1::IncompleteOpening);
        }
        let opening_height = ballot
            .opening_height
            .ok_or(ParliamentReducerErrorV1::IncompleteOpening)?;
        let registration_root = ballot
            .registration_root
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let dropout_root = ballot
            .dropout_root
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let survivor_root = ballot
            .survivor_root
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let timed_commitment_root = ballot
            .timed_commitment_root
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let release_beacon_session_id = ballot
            .release_beacon_session_id
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let tle_key_session_id = ballot
            .tle_key_session_id
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let release_height = ballot
            .release_height
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let release_pulse_id = ballot
            .release_pulse_id
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let ballot_attempt_sequence = ballot.attempt.sequence;
        let registered_at_height = ballot.registered_at_height;
        let registration_close_height = ballot.registration_close_height;
        let survivor_freeze_height = ballot.survivor_freeze_height;
        let commitment_close_height = ballot.commitment_close_height;
        let registration_closed_at_height = ballot
            .registration_closed_at_height
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let survivors_frozen_at_height = ballot
            .survivors_frozen_at_height
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let commitment_closed_at_height = ballot
            .commitment_closed_at_height
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        let max_ballot_retries = ballot.max_ballot_retries;
        let max_corpus_entries = ballot.max_corpus_entries;
        if opening_height < release_height || result_height < opening_height {
            return Err(ParliamentReducerErrorV1::InvalidCertificateHeight);
        }
        tally
            .validate()
            .map_err(|_| ParliamentReducerErrorV1::InvalidTally)?;
        let outcome = tally
            .decision()
            .map_err(|_| ParliamentReducerErrorV1::InvalidTally)?;
        let body_instance_id = ballot.attempt.body_instance_id;
        let result_root = parliament_ballot_result_root_v1(
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            opening_root,
            tally,
            outcome,
            result_height,
        );
        if root_is_zero(&result_root) {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        let body_role = body.instance.body;
        self.ensure_current_body(body_role)?;
        if body.instance.governance_attempt_id != governance_attempt_id
            || body.instance.status != BodyInstanceStatusV1::Balloting
            || self.active_ballots.get(&body_instance_id) != Some(&ballot_attempt_id)
            || self.body_bindings.contains_key(&body_role)
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        let election = self
            .elections
            .get(&body.instance.election_attempt_id)
            .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                ParliamentReducerEntityV1::BodyElection,
            ))?;
        if result_height <= election.attempt.request.pulse_height {
            return Err(ParliamentReducerErrorV1::InvalidCertificateHeight);
        }
        let requires_confirmation = body_role == ParliamentBody::PolicyJury
            && tally
                .requires_confirmation()
                .map_err(|_| ParliamentReducerErrorV1::InvalidTally)?;
        if requires_confirmation
            && self
                .required_bodies
                .iter()
                .any(|entry| entry.body == ParliamentBody::ConfirmationJury)
        {
            return Err(ParliamentReducerErrorV1::InvalidRequiredBodyPipeline);
        }
        {
            let ballot = self
                .ballots
                .get_mut(&ballot_attempt_id)
                .expect("ballot checked above");
            ballot.opening_root = Some(opening_root);
            ballot.tally = Some(tally);
            ballot.outcome = Some(outcome);
            ballot.attempt.status = BallotAttemptStatusV1::Finalized;
        }
        let ballot_binding = ParliamentBallotCertificateBindingV1 {
            ballot_attempt_id,
            ballot_attempt_sequence,
            tle_session_id,
            tle_key_session_id,
            registration_root,
            dropout_root,
            survivor_root,
            corpus_root,
            no_recovery_root,
            timed_commitment_root,
            release_beacon_session_id,
            registered_at_height,
            registration_close_height,
            survivor_freeze_height,
            commitment_close_height,
            registration_closed_at_height,
            survivors_frozen_at_height,
            commitment_closed_at_height,
            max_ballot_retries,
            max_corpus_entries,
            release_height,
            release_pulse_id,
            opening_height,
            opening_root,
            tally,
            outcome,
        };
        {
            let body = self
                .bodies
                .get_mut(&body_instance_id)
                .expect("body checked above");
            body.result_root = Some(result_root);
            body.result_height = Some(result_height);
            body.ballot_binding = Some(ballot_binding);
            body.instance.status = match outcome {
                ParliamentAggregateOutcomeV1::Approved => BodyInstanceStatusV1::Approved,
                ParliamentAggregateOutcomeV1::Rejected => BodyInstanceStatusV1::Rejected,
                ParliamentAggregateOutcomeV1::NoQuorum => BodyInstanceStatusV1::NoQuorum,
                ParliamentAggregateOutcomeV1::NoResult => BodyInstanceStatusV1::NoResult,
            };
        }
        let binding = self.build_body_binding(body_instance_id)?;
        self.body_bindings.insert(body_role, binding);

        match outcome {
            ParliamentAggregateOutcomeV1::Approved => {
                if requires_confirmation {
                    self.required_bodies.push(RequiredParliamentBodyV1 {
                        body: ParliamentBody::ConfirmationJury,
                        decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
                    });
                }
                self.advance_after_body(body_role)?;
            }
            ParliamentAggregateOutcomeV1::Rejected
            | ParliamentAggregateOutcomeV1::NoQuorum
            | ParliamentAggregateOutcomeV1::NoResult => {
                self.attempt.status = GovernanceAttemptStatusV1::Rejected;
            }
        }
        Ok(outcome)
    }

    /// Construct and freeze the complete automatic governance certificate.
    ///
    /// The reducer supplies the exact ordered body bindings; callers cannot
    /// substitute a roster, pulse, corpus, TLE session, result root, proposal,
    /// policy, effect, or compare-and-set head.
    ///
    /// # Errors
    /// Returns an error unless every required body has a final consistent
    /// binding and enactment is strictly later than certification.
    pub fn construct_certificate(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        certified_at_height: u64,
        enact_at_height: u64,
    ) -> Result<GovernanceCertificateV1, ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        self.ensure_stage(GovernanceStageV1::Certification)?;
        if certified_at_height == 0 || enact_at_height <= certified_at_height {
            return Err(ParliamentReducerErrorV1::InvalidCertificateHeight);
        }
        if self.certificate.is_some() || self.body_bindings.len() != self.required_bodies.len() {
            return Err(ParliamentReducerErrorV1::IncompleteCertificate);
        }
        let mut body_bindings = Vec::with_capacity(self.required_bodies.len());
        for requirement in &self.required_bodies {
            let binding = self
                .body_bindings
                .get(&requirement.body)
                .copied()
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
            if binding.body != requirement.body {
                return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
            }
            match requirement.decision_mode {
                ParliamentDecisionModeV1::PublicFinding if binding.ballot.is_some() => {
                    return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                }
                ParliamentDecisionModeV1::HiddenBindingBallot if binding.ballot.is_none() => {
                    return Err(ParliamentReducerErrorV1::IncompleteCertificate);
                }
                _ => {}
            }
            let rebuilt = self.build_body_binding(binding.body_instance_id)?;
            if rebuilt != binding {
                return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
            }
            body_bindings.push(binding);
        }
        let certificate = GovernanceCertificateV1 {
            proposal_content_id: self.attempt.proposal_content_id,
            governance_attempt_id,
            governance_attempt_sequence: self.attempt.sequence,
            risk_tier: self.attempt.risk_tier,
            body_bindings,
            policy_version: self.policy_version,
            effect_preimage_hash: self.effect_preimage_hash,
            expected_head: self.expected_head,
            certified_at_height,
            enact_at_height,
        };
        certificate
            .validate()
            .map_err(|_| ParliamentReducerErrorV1::CertificateBindingMismatch)?;
        self.certificate = Some(certificate.clone());
        self.attempt.stage = GovernanceStageV1::Enactment;
        self.attempt.status = GovernanceAttemptStatusV1::Certified;
        Ok(certificate)
    }

    fn ensure_certified_for_execution(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        at_height: u64,
    ) -> Result<&GovernanceCertificateV1, ParliamentReducerErrorV1> {
        self.ensure_attempt(governance_attempt_id)?;
        if self.attempt.status != GovernanceAttemptStatusV1::Certified
            || self.attempt.stage != GovernanceStageV1::Enactment
        {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::Certificate,
            ));
        }
        let certificate = self
            .certificate
            .as_ref()
            .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
        if at_height < certificate.enact_at_height {
            return Err(ParliamentReducerErrorV1::ReleaseHeightNotReached);
        }
        Ok(certificate)
    }

    /// Mark a due certified effect enacted.
    ///
    /// # Errors
    /// Returns an error before the exact due height, for a wrong attempt, or for
    /// any replay/noncertified transition.
    pub fn mark_enacted(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        at_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_certified_for_execution(governance_attempt_id, at_height)?;
        self.attempt.status = GovernanceAttemptStatusV1::Enacted;
        self.terminal_height = Some(at_height);
        Ok(())
    }

    /// Mark a due certificate superseded by a different compare-and-set head.
    ///
    /// # Errors
    /// Returns an error for an unchanged head, early execution, wrong attempt,
    /// or any replay/noncertified transition.
    pub fn mark_superseded(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        at_height: u64,
        observed_head: GovernanceExpectedHeadV1,
    ) -> Result<(), ParliamentReducerErrorV1> {
        let certificate = self.ensure_certified_for_execution(governance_attempt_id, at_height)?;
        if observed_head == certificate.expected_head {
            return Err(ParliamentReducerErrorV1::ExpectedHeadUnchanged);
        }
        self.attempt.status = GovernanceAttemptStatusV1::Superseded;
        self.terminal_height = Some(at_height);
        self.superseding_head = Some(observed_head);
        Ok(())
    }

    /// Mark deterministic execution failed for the exact certified effect.
    ///
    /// # Errors
    /// Returns an error for effect substitution, zero failure root, early
    /// execution, wrong attempt, or any replay/noncertified transition.
    pub fn mark_execution_failed(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        at_height: u64,
        effect_preimage_hash: [u8; 32],
        failure_root: [u8; 32],
    ) -> Result<(), ParliamentReducerErrorV1> {
        if root_is_zero(&failure_root) {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        let certificate = self.ensure_certified_for_execution(governance_attempt_id, at_height)?;
        if effect_preimage_hash != certificate.effect_preimage_hash {
            return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
        }
        self.attempt.status = GovernanceAttemptStatusV1::ExecutionFailed;
        self.terminal_height = Some(at_height);
        self.execution_failure_root = Some(failure_root);
        Ok(())
    }
}

impl ParliamentAttemptStateV1 {
    /// Validate all cross-object bindings after decoding persisted reducer state.
    ///
    /// This audit is intentionally stricter than individual transition checks:
    /// it proves map keys, immutable identifiers, roots, lifecycle-dependent
    /// fields, consumed pulse batches, TLE ownership, roster denominators, body
    /// bindings, and any certificate are mutually consistent.
    ///
    /// # Errors
    /// Returns the first fail-closed invariant violation found.
    pub fn validate(&self) -> Result<(), ParliamentReducerErrorV1> {
        if self.attempt.id.as_bytes().iter().all(|byte| *byte == 0)
            || self
                .attempt
                .proposal_content_id
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
            || self.policy_version == 0
            || root_is_zero(&self.effect_preimage_hash)
            || !self.attempt.has_canonical_id()
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        if !persisted_pipeline_is_canonical(&self.required_bodies) {
            return Err(ParliamentReducerErrorV1::InvalidRequiredBodyPipeline);
        }
        let confirmation_required = self
            .required_bodies
            .last()
            .is_some_and(|entry| entry.body == ParliamentBody::ConfirmationJury);
        if self.risk_locked
            != self
                .elections
                .values()
                .any(|election| election.attempt.request.body == ParliamentBody::PolicyJury)
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }

        let mut request_ids = BTreeSet::new();
        let mut election_sequences =
            BTreeMap::<ParliamentBody, BTreeMap<u32, BodyElectionAttemptId>>::new();
        for (id, election) in &self.elections {
            let request = election.attempt.request;
            if *id != election.attempt.id
                || *id
                    != BodyElectionAttemptId::derive_v1(
                        self.attempt.id,
                        request.body,
                        election.attempt.sequence,
                    )
                || request.body_election_attempt_id != *id
                || request.governance_attempt_id != self.attempt.id
                || election.attempt.governance_attempt_id != self.attempt.id
                || !request_ids.insert(request.id)
                || root_is_zero(&request.candidate_root)
                || election.candidate_snapshot.is_empty()
                || !election
                    .candidate_snapshot
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                || u32::try_from(election.candidate_snapshot.len()).ok()
                    != Some(request.candidate_count)
                || request.candidate_root
                    != parliament_candidate_root_v1(
                        self.attempt.id,
                        request.body,
                        &election.candidate_snapshot,
                    )
            {
                return Err(ParliamentReducerErrorV1::InvalidCandidateSnapshot);
            }
            if election_sequences
                .entry(request.body)
                .or_default()
                .insert(election.attempt.sequence, *id)
                .is_some()
            {
                return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
            }
            request
                .validate(None)
                .map_err(|_| ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
            self.requirement_for_body(request.body)?;

            let invited_assignments: Vec<_> = election
                .primary_assignments
                .iter()
                .chain(&election.alternate_assignments)
                .collect();
            let invited_ids: BTreeSet<_> = invited_assignments
                .iter()
                .map(|assignment| assignment.assignment_id)
                .collect();
            let invited_members: BTreeSet<_> = invited_assignments
                .iter()
                .map(|assignment| assignment.member.clone())
                .collect();
            let responses_are_valid = election
                .accepted_assignments
                .is_disjoint(&election.declined_assignments)
                && election.accepted_assignments.is_subset(&invited_ids)
                && election.declined_assignments.is_subset(&invited_ids);
            let assignment_plan_is_valid = election
                .cross_body_assignment_cap
                .is_some_and(|cap| cap > 0)
                && !election.primary_assignments.is_empty()
                && u32::try_from(election.primary_assignments.len()).ok()
                    == Some(request.target_seats.min(request.candidate_count))
                && invited_ids.len() == invited_assignments.len()
                && invited_members.len() == invited_assignments.len()
                && invited_assignments.iter().all(|assignment| {
                    election
                        .candidate_snapshot
                        .binary_search(&assignment.member)
                        .is_ok()
                        && assignment.assignment_id
                            == AssignmentId::derive_v1(*id, &assignment.member)
                })
                && election.assignment_root.is_some_and(|assignment_root| {
                    !root_is_zero(&assignment_root)
                        && election.cross_body_assignment_cap.is_some_and(|cap| {
                            assignment_root
                                == parliament_assignment_plan_root_v1(
                                    *id,
                                    &election.primary_assignments,
                                    &election.alternate_assignments,
                                    cap,
                                )
                        })
                })
                && responses_are_valid;
            let invitation_window_is_valid = matches!(
                (
                    election.invitation_opened_at_height,
                    election.invitation_close_height,
                ),
                (Some(opened), Some(close)) if opened >= request.pulse_height && close >= opened
            );
            match election.attempt.status {
                BodyElectionAttemptStatusV1::AwaitingPulse => {
                    if election.pulse_id.is_some()
                        || election.pulse_output.is_some()
                        || election.assignment_root.is_some()
                        || !election.primary_assignments.is_empty()
                        || !election.alternate_assignments.is_empty()
                        || election.cross_body_assignment_cap.is_some()
                        || election.invitation_opened_at_height.is_some()
                        || election.invitation_close_height.is_some()
                        || !election.accepted_assignments.is_empty()
                        || !election.declined_assignments.is_empty()
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BodyElectionAttemptStatusV1::Drawing => {
                    if election.pulse_id.is_none()
                        || election.pulse_output.is_none()
                        || !assignment_plan_is_valid
                        || election.invitation_opened_at_height.is_some()
                        || election.invitation_close_height.is_some()
                        || !election.accepted_assignments.is_empty()
                        || !election.declined_assignments.is_empty()
                    {
                        return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
                    }
                }
                BodyElectionAttemptStatusV1::AcceptingInvitations
                | BodyElectionAttemptStatusV1::Sealed => {
                    if election.pulse_id.is_none()
                        || election.pulse_output.is_none()
                        || !assignment_plan_is_valid
                        || !invitation_window_is_valid
                    {
                        return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
                    }
                    if election.attempt.status == BodyElectionAttemptStatusV1::Sealed
                        && accepted_roster(election)?.is_empty()
                    {
                        return Err(ParliamentReducerErrorV1::InvalidRoster);
                    }
                }
                BodyElectionAttemptStatusV1::NoRoster | BodyElectionAttemptStatusV1::Superseded => {
                    if election.pulse_id.is_none()
                        || election.pulse_output.is_none()
                        || !assignment_plan_is_valid
                        || !invitation_window_is_valid
                        || !accepted_roster(election)?.is_empty()
                    {
                        return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
                    }
                }
            }
            if let Some(pulse_id) = election.pulse_id {
                let consumer = self
                    .used_pulse_ids
                    .get(&pulse_id)
                    .ok_or(ParliamentReducerErrorV1::PulseBindingMismatch)?;
                if !matches!(consumer, ParliamentPulseConsumerV1::SortitionBatch(batch) if batch.binary_search(&request.id).is_ok())
                    || self.used_pulse_slots.get(&ParliamentPulseSlotV1::new(
                        request.beacon_session_id,
                        request.pulse_height,
                    )) != Some(&pulse_id)
                {
                    return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                }
            }
        }
        if self.active_elections.len() != election_sequences.len() {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        for (body, sequences) in &election_sequences {
            if sequences
                .keys()
                .copied()
                .ne(0..u32::try_from(sequences.len())
                    .map_err(|_| ParliamentReducerErrorV1::RetrySequenceMismatch)?)
            {
                return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
            }
            let latest_id = sequences
                .last_key_value()
                .map(|(_, id)| *id)
                .ok_or(ParliamentReducerErrorV1::RetrySequenceMismatch)?;
            if self.active_elections.get(body) != Some(&latest_id) {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
            for (sequence, id) in sequences {
                let status = self
                    .elections
                    .get(id)
                    .ok_or(ParliamentReducerErrorV1::RetrySequenceMismatch)?
                    .attempt
                    .status;
                if (*id == latest_id && status == BodyElectionAttemptStatusV1::Superseded)
                    || (*id != latest_id && status != BodyElectionAttemptStatusV1::Superseded)
                    || *sequence
                        != self
                            .elections
                            .get(id)
                            .expect("sequence map came from elections")
                            .attempt
                            .sequence
                {
                    return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
                }
            }
        }
        for (body, active_id) in &self.active_elections {
            let active =
                self.elections
                    .get(active_id)
                    .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                        ParliamentReducerEntityV1::BodyElection,
                    ))?;
            if active.attempt.request.body != *body
                || active.attempt.status == BodyElectionAttemptStatusV1::Superseded
            {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
        }

        let mut all_members_by_body = BTreeMap::<ParliamentBody, BTreeSet<AccountId>>::new();
        for (id, body) in &self.bodies {
            if *id != body.instance.id
                || *id
                    != BodyInstanceId::derive_v1(
                        body.instance.election_attempt_id,
                        body.roster_root,
                    )
                || body.instance.governance_attempt_id != self.attempt.id
                || root_is_zero(&body.roster_root)
                || body.roster_root
                    != parliament_roster_root_v1(
                        body.instance.election_attempt_id,
                        &body.assignments,
                    )
                || usize::try_from(body.instance.original_seats).ok()
                    != Some(body.assignments.len())
                || body.instance.original_seats == 0
                || body.instance.original_seats > body.instance.target_seats
                || !body
                    .assignments
                    .windows(2)
                    .all(|pair| pair[0].assignment_id < pair[1].assignment_id)
            {
                return Err(ParliamentReducerErrorV1::InvalidRoster);
            }
            if body.assignments.iter().any(|seat| {
                seat.assignment_id
                    != AssignmentId::derive_v1(body.instance.election_attempt_id, &seat.member)
            }) {
                return Err(ParliamentReducerErrorV1::InvalidRoster);
            }
            let members: BTreeSet<_> = body
                .assignments
                .iter()
                .map(|seat| seat.member.clone())
                .collect();
            if members.len() != body.assignments.len()
                || !body.excluded_assignments.iter().all(|excluded| {
                    body.assignments
                        .iter()
                        .any(|seat| seat.assignment_id == *excluded)
                })
            {
                return Err(ParliamentReducerErrorV1::InvalidRoster);
            }
            let election = self
                .elections
                .get(&body.instance.election_attempt_id)
                .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
            if election.attempt.status != BodyElectionAttemptStatusV1::Sealed
                || election.attempt.request.body != body.instance.body
                || election.attempt.request.target_seats != body.instance.target_seats
                || accepted_roster(election)? != body.assignments
            {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
            if body.result_root.is_some() != body.result_height.is_some()
                || body
                    .result_height
                    .is_some_and(|height| height <= election.attempt.request.pulse_height)
            {
                return Err(ParliamentReducerErrorV1::InvalidCertificateHeight);
            }
            match body.instance.status {
                BodyInstanceStatusV1::Approved
                | BodyInstanceStatusV1::Rejected
                | BodyInstanceStatusV1::NoQuorum => {
                    if body.result_root.is_none() || body.result_height.is_none() {
                        return Err(ParliamentReducerErrorV1::IncompleteCertificate);
                    }
                }
                BodyInstanceStatusV1::AwaitingSortition
                | BodyInstanceStatusV1::AcceptingInvitations
                | BodyInstanceStatusV1::RosterSealed
                | BodyInstanceStatusV1::Deliberating(_)
                | BodyInstanceStatusV1::Balloting
                | BodyInstanceStatusV1::NoResult
                | BodyInstanceStatusV1::Superseded => {
                    if body.result_root.is_some() || body.result_height.is_some() {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
            }
            if all_members_by_body
                .insert(body.instance.body, members)
                .is_some()
            {
                return Err(ParliamentReducerErrorV1::InvalidRoster);
            }
        }
        if confirmation_required {
            let policy = all_members_by_body
                .get(&ParliamentBody::PolicyJury)
                .ok_or(ParliamentReducerErrorV1::ConfirmationJuryNotFresh)?;
            if let Some(confirmation) = all_members_by_body.get(&ParliamentBody::ConfirmationJury)
                && !policy.is_disjoint(confirmation)
            {
                return Err(ParliamentReducerErrorV1::ConfirmationJuryNotFresh);
            }
        }
        if self.active_bodies.len() != self.bodies.len() {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        for (body_role, active_id) in &self.active_bodies {
            let body =
                self.bodies
                    .get(active_id)
                    .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                        ParliamentReducerEntityV1::BodyInstance,
                    ))?;
            if body.instance.body != *body_role {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
        }

        let mut ballot_sequences =
            BTreeMap::<BodyInstanceId, BTreeMap<u32, BallotAttemptId>>::new();
        for (id, ballot) in &self.ballots {
            if *id != ballot.attempt.id
                || *id
                    != BallotAttemptId::derive_v1(
                        ballot.attempt.body_instance_id,
                        ballot.attempt.sequence,
                    )
            {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
            if ballot_sequences
                .entry(ballot.attempt.body_instance_id)
                .or_default()
                .insert(ballot.attempt.sequence, *id)
                .is_some()
            {
                return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
            }
            let body = self
                .bodies
                .get(&ballot.attempt.body_instance_id)
                .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
            if ballot.attempt.original_seats != body.instance.original_seats {
                return Err(ParliamentReducerErrorV1::InvalidBallotCount);
            }
            let tle_session_id = ballot
                .tle_session_id
                .ok_or(ParliamentReducerErrorV1::TleSessionAlreadyConsumed)?;
            let tle_key_session_id = ballot
                .tle_key_session_id
                .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
            let release_beacon_session_id = ballot
                .release_beacon_session_id
                .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
            let release_height = ballot
                .release_height
                .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
            if ballot.registered_at_height == 0
                || release_height <= ballot.registered_at_height
                || tle_session_id
                    != TleSessionId::derive_v1(
                        *id,
                        tle_key_session_id,
                        release_beacon_session_id,
                        release_height,
                    )
            {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
            if [
                ballot.registration_root,
                ballot.corpus_root,
                ballot.dropout_root,
                ballot.survivor_root,
                ballot.no_recovery_root,
                ballot.timed_commitment_root,
                ballot.opening_root,
                ballot.failure_root,
            ]
            .into_iter()
            .flatten()
            .any(|root| root_is_zero(&root))
            {
                return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
            }
            if let Some(registered) = ballot.registered_voters {
                let excluded = u32::try_from(body.excluded_assignments.len())
                    .map_err(|_| ParliamentReducerErrorV1::InvalidBallotCount)?;
                if registered > ballot.attempt.original_seats.saturating_sub(excluded) {
                    return Err(ParliamentReducerErrorV1::InvalidBallotCount);
                }
            }
            if let Some(accepted) = ballot.accepted_ballots {
                if accepted > ballot.registered_voters.unwrap_or(0)
                    || ballot.survivors != Some(accepted)
                {
                    return Err(ParliamentReducerErrorV1::InvalidBallotCount);
                }
            }
            if self.used_tle_sessions.get(&tle_session_id) != Some(id) {
                return Err(ParliamentReducerErrorV1::TleSessionAlreadyConsumed);
            }
            if let Some(pulse_id) = ballot.release_pulse_id {
                let consumer = self
                    .used_pulse_ids
                    .get(&pulse_id)
                    .ok_or(ParliamentReducerErrorV1::PulseBindingMismatch)?;
                if !matches!(consumer, ParliamentPulseConsumerV1::BallotBatch(batch) if batch.binary_search(id).is_ok())
                {
                    return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                }
                let session = ballot
                    .release_beacon_session_id
                    .ok_or(ParliamentReducerErrorV1::PulseBindingMismatch)?;
                let height = ballot
                    .release_height
                    .ok_or(ParliamentReducerErrorV1::PulseBindingMismatch)?;
                if self
                    .used_pulse_slots
                    .get(&ParliamentPulseSlotV1::new(session, height))
                    != Some(&pulse_id)
                {
                    return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                }
            }
            if matches!(
                ballot.attempt.status,
                BallotAttemptStatusV1::AwaitingRelease
                    | BallotAttemptStatusV1::Opening
                    | BallotAttemptStatusV1::Finalized
            ) && (ballot.registration_root.is_none()
                || ballot.registered_voters.is_none()
                || ballot.dropout_root.is_none()
                || ballot.survivor_root.is_none()
                || ballot.survivors.is_none()
                || ballot.no_recovery_root.is_none()
                || ballot.corpus_root.is_none()
                || ballot.accepted_ballots.is_none()
                || ballot.timed_commitment_root.is_none()
                || ballot.tle_session_id.is_none()
                || ballot.tle_key_session_id.is_none()
                || ballot.release_beacon_session_id.is_none()
                || ballot.release_height.is_none())
            {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
            if ballot.attempt.status != BallotAttemptStatusV1::Finalized
                && (ballot.tally.is_some() || ballot.outcome.is_some())
            {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
            match ballot.attempt.status {
                BallotAttemptStatusV1::Registration => {
                    if ballot.tle_session_id.is_none()
                        || ballot.tle_key_session_id.is_none()
                        || ballot.release_beacon_session_id.is_none()
                        || ballot.release_height.is_none()
                        || ballot.registration_root.is_some()
                        || ballot.registered_voters.is_some()
                        || ballot.dropout_root.is_some()
                        || ballot.survivor_root.is_some()
                        || ballot.survivors.is_some()
                        || ballot.no_recovery_root.is_some()
                        || ballot.corpus_root.is_some()
                        || ballot.accepted_ballots.is_some()
                        || ballot.timed_commitment_root.is_some()
                        || ballot.release_pulse_id.is_some()
                        || ballot.opening_height.is_some()
                        || ballot.opening_root.is_some()
                        || ballot.failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::SurvivorFreeze => {
                    if ballot.registration_root.is_none()
                        || ballot.registered_voters.is_none()
                        || ballot.dropout_root.is_some()
                        || ballot.survivor_root.is_some()
                        || ballot.survivors.is_some()
                        || ballot.no_recovery_root.is_some()
                        || ballot.corpus_root.is_some()
                        || ballot.accepted_ballots.is_some()
                        || ballot.timed_commitment_root.is_some()
                        || ballot.release_pulse_id.is_some()
                        || ballot.opening_height.is_some()
                        || ballot.opening_root.is_some()
                        || ballot.failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::TimedCommitment => {
                    if ballot.dropout_root.is_none()
                        || ballot.survivor_root.is_none()
                        || ballot.survivors.is_none()
                        || ballot.survivors == Some(0)
                        || ballot.no_recovery_root.is_none()
                        || ballot.corpus_root.is_some()
                        || ballot.accepted_ballots.is_some()
                        || ballot.timed_commitment_root.is_some()
                        || ballot.release_pulse_id.is_some()
                        || ballot.opening_height.is_some()
                        || ballot.opening_root.is_some()
                        || ballot.failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::AwaitingRelease => {
                    if ballot.corpus_root.is_none()
                        || ballot.accepted_ballots.is_none()
                        || ballot.timed_commitment_root.is_none()
                        || ballot.tle_session_id.is_none()
                        || ballot.tle_key_session_id.is_none()
                        || ballot.release_height.is_none()
                        || ballot.release_beacon_session_id.is_none()
                        || ballot.release_pulse_id.is_some()
                        || ballot.opening_height.is_some()
                        || ballot.opening_root.is_some()
                        || ballot.failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::Opening => {
                    if ballot.release_pulse_id.is_none()
                        || ballot.opening_height.is_none()
                        || ballot.opening_height < ballot.release_height
                        || ballot.opening_root.is_some()
                        || ballot.failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::Finalized => {
                    if ballot.opening_root.is_none()
                        || ballot.opening_height.is_none()
                        || ballot.opening_height < ballot.release_height
                        || body.ballot_binding.is_none()
                        || ballot.tally.is_none()
                        || ballot.outcome.is_none()
                        || ballot.failure_root.is_some()
                        || body.result_height.is_none_or(|height| {
                            ballot
                                .opening_height
                                .is_none_or(|opening_height| height < opening_height)
                        })
                    {
                        return Err(ParliamentReducerErrorV1::IncompleteCertificate);
                    }
                    let tally = ballot
                        .tally
                        .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
                    let outcome = ballot
                        .outcome
                        .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
                    if tally
                        .decision()
                        .map_err(|_| ParliamentReducerErrorV1::InvalidTally)?
                        != outcome
                        || self.build_ballot_binding(*id)?
                            != body
                                .ballot_binding
                                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::NoResult => {
                    if ballot.failure_root.is_none() {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::Superseded => {
                    if ballot.failure_root.is_none() {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
            }
        }
        if self.active_ballots.len() != ballot_sequences.len()
            || self.used_tle_sessions.len() != self.ballots.len()
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        for (body_id, sequences) in &ballot_sequences {
            if sequences
                .keys()
                .copied()
                .ne(0..u32::try_from(sequences.len())
                    .map_err(|_| ParliamentReducerErrorV1::RetrySequenceMismatch)?)
            {
                return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
            }
            let latest_id = sequences
                .last_key_value()
                .map(|(_, id)| *id)
                .ok_or(ParliamentReducerErrorV1::RetrySequenceMismatch)?;
            if self.active_ballots.get(body_id) != Some(&latest_id) {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
            for id in sequences.values() {
                let status = self
                    .ballots
                    .get(id)
                    .ok_or(ParliamentReducerErrorV1::RetrySequenceMismatch)?
                    .attempt
                    .status;
                if (*id == latest_id && status == BallotAttemptStatusV1::Superseded)
                    || (*id != latest_id && status != BallotAttemptStatusV1::Superseded)
                {
                    return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
                }
            }
        }
        for (body_id, active_id) in &self.active_ballots {
            let ballot =
                self.ballots
                    .get(active_id)
                    .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                        ParliamentReducerEntityV1::BallotAttempt,
                    ))?;
            if ballot.attempt.body_instance_id != *body_id
                || ballot.attempt.status == BallotAttemptStatusV1::Superseded
            {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
            }
        }
        for (tle_session_id, ballot_id) in &self.used_tle_sessions {
            let ballot =
                self.ballots
                    .get(ballot_id)
                    .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                        ParliamentReducerEntityV1::BallotAttempt,
                    ))?;
            if ballot.tle_session_id != Some(*tle_session_id) {
                return Err(ParliamentReducerErrorV1::TleSessionAlreadyConsumed);
            }
        }
        let unique_slot_pulses: BTreeSet<_> = self.used_pulse_slots.values().copied().collect();
        if self.used_pulse_ids.len() != self.used_pulse_slots.len()
            || unique_slot_pulses.len() != self.used_pulse_slots.len()
        {
            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
        }
        for (pulse_id, consumer) in &self.used_pulse_ids {
            if pulse_id.as_bytes().iter().all(|byte| *byte == 0) {
                return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
            }
            match consumer {
                ParliamentPulseConsumerV1::SortitionBatch(request_ids) => {
                    if request_ids.is_empty()
                        || !request_ids.windows(2).all(|pair| pair[0] < pair[1])
                    {
                        return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                    }
                    let mut slot = None;
                    let mut output = None;
                    let mut assignment_cap = None;
                    let mut candidate_snapshot: Option<&[AccountId]> = None;
                    for request_id in request_ids {
                        let election = self
                            .elections
                            .values()
                            .find(|election| election.attempt.request.id == *request_id)
                            .ok_or(ParliamentReducerErrorV1::PulseBindingMismatch)?;
                        if election.pulse_id != Some(*pulse_id) {
                            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                        }
                        let request_slot = ParliamentPulseSlotV1::new(
                            election.attempt.request.beacon_session_id,
                            election.attempt.request.pulse_height,
                        );
                        if slot.is_some_and(|expected| expected != request_slot) {
                            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                        }
                        if output.is_some_and(|expected| election.pulse_output != Some(expected))
                            || assignment_cap.is_some_and(|expected| {
                                election.cross_body_assignment_cap != Some(expected)
                            })
                            || candidate_snapshot.is_some_and(|expected| {
                                election.candidate_snapshot.as_slice() != expected
                            })
                        {
                            return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
                        }
                        slot = Some(request_slot);
                        output = election.pulse_output;
                        assignment_cap = election.cross_body_assignment_cap;
                        candidate_snapshot = Some(&election.candidate_snapshot);
                    }
                    if slot.is_none_or(|slot| self.used_pulse_slots.get(&slot) != Some(pulse_id))
                        || output.is_none()
                        || assignment_cap.is_none()
                        || candidate_snapshot.is_none()
                    {
                        return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                    }
                }
                ParliamentPulseConsumerV1::BallotBatch(ballot_ids) => {
                    if ballot_ids.is_empty() || !ballot_ids.windows(2).all(|pair| pair[0] < pair[1])
                    {
                        return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                    }
                    let mut slot = None;
                    for ballot_id in ballot_ids {
                        let ballot = self
                            .ballots
                            .get(ballot_id)
                            .ok_or(ParliamentReducerErrorV1::PulseBindingMismatch)?;
                        if ballot.release_pulse_id != Some(*pulse_id) {
                            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                        }
                        let ballot_slot = ParliamentPulseSlotV1::new(
                            ballot
                                .release_beacon_session_id
                                .ok_or(ParliamentReducerErrorV1::PulseBindingMismatch)?,
                            ballot
                                .release_height
                                .ok_or(ParliamentReducerErrorV1::PulseBindingMismatch)?,
                        );
                        if slot.is_some_and(|expected| expected != ballot_slot) {
                            return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                        }
                        slot = Some(ballot_slot);
                    }
                    if slot.is_none_or(|slot| self.used_pulse_slots.get(&slot) != Some(pulse_id)) {
                        return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
                    }
                }
            }
        }
        for pulse_id in self.used_pulse_slots.values() {
            if !self.used_pulse_ids.contains_key(pulse_id) {
                return Err(ParliamentReducerErrorV1::PulseBindingMismatch);
            }
        }

        for (body_role, binding) in &self.body_bindings {
            if binding.body != *body_role
                || self.build_body_binding(binding.body_instance_id)? != *binding
            {
                return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
            }
        }
        let policy_binding = self.body_bindings.get(&ParliamentBody::PolicyJury);
        if let Some(policy_ballot) = policy_binding.and_then(|binding| binding.ballot) {
            let requires_confirmation = policy_ballot
                .tally
                .requires_confirmation()
                .map_err(|_| ParliamentReducerErrorV1::InvalidTally)?;
            if requires_confirmation != confirmation_required {
                return Err(ParliamentReducerErrorV1::IncompleteCertificate);
            }
        }
        if let Some(certificate) = &self.certificate {
            certificate
                .validate()
                .map_err(|_| ParliamentReducerErrorV1::CertificateBindingMismatch)?;
            if certificate.proposal_content_id != self.attempt.proposal_content_id
                || certificate.governance_attempt_id != self.attempt.id
                || certificate.governance_attempt_sequence != self.attempt.sequence
                || certificate.risk_tier != self.attempt.risk_tier
                || certificate.policy_version != self.policy_version
                || certificate.effect_preimage_hash != self.effect_preimage_hash
                || certificate.expected_head != self.expected_head
                || certificate.certified_at_height == 0
                || certificate.enact_at_height <= certificate.certified_at_height
                || certificate.body_bindings.len() != self.required_bodies.len()
                || self.attempt.stage != GovernanceStageV1::Enactment
                || !matches!(
                    self.attempt.status,
                    GovernanceAttemptStatusV1::Certified
                        | GovernanceAttemptStatusV1::Enacted
                        | GovernanceAttemptStatusV1::Superseded
                        | GovernanceAttemptStatusV1::ExecutionFailed
                )
            {
                return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
            }
            for (requirement, binding) in
                self.required_bodies.iter().zip(&certificate.body_bindings)
            {
                if binding.body != requirement.body
                    || self.body_bindings.get(&requirement.body) != Some(binding)
                {
                    return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                }
            }
            match self.attempt.status {
                GovernanceAttemptStatusV1::Certified => {
                    if self.terminal_height.is_some()
                        || self.superseding_head.is_some()
                        || self.execution_failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                GovernanceAttemptStatusV1::Enacted => {
                    if self
                        .terminal_height
                        .is_none_or(|height| height < certificate.enact_at_height)
                        || self.superseding_head.is_some()
                        || self.execution_failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                GovernanceAttemptStatusV1::Superseded => {
                    if self
                        .terminal_height
                        .is_none_or(|height| height < certificate.enact_at_height)
                        || self
                            .superseding_head
                            .is_none_or(|head| head == certificate.expected_head)
                        || self.execution_failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                GovernanceAttemptStatusV1::ExecutionFailed => {
                    if self
                        .terminal_height
                        .is_none_or(|height| height < certificate.enact_at_height)
                        || self.superseding_head.is_some()
                        || self
                            .execution_failure_root
                            .is_none_or(|root| root_is_zero(&root))
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                GovernanceAttemptStatusV1::Active | GovernanceAttemptStatusV1::Rejected => {
                    return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                }
            }
        } else if matches!(
            self.attempt.status,
            GovernanceAttemptStatusV1::Certified
                | GovernanceAttemptStatusV1::Enacted
                | GovernanceAttemptStatusV1::Superseded
                | GovernanceAttemptStatusV1::ExecutionFailed
        ) {
            return Err(ParliamentReducerErrorV1::IncompleteCertificate);
        } else if self.terminal_height.is_some()
            || self.superseding_head.is_some()
            || self.execution_failure_root.is_some()
        {
            return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{block::BlockHeader, governance::types::GovernanceExpectedHeadAbsentV1};

    fn root(tag: u8) -> [u8; 32] {
        [tag.max(1); 32]
    }

    fn proposal_id(tag: u8) -> ProposalContentId {
        ProposalContentId::new(root(tag))
    }

    fn beacon_session(tag: u8) -> BeaconSessionId {
        BeaconSessionId::new(root(tag))
    }

    fn pulse_id(tag: u8) -> BeaconPulseId {
        BeaconPulseId::new(root(tag))
    }

    fn tle_key_session(tag: u8) -> TleKeySessionId {
        TleKeySessionId::new(root(tag))
    }

    fn account(tag: u8) -> AccountId {
        let key = KeyPair::try_from_seed(vec![tag.max(1); 32], Algorithm::Ed25519)
            .expect("derive Parliament reducer fixture key");
        AccountId::new(key.public_key().clone())
    }

    fn network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"parliament-reducer-fixture",
        )))
    }

    fn governance_for_pending_draws(state: &ParliamentAttemptStateV1) -> Governance {
        let mut governance = Governance {
            parliament_alternate_size: Some(16),
            ..Governance::default()
        };
        for election in state.elections.values().filter(|election| {
            election.attempt.status == BodyElectionAttemptStatusV1::AwaitingPulse
        }) {
            let target = usize::try_from(election.attempt.request.target_seats)
                .expect("fixture target fits usize");
            match election.attempt.request.body {
                ParliamentBody::RulesCommittee => governance.rules_committee_size = target,
                ParliamentBody::AgendaCouncil => governance.agenda_council_size = target,
                ParliamentBody::InterestPanel => governance.interest_panel_size = target,
                ParliamentBody::ReviewPanel => governance.review_panel_size = target,
                ParliamentBody::CoordinationCouncil => {
                    governance.coordination_council_size = target;
                }
                ParliamentBody::MpcCommittee => governance.mpc_committee_size = target,
                ParliamentBody::FmaCommittee => governance.fma_committee_size = target,
                ParliamentBody::OversightCommittee => {
                    governance.oversight_committee_size = target;
                }
                ParliamentBody::PolicyJury => governance.policy_jury_size = target,
                ParliamentBody::ConfirmationJury => governance.confirmation_jury_size = target,
            }
        }
        governance
    }

    fn consume_sortition(
        state: &mut ParliamentAttemptStateV1,
        governance_attempt_id: GovernanceAttemptId,
        request_ids: Vec<SortitionRequestId>,
        beacon_session_id: BeaconSessionId,
        pulse_height: u64,
        pulse_id: BeaconPulseId,
    ) -> Result<(), ParliamentReducerErrorV1> {
        let governance = governance_for_pending_draws(state);
        let pulse_output = *pulse_id.as_bytes();
        state.consume_sortition_pulse_batch(
            governance_attempt_id,
            request_ids,
            beacon_session_id,
            pulse_height,
            pulse_id,
            pulse_output,
            &network_id(),
            &governance,
        )
    }

    fn candidates(first_tag: u8, count: u32) -> Vec<AccountId> {
        let mut candidates: Vec<_> = (0..count)
            .map(|offset| {
                let tag = first_tag
                    .checked_add(u8::try_from(offset).expect("fixture candidate count fits u8"))
                    .expect("fixture candidate tags do not overflow");
                account(tag)
            })
            .collect();
        candidates.sort_unstable();
        candidates
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "test helper mirrors the immutable request preimage"
    )]
    fn sortition_request(
        governance_attempt_id: GovernanceAttemptId,
        sequence: u32,
        body: ParliamentBody,
        candidate_first_tag: u8,
        candidate_count: u32,
        target_seats: u32,
        request_height: u64,
        pulse_height: u64,
        beacon_session_id: BeaconSessionId,
        last_consumed_pulse_height: Option<u64>,
    ) -> (SortitionRequestV1, Vec<AccountId>) {
        let election_attempt_id =
            BodyElectionAttemptId::derive_v1(governance_attempt_id, body, sequence);
        let candidate_snapshot = candidates(candidate_first_tag, candidate_count);
        let candidate_root =
            parliament_candidate_root_v1(governance_attempt_id, body, &candidate_snapshot);
        let request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            body,
            candidate_root,
            candidate_count,
            target_seats,
            request_height,
            pulse_height,
            beacon_session_id,
            last_consumed_pulse_height,
        )
        .expect("canonical reducer sortition request");
        (request, candidate_snapshot)
    }

    fn attempt() -> GovernanceAttemptV1 {
        let proposal_content_id = proposal_id(2);
        GovernanceAttemptV1 {
            id: GovernanceAttemptId::derive_v1(proposal_content_id, 0),
            proposal_content_id,
            sequence: 0,
            risk_tier: RiskTierV1::Standard,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        }
    }

    fn state(required_bodies: Vec<RequiredParliamentBodyV1>) -> ParliamentAttemptStateV1 {
        ParliamentAttemptStateV1::try_new(
            attempt(),
            7,
            root(3),
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: root(4),
            }),
            required_bodies,
        )
        .expect("valid reducer fixture")
    }

    fn policy_only_state() -> ParliamentAttemptStateV1 {
        state(vec![RequiredParliamentBodyV1 {
            body: ParliamentBody::PolicyJury,
            decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
        }])
    }

    struct BodyFixture {
        state: ParliamentAttemptStateV1,
        body_id: BodyInstanceId,
        election_id: BodyElectionAttemptId,
        request_id: SortitionRequestId,
    }

    fn sealed_policy_body(seats: u32) -> BodyFixture {
        let mut state = policy_only_state();
        let attempt_id = state.attempt.id;
        state
            .complete_qualification(attempt_id)
            .expect("enter Policy Jury stage");
        let (request, candidate_snapshot) = sortition_request(
            attempt_id,
            0,
            ParliamentBody::PolicyJury,
            12,
            seats,
            seats,
            10,
            20,
            beacon_session(13),
            None,
        );
        let election_id = request.body_election_attempt_id;
        let request_id = request.id;
        state
            .register_sortition_request(attempt_id, 0, request, candidate_snapshot)
            .expect("register policy sortition");
        consume_sortition(
            &mut state,
            attempt_id,
            vec![request_id],
            beacon_session(13),
            20,
            pulse_id(14),
        )
        .expect("consume policy pulse");
        state
            .begin_invitation_acceptance(attempt_id, election_id, 20, 1)
            .expect("begin invitation acceptance");
        let selected: Vec<_> = state
            .election(&election_id)
            .expect("drawn election")
            .primary_assignments()
            .iter()
            .map(|assignment| assignment.member.clone())
            .collect();
        for member in selected {
            state
                .record_invitation_response(attempt_id, election_id, &member, true, 20)
                .expect("accept policy invitation");
        }
        let body_id = state
            .seal_body_roster(attempt_id, election_id, 21)
            .expect("seal policy roster");
        BodyFixture {
            state,
            body_id,
            election_id,
            request_id,
        }
    }

    fn advance_to_vote(state: &mut ParliamentAttemptStateV1, body_id: BodyInstanceId) {
        let attempt_id = state.attempt.id;
        for phase in [
            DeliberationPhaseV1::Orientation,
            DeliberationPhaseV1::Evidence,
            DeliberationPhaseV1::Questions,
            DeliberationPhaseV1::Responses,
            DeliberationPhaseV1::Deliberation,
            DeliberationPhaseV1::Reflection,
            DeliberationPhaseV1::Vote,
        ] {
            state
                .advance_body_phase(attempt_id, body_id, phase)
                .expect("advance one exact deliberation phase");
        }
    }

    struct OpeningFixture {
        state: ParliamentAttemptStateV1,
        body_id: BodyInstanceId,
        ballot_id: BallotAttemptId,
        tle_id: TleSessionId,
        accepted: u32,
    }

    fn opened_policy_ballot(seats: u32, accepted: u32) -> OpeningFixture {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(seats);
        advance_to_vote(&mut state, body_id);
        let attempt_id = state.attempt.id;
        let ballot_id = BallotAttemptId::derive_v1(body_id, 0);
        let release_beacon_session_id = beacon_session(24);
        let tle_key_session_id = tle_key_session(23);
        let release_height = 40;
        let tle_id = TleSessionId::derive_v1(
            ballot_id,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        );
        state
            .register_ballot_attempt(
                attempt_id,
                body_id,
                ballot_id,
                0,
                tle_id,
                tle_key_session_id,
                release_beacon_session_id,
                30,
                release_height,
            )
            .expect("register private ballot");
        state
            .close_ballot_registration(attempt_id, ballot_id, root(19), accepted)
            .expect("freeze registration");
        state
            .freeze_ballot_survivors(
                attempt_id,
                ballot_id,
                root(21),
                root(29),
                accepted,
                root(22),
            )
            .expect("freeze survivor roster");
        state
            .freeze_timed_ovn_corpus(
                attempt_id,
                ballot_id,
                root(20),
                root(29),
                accepted,
                root(25),
            )
            .expect("freeze complete timed OVN corpus");
        assert_eq!(
            state.begin_ballot_opening_batch(
                attempt_id,
                vec![ballot_id],
                beacon_session(24),
                40,
                39,
                pulse_id(26),
            ),
            Err(ParliamentReducerErrorV1::ReleaseHeightNotReached)
        );
        state
            .begin_ballot_opening_batch(
                attempt_id,
                vec![ballot_id],
                beacon_session(24),
                40,
                40,
                pulse_id(26),
            )
            .expect("begin timed opening");
        OpeningFixture {
            state,
            body_id,
            ballot_id,
            tle_id,
            accepted,
        }
    }

    fn finalize_policy(
        fixture: &mut OpeningFixture,
        aye: u32,
        nay: u32,
        abstain: u32,
    ) -> ParliamentAggregateOutcomeV1 {
        let attempt_id = fixture.state.attempt.id;
        fixture
            .state
            .finalize_opened_ballot(
                attempt_id,
                fixture.ballot_id,
                root(20),
                root(22),
                fixture.tle_id,
                root(27),
                fixture.accepted,
                ParliamentAggregateTallyV1 {
                    original_seats: fixture
                        .state
                        .body(&fixture.body_id)
                        .expect("fixture body")
                        .instance
                        .original_seats,
                    accepted_ballots: fixture.accepted,
                    aye,
                    nay,
                    abstain,
                },
                root(28),
                41,
            )
            .expect("finalize policy aggregate")
    }

    #[test]
    fn risk_only_escalates_and_policy_request_locks_it() {
        let mut state = policy_only_state();
        let id = state.attempt.id;
        assert_eq!(
            state.escalate_risk(id, RiskTierV1::Routine),
            Err(ParliamentReducerErrorV1::RiskDowngrade)
        );
        assert_eq!(
            state.escalate_risk(id, RiskTierV1::Standard),
            Err(ParliamentReducerErrorV1::RiskEscalationReplay)
        );
        state
            .escalate_risk(id, RiskTierV1::Constitutional)
            .expect("upward escalation succeeds");
        let (request, candidate_snapshot) = sortition_request(
            id,
            0,
            ParliamentBody::PolicyJury,
            12,
            3,
            3,
            10,
            20,
            beacon_session(13),
            None,
        );
        state
            .register_sortition_request(id, 0, request, candidate_snapshot)
            .expect("Policy Jury request locks risk");
        assert_eq!(
            state.escalate_risk(id, RiskTierV1::Emergency),
            Err(ParliamentReducerErrorV1::RiskTierLocked)
        );
    }

    #[test]
    fn simultaneous_sortition_consumes_one_exact_canonical_batch() {
        let mut state = state(vec![
            RequiredParliamentBodyV1 {
                body: ParliamentBody::InterestPanel,
                decision_mode: ParliamentDecisionModeV1::PublicFinding,
            },
            RequiredParliamentBodyV1 {
                body: ParliamentBody::PolicyJury,
                decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
            },
        ]);
        let id = state.attempt.id;
        state.complete_qualification(id).expect("enter interest");
        let mut request_ids = Vec::new();
        for body in [ParliamentBody::InterestPanel, ParliamentBody::PolicyJury] {
            let (request, candidate_snapshot) = sortition_request(
                id,
                0,
                body,
                12,
                3,
                3,
                10,
                20,
                beacon_session(30),
                None,
            );
            request_ids.push(request.id);
            state
                .register_sortition_request(id, 0, request, candidate_snapshot)
                .expect("register simultaneous request");
        }
        request_ids.sort_unstable();
        assert_eq!(
            consume_sortition(
                &mut state,
                id,
                vec![request_ids[0]],
                beacon_session(30),
                20,
                pulse_id(31),
            ),
            Err(ParliamentReducerErrorV1::PulseBindingMismatch)
        );
        consume_sortition(
            &mut state,
            id,
            request_ids,
            beacon_session(30),
            20,
            pulse_id(31),
        )
        .expect("consume complete canonical batch");
        assert!(
            state.elections.values().all(|election| {
                election.attempt.status == BodyElectionAttemptStatusV1::Drawing
            })
        );
        assert!(state.validate().is_ok());
    }

    #[test]
    fn invitation_responses_seal_only_the_ranked_accepted_roster() {
        let mut state = policy_only_state();
        let id = state.attempt.id;
        state
            .complete_qualification(id)
            .expect("enter Policy Jury stage");
        let (request, candidates) = sortition_request(
            id,
            0,
            ParliamentBody::PolicyJury,
            70,
            5,
            2,
            10,
            20,
            beacon_session(71),
            None,
        );
        let election_id = request.body_election_attempt_id;
        let request_id = request.id;
        state
            .register_sortition_request(id, 0, request, candidates)
            .expect("register invitation test election");
        consume_sortition(
            &mut state,
            id,
            vec![request_id],
            beacon_session(71),
            20,
            pulse_id(72),
        )
        .expect("derive ranked invitation plan");
        state
            .begin_invitation_acceptance(id, election_id, 20, 2)
            .expect("open two-block invitation window");
        let election = state.election(&election_id).expect("drawn election");
        let first_primary = election.primary_assignments()[0].clone();
        let second_primary = election.primary_assignments()[1].clone();
        let first_alternate = election.alternate_assignments()[0].clone();
        let late_alternate = election.alternate_assignments()[1].clone();

        state
            .record_invitation_response(id, election_id, &first_primary.member, true, 20)
            .expect("first primary accepts");
        assert_eq!(
            state.record_invitation_response(id, election_id, &first_primary.member, false, 20),
            Err(ParliamentReducerErrorV1::InvitationResponseReplay)
        );
        state
            .record_invitation_response(id, election_id, &second_primary.member, false, 21)
            .expect("second primary declines");
        state
            .record_invitation_response(id, election_id, &first_alternate.member, true, 21)
            .expect("first ranked alternate accepts");
        assert_eq!(
            state.record_invitation_response(id, election_id, &late_alternate.member, true, 22),
            Err(ParliamentReducerErrorV1::InvitationWindowClosed)
        );
        assert_eq!(
            state.seal_body_roster(id, election_id, 21),
            Err(ParliamentReducerErrorV1::InvitationWindowStillOpen)
        );
        let body_id = state
            .seal_body_roster(id, election_id, 22)
            .expect("seal derived accepted roster after close");
        let body = state.body(&body_id).expect("sealed body");
        let expected_members: BTreeSet<_> = [first_primary.member, first_alternate.member]
            .into_iter()
            .collect();
        assert_eq!(
            body.assignments()
                .iter()
                .map(|assignment| assignment.member.clone())
                .collect::<BTreeSet<_>>(),
            expected_members
        );
        assert!(state.validate().is_ok());
    }

    #[test]
    fn election_retry_supersedes_only_no_roster_and_rejects_pulse_reuse() {
        let BodyFixture {
            mut state,
            election_id: first_election,
            request_id: first_request,
            ..
        } = sealed_policy_body(3);
        let id = state.attempt.id;
        assert_eq!(
            state.fail_body_election_no_roster(id, first_election, 22),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyElection
            ))
        );
        assert_eq!(
            consume_sortition(
                &mut state,
                id,
                vec![first_request],
                beacon_session(13),
                20,
                pulse_id(14),
            ),
            Err(ParliamentReducerErrorV1::PulseBindingMismatch)
        );

        let mut state = policy_only_state();
        state.complete_qualification(id).expect("enter policy");
        let (first, first_candidates) = sortition_request(
            id,
            0,
            ParliamentBody::PolicyJury,
            12,
            3,
            3,
            10,
            20,
            beacon_session(13),
            None,
        );
        let first_request_id = first.id;
        let first_election_id = first.body_election_attempt_id;
        state
            .register_sortition_request(id, 0, first, first_candidates)
            .expect("register first election");
        consume_sortition(
            &mut state,
            id,
            vec![first_request_id],
            beacon_session(13),
            20,
            pulse_id(14),
        )
        .expect("consume first pulse");
        state
            .begin_invitation_acceptance(id, first_election_id, 20, 1)
            .expect("begin first invitation window");
        let invited: Vec<_> = state
            .election(&first_election_id)
            .expect("drawn first election")
            .primary_assignments()
            .iter()
            .chain(
                state
                    .election(&first_election_id)
                    .expect("drawn first election")
                    .alternate_assignments(),
            )
            .map(|assignment| assignment.member.clone())
            .collect();
        for member in invited {
            state
                .record_invitation_response(id, first_election_id, &member, false, 20)
                .expect("decline first election invitation");
        }
        state
            .fail_body_election_no_roster(id, first_election_id, 21)
            .expect("record no roster");
        let (retry, retry_candidates) = sortition_request(
            id,
            1,
            ParliamentBody::PolicyJury,
            17,
            3,
            3,
            21,
            30,
            beacon_session(13),
            Some(20),
        );
        let retry_request_id = retry.id;
        state
            .register_sortition_request(id, 1, retry, retry_candidates)
            .expect("register exact retry");
        assert_eq!(
            state
                .election(&first_election_id)
                .expect("old election")
                .attempt
                .status,
            BodyElectionAttemptStatusV1::Superseded
        );
        assert_eq!(
            consume_sortition(
                &mut state,
                id,
                vec![retry_request_id],
                beacon_session(13),
                30,
                pulse_id(14),
            ),
            Err(ParliamentReducerErrorV1::BeaconPulseAlreadyConsumed)
        );
    }

    #[test]
    fn body_phase_transition_table_rejects_skip_replay_and_reverse() {
        let BodyFixture { state, body_id, .. } = sealed_policy_body(3);
        let id = state.attempt.id;
        let phases = [
            DeliberationPhaseV1::Orientation,
            DeliberationPhaseV1::Evidence,
            DeliberationPhaseV1::Questions,
            DeliberationPhaseV1::Responses,
            DeliberationPhaseV1::Deliberation,
            DeliberationPhaseV1::Reflection,
            DeliberationPhaseV1::Vote,
        ];
        let mut cursor = state;
        for (index, expected) in phases.into_iter().enumerate() {
            for candidate in phases {
                let mut probe = cursor.clone();
                let result = probe.advance_body_phase(id, body_id, candidate);
                assert_eq!(
                    result.is_ok(),
                    candidate == expected,
                    "phase row {index:?}, candidate {candidate:?}"
                );
            }
            cursor
                .advance_body_phase(id, body_id, expected)
                .expect("exact next phase succeeds");
        }
        assert_eq!(
            cursor.advance_body_phase(id, body_id, DeliberationPhaseV1::Vote),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance
            ))
        );
    }

    #[test]
    fn absence_is_attempt_local_and_never_changes_original_quorum() {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        let id = state.attempt.id;
        let absent = state
            .body(&body_id)
            .expect("body")
            .assignments()
            .first()
            .expect("fixture has a seat")
            .assignment_id;
        state
            .record_attempt_absence(id, body_id, absent)
            .expect("record first absence");
        assert_eq!(
            state.record_attempt_absence(id, body_id, absent),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance
            ))
        );
        assert_eq!(
            state.body(&body_id).expect("body").instance.original_seats,
            3
        );
        advance_to_vote(&mut state, body_id);
        let ballot = BallotAttemptId::derive_v1(body_id, 0);
        let release_beacon_session_id = beacon_session(53);
        let tle_key_session_id = tle_key_session(52);
        let release_height = 40;
        let tle_session_id = TleSessionId::derive_v1(
            ballot,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        );
        state
            .register_ballot_attempt(
                id,
                body_id,
                ballot,
                0,
                tle_session_id,
                tle_key_session_id,
                release_beacon_session_id,
                30,
                release_height,
            )
            .expect("register ballot");
        assert_eq!(
            state.close_ballot_registration(id, ballot, root(51), 3),
            Err(ParliamentReducerErrorV1::InvalidBallotCount)
        );
        state
            .close_ballot_registration(id, ballot, root(51), 2)
            .expect("only nonabsent seats register");
        assert_eq!(
            state
                .ballot(&ballot)
                .expect("ballot")
                .attempt
                .original_seats,
            3
        );
    }

    #[test]
    fn ballot_transition_table_freezes_corpus_and_retries_without_fallback() {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        advance_to_vote(&mut state, body_id);
        let id = state.attempt.id;
        let ballot = BallotAttemptId::derive_v1(body_id, 0);
        let first_release_beacon_session_id = beacon_session(24);
        let first_tle_key_session_id = tle_key_session(23);
        let first_release_height = 40;
        let first_tle_session_id = TleSessionId::derive_v1(
            ballot,
            first_tle_key_session_id,
            first_release_beacon_session_id,
            first_release_height,
        );
        state
            .register_ballot_attempt(
                id,
                body_id,
                ballot,
                0,
                first_tle_session_id,
                first_tle_key_session_id,
                first_release_beacon_session_id,
                30,
                first_release_height,
            )
            .expect("registration");
        assert_eq!(
            state.freeze_ballot_survivors(id, ballot, root(21), root(29), 3, root(22)),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
        );
        state
            .close_ballot_registration(id, ballot, root(19), 3)
            .expect("commitment");
        assert_eq!(
            state.close_ballot_registration(id, ballot, root(19), 3),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
        );
        state
            .freeze_ballot_survivors(id, ballot, root(21), root(29), 2, root(22))
            .expect("freeze nonempty survivor roster");
        assert_eq!(
            state.freeze_timed_ovn_corpus(id, ballot, root(20), root(28), 2, root(25)),
            Err(ParliamentReducerErrorV1::AcceptedCorpusMutation)
        );
        state
            .freeze_timed_ovn_corpus(id, ballot, root(20), root(29), 2, root(25))
            .expect("freeze complete intrinsic timed OVN corpus");
        assert_eq!(
            state.finalize_opened_ballot(
                id,
                ballot,
                root(20),
                root(22),
                first_tle_session_id,
                root(26),
                2,
                ParliamentAggregateTallyV1 {
                    original_seats: 3,
                    accepted_ballots: 2,
                    aye: 1,
                    nay: 1,
                    abstain: 0,
                },
                root(27),
                41,
            ),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
        );
        state
            .fail_ballot_no_result(id, ballot, root(30))
            .expect("pulse/TLE failure is NoResult");
        let retry = BallotAttemptId::derive_v1(body_id, 1);
        assert_eq!(
            state.register_ballot_attempt(
                id,
                body_id,
                retry,
                1,
                first_tle_session_id,
                tle_key_session(31),
                beacon_session(32),
                41,
                50,
            ),
            Err(ParliamentReducerErrorV1::TleSessionAlreadyConsumed)
        );
        let retry_release_beacon_session_id = beacon_session(33);
        let retry_tle_key_session_id = tle_key_session(32);
        let retry_release_height = 50;
        let retry_tle_session_id = TleSessionId::derive_v1(
            retry,
            retry_tle_key_session_id,
            retry_release_beacon_session_id,
            retry_release_height,
        );
        state
            .register_ballot_attempt(
                id,
                body_id,
                retry,
                1,
                retry_tle_session_id,
                retry_tle_key_session_id,
                retry_release_beacon_session_id,
                41,
                retry_release_height,
            )
            .expect("fresh attempt retries");
        assert_eq!(
            state.ballot(&ballot).expect("old ballot").attempt.status,
            BallotAttemptStatusV1::Superseded
        );
        assert_eq!(
            state
                .ballot(&retry)
                .expect("retry ballot")
                .attempt
                .original_seats,
            3
        );
    }

    #[test]
    fn policy_margin_is_strict_and_confirmation_roster_is_fresh() {
        let mut narrow = opened_policy_ballot(100, 100);
        assert_eq!(
            finalize_policy(&mut narrow, 51, 49, 0),
            ParliamentAggregateOutcomeV1::Approved
        );
        assert_eq!(
            narrow.state.attempt.stage,
            GovernanceStageV1::ConfirmationJury
        );
        assert_eq!(
            narrow.state.required_bodies.last(),
            Some(&RequiredParliamentBodyV1 {
                body: ParliamentBody::ConfirmationJury,
                decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
            })
        );
        let id = narrow.state.attempt.id;
        let policy_members: Vec<_> = narrow
            .state
            .bodies
            .values()
            .find(|body| body.instance.body == ParliamentBody::PolicyJury)
            .expect("completed policy body")
            .assignments
            .iter()
            .take(3)
            .map(|assignment| assignment.member.clone())
            .collect();
        let mut overlapping_candidates = policy_members.clone();
        overlapping_candidates.sort_unstable();
        let overlapping_election_id =
            BodyElectionAttemptId::derive_v1(id, ParliamentBody::ConfirmationJury, 0);
        let overlapping_request = SortitionRequestV1::try_new_canonical(
            id,
            overlapping_election_id,
            ParliamentBody::ConfirmationJury,
            parliament_candidate_root_v1(
                id,
                ParliamentBody::ConfirmationJury,
                &overlapping_candidates,
            ),
            u32::try_from(overlapping_candidates.len()).expect("fixture candidate count"),
            3,
            50,
            60,
            beacon_session(104),
            None,
        )
        .expect("canonical overlapping confirmation request");
        assert_eq!(
            narrow.state.register_sortition_request(
                id,
                0,
                overlapping_request,
                overlapping_candidates,
            ),
            Err(ParliamentReducerErrorV1::ConfirmationJuryNotFresh)
        );
        let (request, candidate_snapshot) = sortition_request(
            id,
            0,
            ParliamentBody::ConfirmationJury,
            103,
            3,
            3,
            50,
            60,
            beacon_session(104),
            None,
        );
        let confirmation_request_id = request.id;
        let confirmation_election_id = request.body_election_attempt_id;
        narrow
            .state
            .register_sortition_request(id, 0, request, candidate_snapshot)
            .expect("register fresh confirmation draw");
        consume_sortition(
            &mut narrow.state,
            id,
            vec![confirmation_request_id],
            beacon_session(104),
            60,
            pulse_id(105),
        )
        .expect("consume confirmation pulse");
        narrow
            .state
            .begin_invitation_acceptance(id, confirmation_election_id, 60, 1)
            .expect("confirmation invitations");
        let confirmation_members: Vec<_> = narrow
            .state
            .election(&confirmation_election_id)
            .expect("drawn confirmation election")
            .primary_assignments()
            .iter()
            .map(|assignment| assignment.member.clone())
            .collect();
        for member in confirmation_members {
            narrow
                .state
                .record_invitation_response(id, confirmation_election_id, &member, true, 60)
                .expect("accept confirmation invitation");
        }
        narrow
            .state
            .seal_body_roster(id, confirmation_election_id, 61)
            .expect("disjoint confirmation roster");

        let mut exact_five = opened_policy_ballot(40, 40);
        assert_eq!(
            finalize_policy(&mut exact_five, 21, 19, 0),
            ParliamentAggregateOutcomeV1::Approved
        );
        assert_eq!(
            exact_five.state.attempt.stage,
            GovernanceStageV1::Certification
        );
        assert!(
            exact_five
                .state
                .required_bodies
                .iter()
                .all(|required| required.body != ParliamentBody::ConfirmationJury)
        );
    }

    #[test]
    fn certificate_and_terminal_transition_table_are_fail_closed() {
        let mut fixture = opened_policy_ballot(3, 3);
        assert_eq!(
            finalize_policy(&mut fixture, 2, 1, 0),
            ParliamentAggregateOutcomeV1::Approved
        );
        let id = fixture.state.attempt.id;
        assert_eq!(
            fixture.state.construct_certificate(id, 50, 50),
            Err(ParliamentReducerErrorV1::InvalidCertificateHeight)
        );
        let certificate = fixture
            .state
            .construct_certificate(id, 50, 60)
            .expect("complete certificate");
        assert_eq!(certificate.body_bindings.len(), 1);
        assert_eq!(
            fixture.state.mark_enacted(id, 59),
            Err(ParliamentReducerErrorV1::ReleaseHeightNotReached)
        );

        let mut enacted = fixture.state.clone();
        enacted.mark_enacted(id, 60).expect("enact due certificate");
        assert_eq!(enacted.attempt.status, GovernanceAttemptStatusV1::Enacted);
        assert_eq!(enacted.terminal_height(), Some(60));
        enacted
            .validate()
            .expect("enacted terminal state validates");
        assert!(matches!(
            enacted.mark_enacted(id, 61),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(_))
        ));

        let mut superseded = fixture.state.clone();
        assert_eq!(
            superseded.mark_superseded(id, 60, certificate.expected_head),
            Err(ParliamentReducerErrorV1::ExpectedHeadUnchanged)
        );
        superseded
            .mark_superseded(
                id,
                60,
                GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                    subject_id: root(99),
                }),
            )
            .expect("different head supersedes");
        assert_eq!(
            superseded.attempt.status,
            GovernanceAttemptStatusV1::Superseded
        );
        assert_eq!(superseded.terminal_height(), Some(60));
        assert_ne!(
            superseded.superseding_head(),
            Some(certificate.expected_head)
        );
        superseded
            .validate()
            .expect("superseded terminal state validates");

        let mut failed = fixture.state;
        assert_eq!(
            failed.mark_execution_failed(id, 60, root(99), root(100)),
            Err(ParliamentReducerErrorV1::CertificateBindingMismatch)
        );
        failed
            .mark_execution_failed(id, 60, root(3), root(100))
            .expect("exact effect records execution failure");
        assert_eq!(
            failed.attempt.status,
            GovernanceAttemptStatusV1::ExecutionFailed
        );
        assert_eq!(failed.terminal_height(), Some(60));
        assert_eq!(failed.execution_failure_root(), Some(root(100)));
        failed
            .validate()
            .expect("execution-failed terminal state validates");
    }

    #[test]
    fn reducer_norito_roundtrip_is_deterministic_and_revalidated() {
        let mut fixture = opened_policy_ballot(3, 3);
        finalize_policy(&mut fixture, 2, 1, 0);
        fixture
            .state
            .construct_certificate(fixture.state.attempt.id, 50, 60)
            .expect("certificate");
        fixture.state.validate().expect("source state validates");
        let bytes = norito::to_bytes(&fixture.state).expect("encode reducer state");
        let decoded = norito::decode_from_bytes::<ParliamentAttemptStateV1>(&bytes)
            .expect("decode reducer state");
        decoded.validate().expect("decoded state validates");
        assert_eq!(decoded, fixture.state);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode reducer state"),
            bytes
        );
        let json = norito::json::to_json(&decoded).expect("encode reducer state as Norito JSON");
        let json_decoded: ParliamentAttemptStateV1 =
            norito::json::from_json(&json).expect("decode reducer state from Norito JSON");
        json_decoded
            .validate()
            .expect("JSON-decoded state validates");
        assert_eq!(json_decoded, decoded);
    }
}
