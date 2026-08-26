//! Deterministic, fail-closed reducer for one SORA Parliament governance attempt.
//!
//! The reducer owns the consensus-relevant lifecycle links between an immutable
//! proposal, future-beacon sortition, sealed body instances, private OVN ballot
//! attempts, certification, and enactment.  Cryptographic verification happens
//! before a transition is submitted; this module makes the verified bindings
//! immutable and rejects replay, stage skipping, and cross-attempt substitution.
//!
//! There is deliberately no plaintext ballot or manual-opening transition.  A
//! missed phase deadline or objectively absent finalized release pulse produces
//! `NoResult`; retry requires a fresh ballot attempt and a fresh TLE session.

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
        MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1, MAX_PARLIAMENT_BALLOT_RETRIES_V1,
        ParliamentAggregateOutcomeV1, ParliamentAggregateTallyV1, ParliamentBallotAttemptV1,
        ParliamentBallotCertificateBindingV1, ParliamentBallotFailureKindV1, ParliamentBody,
        ParliamentBodyCertificateBindingV1, ParliamentBodyInstanceV1, ParliamentNoResultKindV1,
        ParliamentPublicFindingCertificateBindingV1, ParliamentSeatAssignmentV1, ProposalContentId,
        ProposalKind, RiskTierV1, SortitionRequestId, SortitionRequestV1, TleKeySessionId,
        TleSessionId, parliament_assignment_plan_root_v1, parliament_ballot_failure_root_v1,
        parliament_ballot_result_root_v1, parliament_candidate_root_v1,
        parliament_execution_failure_root_v1, parliament_public_finding_endorsement_root_v1,
        parliament_quorum_seats_v1, parliament_roster_root_v1,
    },
    isi::governance::parliament_timed_ovn_required_chunk_blocks_v1,
};
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

use super::{
    draw::{body_committee_size, derive_attempt_body_plan_v1},
    timed_ovn::TimedOvnParliamentReducerBindingV1,
};

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
    /// A sortition election was failed before its immutable pulse height passed.
    SortitionPulseStillPending,
    /// A candidate snapshot was empty, noncanonical, or disagreed with its committed root/count.
    InvalidCandidateSnapshot,
    /// The Confirmation Jury reused a Policy Jury member.
    ConfirmationJuryNotFresh,
    /// The transition used a public finding for a binding body, or vice versa.
    DecisionModeMismatch,
    /// The frozen public-finding endorsement schedule is zero, overflows, or changed.
    InvalidPublicFindingSchedule,
    /// An endorsement arrived after the inclusive public-finding deadline.
    PublicFindingWindowClosed,
    /// Public-finding expiry was requested no later than its inclusive deadline.
    PublicFindingWindowStillOpen,
    /// Persisted public-finding no-result evidence does not match reducer state.
    PublicFindingFailureKindMismatch,
    /// A ballot count exceeded a frozen registration, survivor, or seat bound.
    InvalidBallotCount,
    /// The transaction authority does not own a nonexcluded seat in this ballot body.
    UnauthorizedBallotParticipant,
    /// The transaction authority does not own the assignment declaring absence.
    UnauthorizedBodyMember,
    /// A later ballot transition attempted to change the accepted corpus.
    AcceptedCorpusMutation,
    /// The release height was not strictly after the timed seal height.
    ReleaseHeightNotFuture,
    /// The immutable timed-ballot phase schedule is invalid or changed mid-attempt.
    InvalidBallotSchedule,
    /// A certified terminal transition did not execute at its exact due height.
    WrongEnactmentHeight,
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
    /// Persisted reducer state records an action after the restored ledger height.
    FuturePersistedHeight,
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
            Self::SortitionPulseStillPending => {
                f.write_str("Parliament sortition pulse height has not passed")
            }
            Self::InvalidCandidateSnapshot => {
                f.write_str("invalid or mismatched Parliament candidate snapshot")
            }
            Self::ConfirmationJuryNotFresh => {
                f.write_str("Confirmation Jury is not disjoint from Policy Jury")
            }
            Self::DecisionModeMismatch => f.write_str("body decision mode mismatch"),
            Self::InvalidPublicFindingSchedule => {
                f.write_str("invalid public-finding endorsement schedule")
            }
            Self::PublicFindingWindowClosed => {
                f.write_str("public-finding endorsement window is closed")
            }
            Self::PublicFindingWindowStillOpen => {
                f.write_str("public-finding endorsement window is still open")
            }
            Self::PublicFindingFailureKindMismatch => {
                f.write_str("public-finding no-result evidence does not match reducer state")
            }
            Self::InvalidBallotCount => f.write_str("ballot count exceeds a frozen bound"),
            Self::UnauthorizedBallotParticipant => {
                f.write_str("account is not a nonexcluded member of this ballot body")
            }
            Self::UnauthorizedBodyMember => {
                f.write_str("account does not own this Parliament body assignment")
            }
            Self::AcceptedCorpusMutation => f.write_str("accepted ballot corpus is immutable"),
            Self::ReleaseHeightNotFuture => {
                f.write_str("ballot release height must be strictly future")
            }
            Self::InvalidBallotSchedule => f.write_str("invalid timed-ballot phase schedule"),
            Self::WrongEnactmentHeight => {
                f.write_str("certified governance effect must execute at its exact due height")
            }
            Self::WrongBallotPhaseHeight => {
                f.write_str("timed-ballot transition is outside its immutable phase boundary")
            }
            Self::BallotRetryLimitExceeded => f.write_str("private ballot retry limit exceeded"),
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
            Self::FuturePersistedHeight => {
                f.write_str("persisted Parliament action is ahead of the restored ledger height")
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

/// The one consensus policy version implemented by first-release Parliament.
pub(crate) const PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1: u64 = 1;

/// Derive the immutable initial risk and body pipeline for one typed proposal.
///
/// A reducer may subsequently raise the risk tier and append the dynamically
/// required Confirmation Jury, but neither operation may weaken this base
/// policy.
#[must_use]
pub(crate) fn parliament_attempt_policy_v1(
    proposal: &ProposalKind,
) -> (RiskTierV1, Vec<RequiredParliamentBodyV1>) {
    let bodies: &[ParliamentBody] = match proposal {
        ProposalKind::DeployContract(_) => &[
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::OversightCommittee,
            ParliamentBody::PolicyJury,
        ],
        ProposalKind::SccpRouteGovernance(_)
        | ProposalKind::ValidationFeePolicy(_)
        | ProposalKind::ValidationFeePayoutLifecycle(_) => &[
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::CoordinationCouncil,
            ParliamentBody::FmaCommittee,
            ParliamentBody::OversightCommittee,
            ParliamentBody::PolicyJury,
        ],
        ProposalKind::RuntimeUpgrade(_)
        | ProposalKind::MusubiRegistryGovernance(_)
        | ProposalKind::SorafsProviderGovernance(_) => &[
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::CoordinationCouncil,
            ParliamentBody::OversightCommittee,
            ParliamentBody::PolicyJury,
        ],
    };
    let risk_tier = if matches!(proposal, ProposalKind::DeployContract(_)) {
        RiskTierV1::Standard
    } else {
        RiskTierV1::Constitutional
    };
    let requirements = bodies
        .iter()
        .copied()
        .map(|body| RequiredParliamentBodyV1 {
            body,
            decision_mode: if body == ParliamentBody::PolicyJury {
                ParliamentDecisionModeV1::HiddenBindingBallot
            } else {
                ParliamentDecisionModeV1::PublicFinding
            },
        })
        .collect();
    (risk_tier, requirements)
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
    public_finding_endorsements: BTreeMap<AssignmentId, [u8; 32]>,
    public_finding_opened_at_height: Option<u64>,
    public_finding_phase_blocks: Option<u64>,
    public_finding_deadline_height: Option<u64>,
    public_finding_no_result_kind: Option<ParliamentNoResultKindV1>,
    public_finding_no_result_height: Option<u64>,
    public_finding_binding: Option<ParliamentPublicFindingCertificateBindingV1>,
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

    /// Return authority-authenticated public-finding endorsements by assignment.
    #[must_use]
    pub const fn public_finding_endorsements(&self) -> &BTreeMap<AssignmentId, [u8; 32]> {
        &self.public_finding_endorsements
    }

    /// Return the height opening the public-finding endorsement window.
    #[must_use]
    pub const fn public_finding_opened_at_height(&self) -> Option<u64> {
        self.public_finding_opened_at_height
    }

    /// Return the frozen public-finding endorsement span in blocks.
    #[must_use]
    pub const fn public_finding_phase_blocks(&self) -> Option<u64> {
        self.public_finding_phase_blocks
    }

    /// Return the inclusive frozen public-finding endorsement deadline.
    #[must_use]
    pub const fn public_finding_deadline_height(&self) -> Option<u64> {
        self.public_finding_deadline_height
    }

    /// Return the objective public-finding no-result class, when terminal.
    #[must_use]
    pub const fn public_finding_no_result_kind(&self) -> Option<ParliamentNoResultKindV1> {
        self.public_finding_no_result_kind
    }

    /// Return the containing height that made the public finding terminal.
    #[must_use]
    pub const fn public_finding_no_result_height(&self) -> Option<u64> {
        self.public_finding_no_result_height
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
    opening_phase_blocks: u64,
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
    opening_deadline_height: u64,
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

    /// Return the frozen authenticated registration count after registration closes.
    #[must_use]
    pub const fn registered_voters(&self) -> Option<u32> {
        self.registered_voters
    }

    /// Return the dedicated TLE session, once the timed seal is complete.
    #[must_use]
    pub const fn tle_session_id(&self) -> Option<TleSessionId> {
        self.tle_session_id
    }

    /// Return the finalized adaptive TLE key session bound to this ballot.
    #[must_use]
    pub const fn tle_key_session_id(&self) -> Option<TleKeySessionId> {
        self.tle_key_session_id
    }

    /// Return the threshold-beacon session committed for release.
    #[must_use]
    pub const fn release_beacon_session_id(&self) -> Option<BeaconSessionId> {
        self.release_beacon_session_id
    }

    /// Return the committed release height, once the timed seal is complete.
    #[must_use]
    pub const fn release_height(&self) -> Option<u64> {
        self.release_height
    }

    /// Return the immutable height that opened registration for this ballot.
    #[must_use]
    pub const fn registered_at_height(&self) -> u64 {
        self.registered_at_height
    }

    /// Return the exact height that closes authenticated registration.
    #[must_use]
    pub(crate) const fn registration_close_height(&self) -> u64 {
        self.registration_close_height
    }

    /// Return the exact height that freezes the survivor subsequence.
    #[must_use]
    pub(crate) const fn survivor_freeze_height(&self) -> u64 {
        self.survivor_freeze_height
    }

    /// Return the exact height that freezes the masked-ballot corpus.
    #[must_use]
    pub(crate) const fn commitment_close_height(&self) -> u64 {
        self.commitment_close_height
    }

    /// Return the immutable last height at which aggregate opening may complete.
    #[must_use]
    pub const fn opening_deadline_height(&self) -> u64 {
        self.opening_deadline_height
    }

    /// Return the Core-derived terminal failure class, when present.
    #[must_use]
    pub const fn failure_kind(&self) -> Option<ParliamentBallotFailureKindV1> {
        self.failure_kind
    }

    /// Return the containing height of the Core-derived terminal failure.
    #[must_use]
    pub const fn failure_height(&self) -> Option<u64> {
        self.failure_height
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

fn expected_head_is_valid(expected_head: GovernanceExpectedHeadV1) -> bool {
    match expected_head {
        GovernanceExpectedHeadV1::Absent(head) => !root_is_zero(&head.subject_id),
        GovernanceExpectedHeadV1::Present(head) => {
            !root_is_zero(&head.subject_id) && !root_is_zero(&head.head_root)
        }
    }
}

fn timed_ballot_schedule(
    registered_at_height: u64,
    policy: ParliamentTimedOvn,
) -> Result<(u64, u64, u64, u64, u64), ParliamentReducerErrorV1> {
    let minimum_registration_phase_blocks = u64::from(policy.max_corpus_entries)
        .checked_add(1)
        .ok_or(ParliamentReducerErrorV1::InvalidBallotSchedule)?;
    let minimum_survivor_freeze_phase_blocks = u64::from(policy.max_corpus_entries);
    if registered_at_height == 0
        || policy.registration_phase_blocks == 0
        || policy.survivor_freeze_phase_blocks == 0
        || policy.commitment_phase_blocks == 0
        || policy.release_delay_blocks == 0
        || policy.opening_phase_blocks == 0
        || policy.max_ballot_retries > MAX_PARLIAMENT_BALLOT_RETRIES_V1
        || !(1..=MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1).contains(&policy.max_corpus_entries)
        || policy.registration_phase_blocks < minimum_registration_phase_blocks
        || policy.survivor_freeze_phase_blocks < minimum_survivor_freeze_phase_blocks
        || policy.commitment_phase_blocks
            < parliament_timed_ovn_required_chunk_blocks_v1(policy.max_corpus_entries)
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
    let opening_deadline_height = release_height
        .checked_add(policy.opening_phase_blocks)
        .ok_or(ParliamentReducerErrorV1::InvalidBallotSchedule)?;
    Ok((
        registration_close_height,
        survivor_freeze_height,
        commitment_close_height,
        release_height,
        opening_deadline_height,
    ))
}

fn ballot_policy_matches(ballot: &ParliamentBallotStateV1, policy: ParliamentTimedOvn) -> bool {
    ballot.registration_phase_blocks == policy.registration_phase_blocks
        && ballot.survivor_freeze_phase_blocks == policy.survivor_freeze_phase_blocks
        && ballot.commitment_phase_blocks == policy.commitment_phase_blocks
        && ballot.release_delay_blocks == policy.release_delay_blocks
        && ballot.opening_phase_blocks == policy.opening_phase_blocks
        && ballot.max_ballot_retries == policy.max_ballot_retries
        && ballot.max_corpus_entries == policy.max_corpus_entries
}

fn ballot_policy(ballot: &ParliamentBallotStateV1) -> ParliamentTimedOvn {
    ParliamentTimedOvn {
        registration_phase_blocks: ballot.registration_phase_blocks,
        survivor_freeze_phase_blocks: ballot.survivor_freeze_phase_blocks,
        commitment_phase_blocks: ballot.commitment_phase_blocks,
        release_delay_blocks: ballot.release_delay_blocks,
        opening_phase_blocks: ballot.opening_phase_blocks,
        max_ballot_retries: ballot.max_ballot_retries,
        max_corpus_entries: ballot.max_corpus_entries,
    }
}

fn timed_commitment_height_is_in_window(ballot: &ParliamentBallotStateV1, height: u64) -> bool {
    height > ballot.survivor_freeze_height && height <= ballot.commitment_close_height
}

fn timed_commitment_completed_in_window(ballot: &ParliamentBallotStateV1) -> bool {
    ballot
        .commitment_closed_at_height
        .is_some_and(|height| timed_commitment_height_is_in_window(ballot, height))
}

fn classify_ballot_failure(
    ballot: &ParliamentBallotStateV1,
    release_pulse_available: bool,
    current_height: u64,
) -> Option<ParliamentBallotFailureKindV1> {
    match ballot.attempt.status {
        BallotAttemptStatusV1::Registration
            if current_height > ballot.registration_close_height =>
        {
            Some(ParliamentBallotFailureKindV1::RegistrationDeadlineExpired)
        }
        BallotAttemptStatusV1::SurvivorFreeze
            if ballot.registration_closed_at_height == Some(ballot.registration_close_height)
                && current_height > ballot.survivor_freeze_height =>
        {
            Some(ParliamentBallotFailureKindV1::SurvivorDeadlineExpired)
        }
        BallotAttemptStatusV1::TimedCommitment
            if ballot.survivors_frozen_at_height == Some(ballot.survivor_freeze_height)
                && current_height > ballot.commitment_close_height =>
        {
            Some(ParliamentBallotFailureKindV1::CommitmentDeadlineExpired)
        }
        BallotAttemptStatusV1::AwaitingRelease
            if timed_commitment_completed_in_window(ballot)
                && current_height > ballot.opening_deadline_height =>
        {
            Some(ParliamentBallotFailureKindV1::OpeningDeadlineExpired)
        }
        BallotAttemptStatusV1::AwaitingRelease
            if timed_commitment_completed_in_window(ballot)
                && !release_pulse_available
                && ballot
                    .release_height
                    .is_some_and(|release_height| current_height > release_height) =>
        {
            Some(ParliamentBallotFailureKindV1::ReleasePulseUnavailable)
        }
        BallotAttemptStatusV1::Opening if current_height > ballot.opening_deadline_height => {
            Some(ParliamentBallotFailureKindV1::OpeningDeadlineExpired)
        }
        _ => None,
    }
}

fn ballot_failure_matches_state(
    governance_attempt_id: GovernanceAttemptId,
    ballot_attempt_id: BallotAttemptId,
    ballot: &ParliamentBallotStateV1,
) -> bool {
    let Some(failure_kind) = ballot.failure_kind else {
        return false;
    };
    let Some(failure_height) = ballot.failure_height else {
        return false;
    };
    if ballot.failure_root
        != Some(parliament_ballot_failure_root_v1(
            governance_attempt_id,
            ballot_attempt_id,
            failure_kind,
            failure_height,
        ))
        || ballot.opening_root.is_some()
        || ballot.tally.is_some()
        || ballot.outcome.is_some()
    {
        return false;
    }

    let registration_frozen = ballot.registration_root.is_some()
        && ballot.registered_voters.is_some()
        && ballot.registration_closed_at_height == Some(ballot.registration_close_height);
    let survivors_frozen = registration_frozen
        && ballot.dropout_root.is_some()
        && ballot.survivor_root.is_some()
        && ballot.survivors.is_some()
        && ballot.no_recovery_root.is_some()
        && ballot.survivors_frozen_at_height == Some(ballot.survivor_freeze_height);
    let corpus_frozen = survivors_frozen
        && ballot.corpus_root.is_some()
        && ballot.accepted_ballots.is_some()
        && ballot.timed_commitment_root.is_some()
        && timed_commitment_completed_in_window(ballot);

    match failure_kind {
        ParliamentBallotFailureKindV1::RegistrationDeadlineExpired => {
            failure_height > ballot.registration_close_height
                && ballot.registration_root.is_none()
                && ballot.registered_voters.is_none()
                && ballot.registration_closed_at_height.is_none()
                && ballot.dropout_root.is_none()
                && ballot.survivor_root.is_none()
                && ballot.survivors.is_none()
                && ballot.no_recovery_root.is_none()
                && ballot.survivors_frozen_at_height.is_none()
                && ballot.corpus_root.is_none()
                && ballot.accepted_ballots.is_none()
                && ballot.timed_commitment_root.is_none()
                && ballot.commitment_closed_at_height.is_none()
                && ballot.release_pulse_id.is_none()
                && ballot.opening_height.is_none()
        }
        ParliamentBallotFailureKindV1::SurvivorDeadlineExpired => {
            failure_height > ballot.survivor_freeze_height
                && registration_frozen
                && ballot.dropout_root.is_none()
                && ballot.survivor_root.is_none()
                && ballot.survivors.is_none()
                && ballot.no_recovery_root.is_none()
                && ballot.survivors_frozen_at_height.is_none()
                && ballot.corpus_root.is_none()
                && ballot.accepted_ballots.is_none()
                && ballot.timed_commitment_root.is_none()
                && ballot.commitment_closed_at_height.is_none()
                && ballot.release_pulse_id.is_none()
                && ballot.opening_height.is_none()
        }
        ParliamentBallotFailureKindV1::CommitmentDeadlineExpired => {
            failure_height > ballot.commitment_close_height
                && survivors_frozen
                && ballot.corpus_root.is_none()
                && ballot.accepted_ballots.is_none()
                && ballot.timed_commitment_root.is_none()
                && ballot.commitment_closed_at_height.is_none()
                && ballot.release_pulse_id.is_none()
                && ballot.opening_height.is_none()
        }
        ParliamentBallotFailureKindV1::ReleasePulseUnavailable => {
            corpus_frozen
                && ballot
                    .release_height
                    .is_some_and(|release_height| failure_height > release_height)
                && failure_height <= ballot.opening_deadline_height
                && ballot.release_pulse_id.is_none()
                && ballot.opening_height.is_none()
        }
        ParliamentBallotFailureKindV1::OpeningDeadlineExpired => {
            corpus_frozen
                && failure_height > ballot.opening_deadline_height
                && match (ballot.release_pulse_id, ballot.opening_height) {
                    (None, None) => true,
                    (Some(_), Some(opening_height)) => {
                        ballot
                            .release_height
                            .is_some_and(|release_height| opening_height >= release_height)
                            && opening_height <= ballot.opening_deadline_height
                    }
                    _ => false,
                }
        }
    }
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

fn public_finding_quorum_is_unreachable(
    body: &ParliamentBodyStateV1,
) -> Result<bool, ParliamentReducerErrorV1> {
    let quorum = usize::try_from(parliament_quorum_seats_v1(body.instance.original_seats))
        .map_err(|_| ParliamentReducerErrorV1::InvalidRoster)?;
    let eligible = body
        .assignments
        .len()
        .checked_sub(body.excluded_assignments.len())
        .ok_or(ParliamentReducerErrorV1::InvalidRoster)?;
    let remaining = eligible
        .checked_sub(body.public_finding_endorsements.len())
        .ok_or(ParliamentReducerErrorV1::InvalidRoster)?;
    let mut endorsements_by_root = BTreeMap::<[u8; 32], usize>::new();
    for result_root in body.public_finding_endorsements.values() {
        *endorsements_by_root.entry(*result_root).or_default() += 1;
    }
    let strongest_existing_root = endorsements_by_root.values().copied().max().unwrap_or(0);
    Ok(strongest_existing_root.saturating_add(remaining) < quorum)
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
            || !expected_head_is_valid(expected_head)
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

    /// Return whether this immutable attempt references `member` in a selected
    /// Parliament seat assignment.
    #[must_use]
    pub(crate) fn references_parliament_member(&self, member: &AccountId) -> bool {
        self.elections.values().any(|election| {
            election
                .primary_assignments()
                .iter()
                .chain(election.alternate_assignments())
                .any(|assignment| &assignment.member == member)
        }) || self.bodies.values().any(|body| {
            body.assignments()
                .iter()
                .any(|assignment| &assignment.member == member)
        })
    }

    /// Return whether an active attempt still retains `member`'s citizenship bond.
    #[must_use]
    pub(crate) fn retains_citizenship_bond(&self, member: &AccountId) -> bool {
        matches!(
            self.attempt.status,
            GovernanceAttemptStatusV1::Active | GovernanceAttemptStatusV1::Certified
        ) && self.references_parliament_member(member)
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

    /// Validate immutable attempt bindings against their retained typed proposal.
    ///
    /// This check is required after persistence restore and again immediately
    /// before due execution. It accepts only upward risk escalation and the
    /// reducer-validated dynamic Confirmation Jury; all base proposal policy,
    /// effect, identity, and compare-and-set subject bindings remain exact.
    ///
    /// # Errors
    /// Returns an error when any proposal-derived binding was weakened or
    /// substituted.
    pub fn validate_proposal_bindings_v1(
        &self,
        proposal: &ProposalKind,
    ) -> Result<(), ParliamentReducerErrorV1> {
        let expected_proposal_content_id = ProposalContentId::new(proposal.fingerprint());
        let expected_effect_preimage_hash = proposal.effect_preimage_hash_v1();
        let expected_subject = proposal
            .governed_subject_id_v1()
            .map_err(|_| ParliamentReducerErrorV1::ProposalBindingMismatch)?;
        let observed_subject = match self.expected_head {
            GovernanceExpectedHeadV1::Absent(head) => head.subject_id,
            GovernanceExpectedHeadV1::Present(head) => head.subject_id,
        };
        let (base_risk_tier, base_required_bodies) = parliament_attempt_policy_v1(proposal);
        let persisted_base =
            self.required_bodies
                .last()
                .map_or(self.required_bodies.as_slice(), |last| {
                    if last.body == ParliamentBody::ConfirmationJury {
                        &self.required_bodies[..self.required_bodies.len() - 1]
                    } else {
                        self.required_bodies.as_slice()
                    }
                });

        if self.attempt.proposal_content_id != expected_proposal_content_id
            || self.policy_version != PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1
            || self.effect_preimage_hash != expected_effect_preimage_hash
            || observed_subject != expected_subject
            || !base_risk_tier.can_escalate_to(self.attempt.risk_tier)
            || persisted_base != base_required_bodies
        {
            return Err(ParliamentReducerErrorV1::ProposalBindingMismatch);
        }
        Ok(())
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

    /// Return the active sealed body instance for an exact Parliament role.
    ///
    /// This is used to derive the disjoint Confirmation Jury candidate set
    /// from the immutable Policy Jury roster without exposing mutable maps.
    #[must_use]
    pub fn sealed_body_for_role(&self, body: ParliamentBody) -> Option<&ParliamentBodyStateV1> {
        self.active_bodies
            .get(&body)
            .and_then(|body_instance_id| self.bodies.get(body_instance_id))
    }

    /// Return a private ballot attempt.
    #[must_use]
    pub fn ballot(&self, id: &BallotAttemptId) -> Option<&ParliamentBallotStateV1> {
        self.ballots.get(id)
    }

    /// Return the complete immutable reducer projection that must agree with
    /// replayed timed-OVN lifecycle evidence during snapshot restoration.
    #[must_use]
    pub(crate) fn timed_ovn_reducer_binding(
        &self,
        id: &BallotAttemptId,
    ) -> Option<TimedOvnParliamentReducerBindingV1> {
        let ballot = self.ballots.get(id)?;
        Some(TimedOvnParliamentReducerBindingV1 {
            proposal_content_id: *self.attempt.proposal_content_id.as_bytes(),
            governance_attempt_id: *self.attempt.id.as_bytes(),
            body_instance_id: *ballot.attempt.body_instance_id.as_bytes(),
            ballot_attempt_id: *ballot.attempt.id.as_bytes(),
            tle_key_session_id: ballot.tle_key_session_id,
            registration_opened_at_finalized_height: ballot
                .corpus_root
                .is_none()
                .then_some(ballot.registered_at_height),
            release_height: ballot.release_height,
            registration_root: ballot.registration_root,
            registered_voters: ballot.registered_voters,
            dropout_root: ballot.dropout_root,
            survivor_root: ballot.survivor_root,
            survivors: ballot.survivors,
            no_recovery_root: ballot.no_recovery_root,
            corpus_root: ballot.corpus_root,
            accepted_ballots: ballot.accepted_ballots,
            timed_commitment_root: ballot.timed_commitment_root,
            opening_root: ballot.opening_root,
            tally_counts: ballot
                .tally
                .map(|tally| [tally.aye, tally.nay, tally.abstain]),
        })
    }

    /// Return whether replayed timed-OVN evidence matches every reducer-owned
    /// field that the lifecycle duplicates.
    #[must_use]
    pub(crate) fn timed_ovn_reducer_binding_matches(
        &self,
        id: &BallotAttemptId,
        replayed: &TimedOvnParliamentReducerBindingV1,
    ) -> bool {
        self.timed_ovn_reducer_binding(id).as_ref() == Some(replayed)
    }

    /// Return the latest private ballot attempt bound to one exact body instance.
    ///
    /// The returned state exposes only bounded lifecycle getters. Timed-OVN
    /// registrations, masked ballot records, shares, and individual openings
    /// remain outside this reducer projection.
    #[must_use]
    pub fn active_ballot_for_body(
        &self,
        body_instance_id: &BodyInstanceId,
    ) -> Option<&ParliamentBallotStateV1> {
        self.active_ballots
            .get(body_instance_id)
            .and_then(|ballot_attempt_id| self.ballots.get(ballot_attempt_id))
    }

    /// Validate one seated authority's timed-OVN registration window.
    ///
    /// # Errors
    /// Returns an error unless the exact ballot is in registration, the
    /// containing height precedes the immutable close boundary, and `member`
    /// owns a nonexcluded seat in the bound body instance.
    pub(crate) fn validate_ballot_registration_member(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        member: &AccountId,
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.validate_ballot_member_action(
            governance_attempt_id,
            ballot_attempt_id,
            member,
            current_height,
            BallotAttemptStatusV1::Registration,
        )
    }

    /// Validate one registered authority's pre-freeze dropout window.
    ///
    /// # Errors
    /// Returns an error unless registration is closed, the immutable survivor
    /// boundary has not arrived, and `member` owns a nonexcluded seat in the
    /// exact body instance. Cryptographic lifecycle validation separately
    /// requires that this member registered and has not already withdrawn.
    pub(crate) fn validate_ballot_dropout_member(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        member: &AccountId,
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.validate_ballot_member_action(
            governance_attempt_id,
            ballot_attempt_id,
            member,
            current_height,
            BallotAttemptStatusV1::SurvivorFreeze,
        )
    }

    fn validate_ballot_member_action(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        member: &AccountId,
        current_height: u64,
        expected_status: BallotAttemptStatusV1,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
        if ballot.attempt.status != expected_status {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        let inside_window = match expected_status {
            BallotAttemptStatusV1::Registration => {
                current_height >= ballot.registered_at_height
                    && current_height < ballot.registration_close_height
            }
            BallotAttemptStatusV1::SurvivorFreeze => {
                current_height >= ballot.registration_close_height
                    && current_height < ballot.survivor_freeze_height
            }
            _ => false,
        };
        if !inside_window {
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
        let assignment = body
            .assignments
            .iter()
            .find(|assignment| &assignment.member == member)
            .ok_or(ParliamentReducerErrorV1::UnauthorizedBallotParticipant)?;
        if body
            .excluded_assignments
            .contains(&assignment.assignment_id)
        {
            return Err(ParliamentReducerErrorV1::UnauthorizedBallotParticipant);
        }
        Ok(())
    }

    /// Iterate every canonical ballot-attempt entry for persistence validation.
    pub(crate) fn ballot_attempts(
        &self,
    ) -> impl ExactSizeIterator<Item = (&BallotAttemptId, &ParliamentBallotStateV1)> {
        self.ballots.iter()
    }

    /// Return the greatest committed opening deadline that references one TLE key session.
    ///
    /// Runtime secret-share custody uses this read-only projection before
    /// retiring a rotating share. Historical retries remain included: a share
    /// is retained through every deadline ever committed for this attempt,
    /// even when a later retry superseded the corresponding ballot.
    #[must_use]
    pub(crate) fn tle_key_session_retention_deadline(
        &self,
        key_session_id: TleKeySessionId,
    ) -> Option<u64> {
        self.ballots
            .values()
            .filter(|ballot| ballot.tle_key_session_id == Some(key_session_id))
            .map(|ballot| ballot.opening_deadline_height)
            .max()
    }

    /// Return whether a live reducer object requests the exact beacon slot.
    ///
    /// Sortition requests remain live only while awaiting their future pulse.
    /// Timed ballots request their frozen release slot from registration until
    /// they either consume the pulse or become terminal, so an otherwise valid
    /// arbitrary release height is visible to consensus before it arrives.
    #[must_use]
    pub(crate) fn requires_beacon_pulse_at(
        &self,
        beacon_session_id: BeaconSessionId,
        height: u64,
    ) -> bool {
        if self.attempt.status != GovernanceAttemptStatusV1::Active {
            return false;
        }
        self.elections.values().any(|election| {
            election.attempt.status == BodyElectionAttemptStatusV1::AwaitingPulse
                && election.attempt.request.beacon_session_id == beacon_session_id
                && election.attempt.request.pulse_height == height
        }) || self.ballots.values().any(|ballot| {
            matches!(
                ballot.attempt.status,
                BallotAttemptStatusV1::Registration
                    | BallotAttemptStatusV1::SurvivorFreeze
                    | BallotAttemptStatusV1::TimedCommitment
                    | BallotAttemptStatusV1::AwaitingRelease
            ) && ballot.release_beacon_session_id == Some(beacon_session_id)
                && ballot.release_height == Some(height)
                && ballot.release_pulse_id.is_none()
        })
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

    /// Validate all realized lifecycle chronology against a restored committed height.
    ///
    /// A still-certified effect must remain strictly future. Conversely, a
    /// terminal execution outcome and the certificate that authorized it may
    /// not claim heights beyond the restored ledger boundary. Persisted
    /// elections, bodies, ballots, and consumed pulses likewise cannot record
    /// actions from future blocks, and the atomic pre-certificate transient is
    /// never a valid restart state.
    ///
    /// # Errors
    /// Returns an error when persisted lifecycle chronology is impossible at
    /// `restored_height`.
    pub(crate) fn validate_restored_height_v1(
        &self,
        restored_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        let missing_certificate_at_persisted_stage = self.certificate.is_none()
            && matches!(
                self.attempt.stage,
                GovernanceStageV1::Certification | GovernanceStageV1::Enactment
            );
        if let Some(certificate) = self.certificate.as_ref() {
            if certificate.certified_at_height > restored_height {
                return Err(ParliamentReducerErrorV1::InvalidCertificateHeight);
            }
            match self.attempt.status {
                GovernanceAttemptStatusV1::Certified
                    if certificate.enact_at_height <= restored_height =>
                {
                    return Err(ParliamentReducerErrorV1::WrongEnactmentHeight);
                }
                GovernanceAttemptStatusV1::Enacted
                | GovernanceAttemptStatusV1::Superseded
                | GovernanceAttemptStatusV1::ExecutionFailed
                    if certificate.enact_at_height > restored_height =>
                {
                    return Err(ParliamentReducerErrorV1::InvalidCertificateHeight);
                }
                _ => {}
            }
        }

        let future_election_height = self.elections.values().any(|election| {
            let request = election.attempt.request;
            request.request_height > restored_height
                || (election.pulse_id.is_some() && request.pulse_height > restored_height)
                || election
                    .invitation_opened_at_height
                    .is_some_and(|height| height > restored_height)
                || (matches!(
                    election.attempt.status,
                    BodyElectionAttemptStatusV1::Sealed
                        | BodyElectionAttemptStatusV1::NoRoster
                        | BodyElectionAttemptStatusV1::Superseded
                ) && election
                    .invitation_close_height
                    .is_none_or(|height| height >= restored_height))
        });
        let future_body_height = self.bodies.values().any(|body| {
            body.result_height
                .is_some_and(|height| height > restored_height)
                || body
                    .public_finding_opened_at_height
                    .is_some_and(|height| height > restored_height)
                || body
                    .public_finding_no_result_height
                    .is_some_and(|height| height > restored_height)
        });
        let future_ballot_height = self.ballots.values().any(|ballot| {
            ballot.registered_at_height > restored_height
                || ballot
                    .registration_closed_at_height
                    .is_some_and(|height| height > restored_height)
                || ballot
                    .survivors_frozen_at_height
                    .is_some_and(|height| height > restored_height)
                || ballot
                    .commitment_closed_at_height
                    .is_some_and(|height| height > restored_height)
                || ballot
                    .opening_height
                    .is_some_and(|height| height > restored_height)
                || ballot
                    .failure_height
                    .is_some_and(|height| height > restored_height)
        });
        let future_consumed_pulse = self
            .used_pulse_slots
            .keys()
            .any(|slot| slot.height > restored_height);
        if future_election_height
            || future_body_height
            || future_ballot_height
            || future_consumed_pulse
        {
            return Err(ParliamentReducerErrorV1::FuturePersistedHeight);
        }
        if missing_certificate_at_persisted_stage {
            return Err(ParliamentReducerErrorV1::IncompleteCertificate);
        }
        Ok(())
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
    /// The first consumed pulse must cover every initially required body in one
    /// simultaneous batch, so a trigger cannot evade cross-body concentration
    /// limits by splitting the draw. Later no-roster retries and a dynamically
    /// required Confirmation Jury may use fresh dedicated pulse slots.
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
        if self.used_pulse_ids.is_empty() {
            let mut expected_initial_bodies: Vec<_> = self
                .required_bodies
                .iter()
                .filter_map(|requirement| {
                    (requirement.body != ParliamentBody::ConfirmationJury)
                        .then_some(requirement.body)
                })
                .collect();
            expected_initial_bodies.sort_unstable();
            if bodies != expected_initial_bodies {
                return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
            }
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

    /// Mark an election unable to obtain its pulse or form a nonempty roster.
    ///
    /// # Errors
    /// Returns an error for a wrong attempt, unknown election, or replay from a
    /// state other than an objectively expired pulse wait or invitation
    /// acceptance.
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
        if election.attempt.status == BodyElectionAttemptStatusV1::AwaitingPulse {
            if current_height <= election.attempt.request.pulse_height {
                return Err(ParliamentReducerErrorV1::SortitionPulseStillPending);
            }
            self.elections
                .get_mut(&election_attempt_id)
                .expect("election checked above")
                .attempt
                .status = BodyElectionAttemptStatusV1::NoRoster;
            return Ok(());
        }
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
                public_finding_endorsements: BTreeMap::new(),
                public_finding_opened_at_height: None,
                public_finding_phase_blocks: None,
                public_finding_deadline_height: None,
                public_finding_no_result_kind: None,
                public_finding_no_result_height: None,
                public_finding_binding: None,
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
        current_height: u64,
        public_finding_phase_blocks: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        let decision_mode = self.ensure_current_body(body.instance.body)?.decision_mode;
        if body.instance.governance_attempt_id != governance_attempt_id
            || self.active_bodies.get(&body.instance.body) != Some(&body_instance_id)
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        let election = self
            .elections
            .get(&body.instance.election_attempt_id)
            .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
        if current_height <= election.attempt.request.pulse_height {
            return Err(ParliamentReducerErrorV1::InvalidPublicFindingSchedule);
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
            && decision_mode != ParliamentDecisionModeV1::HiddenBindingBallot
        {
            return Err(ParliamentReducerErrorV1::DecisionModeMismatch);
        }
        let body = self
            .bodies
            .get_mut(&body_instance_id)
            .expect("body checked above");
        if target == DeliberationPhaseV1::Reflection
            && decision_mode == ParliamentDecisionModeV1::PublicFinding
        {
            let deadline = current_height
                .checked_add(public_finding_phase_blocks)
                .filter(|_| public_finding_phase_blocks != 0)
                .ok_or(ParliamentReducerErrorV1::InvalidPublicFindingSchedule)?;
            if body.public_finding_opened_at_height.is_some()
                || body.public_finding_phase_blocks.is_some()
                || body.public_finding_deadline_height.is_some()
            {
                return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                    ParliamentReducerEntityV1::BodyInstance,
                ));
            }
            body.public_finding_opened_at_height = Some(current_height);
            body.public_finding_phase_blocks = Some(public_finding_phase_blocks);
            body.public_finding_deadline_height = Some(deadline);
        }
        body.instance.status = BodyInstanceStatusV1::Deliberating(target);
        Ok(())
    }

    /// Record a member-authenticated absence without changing the quorum denominator.
    ///
    /// The reducer records no slash, cooldown, or future-selection penalty. The
    /// same assignment cannot be excluded twice, and an exclusion cannot be
    /// introduced after balloting starts. `member` must own the exact named
    /// assignment, preventing a manager or another member from fabricating it.
    /// A public-finding body is marked `NoResult` and the governance attempt is
    /// rejected as soon as the remaining nonexcluded seats can no longer reach
    /// its immutable original-seat quorum.
    ///
    /// # Errors
    /// Returns an error for wrong bindings, an unknown assignment, an authority
    /// that does not own it, replay, or a body that has already entered or
    /// completed balloting.
    pub fn record_attempt_absence(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        assignment_id: AssignmentId,
        member: &AccountId,
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        let decision_mode = self.ensure_current_body(body.instance.body)?.decision_mode;
        if body.instance.governance_attempt_id != governance_attempt_id {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        let election = self
            .elections
            .get(&body.instance.election_attempt_id)
            .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
        if current_height <= election.attempt.request.pulse_height {
            return Err(ParliamentReducerErrorV1::InvalidPublicFindingSchedule);
        }
        if decision_mode == ParliamentDecisionModeV1::PublicFinding
            && body
                .public_finding_deadline_height
                .is_some_and(|deadline| current_height > deadline)
        {
            return Err(ParliamentReducerErrorV1::PublicFindingWindowClosed);
        }
        let assignment = body
            .assignments
            .iter()
            .find(|seat| seat.assignment_id == assignment_id)
            .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
        if &assignment.member != member {
            return Err(ParliamentReducerErrorV1::UnauthorizedBodyMember);
        }
        if !matches!(
            body.instance.status,
            BodyInstanceStatusV1::RosterSealed | BodyInstanceStatusV1::Deliberating(_)
        ) || !body.public_finding_endorsements.is_empty()
        {
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
        if decision_mode == ParliamentDecisionModeV1::PublicFinding
            && public_finding_quorum_is_unreachable(body)?
        {
            body.instance.status = BodyInstanceStatusV1::NoResult;
            body.public_finding_no_result_kind =
                Some(ParliamentNoResultKindV1::PublicFindingQuorumUnreachable);
            body.public_finding_no_result_height = Some(current_height);
            self.attempt.status = GovernanceAttemptStatusV1::Rejected;
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
            opening_deadline_height: ballot.opening_deadline_height,
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
            original_seats: body.instance.original_seats,
            beacon_session_id: election.attempt.request.beacon_session_id,
            beacon_pulse_id,
            roster_root: body.roster_root,
            assignment_root,
            result_root,
            result_height,
            public_finding: body.public_finding_binding.clone(),
            ballot,
        })
    }

    /// Record one seated member's endorsement of a public, nonbinding finding.
    ///
    /// Each assignment may endorse exactly one result root. The body result
    /// finalizes automatically once one root reaches the immutable two-thirds
    /// original-seat quorum, so a manager cannot invent or select the finding.
    /// If immutable endorsements split so that no root can reach quorum even if
    /// every remaining eligible assignment joins it, the body becomes
    /// `NoResult` and the governance attempt is rejected deterministically.
    ///
    /// # Errors
    /// Returns an error for a binding body, wrong stage/bindings, zero result
    /// root, a nonmember/excluded authority, replay, or a body that has not
    /// completed reflection. Returns whether this endorsement finalized the body.
    pub fn endorse_public_finding(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        result_root: [u8; 32],
        member: &AccountId,
        result_height: u64,
    ) -> Result<bool, ParliamentReducerErrorV1> {
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
        let opened_at_height = body
            .public_finding_opened_at_height
            .ok_or(ParliamentReducerErrorV1::InvalidPublicFindingSchedule)?;
        let deadline_height = body
            .public_finding_deadline_height
            .ok_or(ParliamentReducerErrorV1::InvalidPublicFindingSchedule)?;
        if result_height < opened_at_height || result_height > deadline_height {
            return Err(ParliamentReducerErrorV1::PublicFindingWindowClosed);
        }
        let assignment = body
            .assignments
            .iter()
            .find(|assignment| &assignment.member == member)
            .ok_or(ParliamentReducerErrorV1::UnauthorizedBodyMember)?;
        if body
            .excluded_assignments
            .contains(&assignment.assignment_id)
            || body
                .public_finding_endorsements
                .contains_key(&assignment.assignment_id)
        {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        let assignment_id = assignment.assignment_id;
        let quorum = parliament_quorum_seats_v1(body.instance.original_seats);
        {
            let body = self
                .bodies
                .get_mut(&body_instance_id)
                .expect("body checked above");
            body.public_finding_endorsements
                .insert(assignment_id, result_root);
        }
        let endorsing_assignments = self
            .bodies
            .get(&body_instance_id)
            .expect("body checked above")
            .public_finding_endorsements
            .iter()
            .filter_map(|(assignment_id, endorsed_root)| {
                (*endorsed_root == result_root).then_some(*assignment_id)
            })
            .collect::<Vec<_>>();
        let endorsements = u32::try_from(endorsing_assignments.len())
            .map_err(|_| ParliamentReducerErrorV1::InvalidRoster)?;
        if endorsements < quorum {
            let body = self
                .bodies
                .get_mut(&body_instance_id)
                .expect("body checked above");
            if public_finding_quorum_is_unreachable(body)? {
                body.instance.status = BodyInstanceStatusV1::NoResult;
                body.public_finding_no_result_kind =
                    Some(ParliamentNoResultKindV1::PublicFindingQuorumUnreachable);
                body.public_finding_no_result_height = Some(result_height);
                self.attempt.status = GovernanceAttemptStatusV1::Rejected;
            }
            return Ok(false);
        }
        let endorsement_root = parliament_public_finding_endorsement_root_v1(
            governance_attempt_id,
            body_instance_id,
            result_root,
            &endorsing_assignments,
        );
        if root_is_zero(&endorsement_root) {
            return Err(ParliamentReducerErrorV1::ZeroCommitmentRoot);
        }
        {
            let body = self
                .bodies
                .get_mut(&body_instance_id)
                .expect("body checked above");
            body.instance.status = BodyInstanceStatusV1::Approved;
            body.result_root = Some(result_root);
            body.result_height = Some(result_height);
            body.public_finding_binding = Some(ParliamentPublicFindingCertificateBindingV1 {
                endorsement_root,
                endorsing_assignments,
                endorsements,
                quorum,
            });
        }
        let binding = self.build_body_binding(body_instance_id)?;
        if self.body_bindings.insert(body_role, binding).is_some() {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        self.advance_after_body(body_role)?;
        Ok(true)
    }

    /// Terminally reject a public-finding body after its frozen endorsement deadline.
    ///
    /// The caller supplies only the body identifier. Core derives the schedule,
    /// verifies that no finding was finalized, and records the containing height
    /// as objective no-result evidence.
    ///
    /// # Errors
    /// Returns an error for a wrong attempt/body/mode, a replay, a body outside
    /// Reflection, or a trigger submitted no later than the inclusive deadline.
    pub fn fail_public_finding_no_result(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let body =
            self.bodies
                .get(&body_instance_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BodyInstance,
                ))?;
        let decision_mode = self.ensure_current_body(body.instance.body)?.decision_mode;
        if decision_mode != ParliamentDecisionModeV1::PublicFinding {
            return Err(ParliamentReducerErrorV1::DecisionModeMismatch);
        }
        if body.instance.governance_attempt_id != governance_attempt_id
            || body.instance.status
                != BodyInstanceStatusV1::Deliberating(DeliberationPhaseV1::Reflection)
            || body.public_finding_binding.is_some()
            || body.public_finding_no_result_kind.is_some()
            || body.public_finding_no_result_height.is_some()
        {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance,
            ));
        }
        let deadline_height = body
            .public_finding_deadline_height
            .ok_or(ParliamentReducerErrorV1::InvalidPublicFindingSchedule)?;
        if current_height <= deadline_height {
            return Err(ParliamentReducerErrorV1::PublicFindingWindowStillOpen);
        }
        let body = self
            .bodies
            .get_mut(&body_instance_id)
            .expect("body checked above");
        body.instance.status = BodyInstanceStatusV1::NoResult;
        body.public_finding_no_result_kind =
            Some(ParliamentNoResultKindV1::PublicFindingDeadlineExpired);
        body.public_finding_no_result_height = Some(current_height);
        self.attempt.status = GovernanceAttemptStatusV1::Rejected;
        Ok(())
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
            opening_deadline_height,
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
        let election = self
            .elections
            .get(&body.instance.election_attempt_id)
            .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
        if election
            .invitation_close_height
            .is_none_or(|close_height| registered_at_height <= close_height)
        {
            return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
        }
        let original_seats = body.instance.original_seats;
        if timed_ovn_policy.max_corpus_entries < original_seats {
            return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
        }
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
                if previous
                    .failure_height
                    .is_none_or(|failure_height| registered_at_height < failure_height)
                {
                    return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
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
                opening_phase_blocks: timed_ovn_policy.opening_phase_blocks,
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
                opening_deadline_height,
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

    /// Cheaply authorize the registration-close checkpoint before corpus replay.
    ///
    /// This checks only reducer-owned scalar state and bindings. Callers must
    /// still replay and validate the complete timed-OVN registration corpus
    /// after this succeeds.
    ///
    /// # Errors
    /// Returns an error for an inactive attempt, wrong ballot/body binding,
    /// replayed phase, or a containing height other than the frozen deadline.
    pub(crate) fn precheck_close_ballot_registration(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        current_height: u64,
    ) -> Result<&ParliamentBallotStateV1, ParliamentReducerErrorV1> {
        self.precheck_ballot_checkpoint(
            governance_attempt_id,
            ballot_attempt_id,
            BallotAttemptStatusV1::Registration,
            |ballot| current_height == ballot.registration_close_height,
        )
    }

    /// Cheaply authorize the survivor-freeze checkpoint before corpus replay.
    ///
    /// # Errors
    /// Returns an error for an inactive attempt, wrong ballot/body binding,
    /// replayed phase, or a containing height other than the frozen deadline.
    pub(crate) fn precheck_freeze_ballot_survivors(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        current_height: u64,
    ) -> Result<&ParliamentBallotStateV1, ParliamentReducerErrorV1> {
        self.precheck_ballot_checkpoint(
            governance_attempt_id,
            ballot_attempt_id,
            BallotAttemptStatusV1::SurvivorFreeze,
            |ballot| {
                current_height == ballot.survivor_freeze_height
                    && ballot.registration_closed_at_height
                        == Some(ballot.registration_close_height)
            },
        )
    }

    /// Cheaply authorize one bounded corpus append during the commitment window.
    ///
    /// # Errors
    /// Returns an error for an inactive attempt, wrong ballot/body binding,
    /// replayed phase, or a containing height outside the frozen window.
    pub(crate) fn precheck_freeze_timed_ovn_corpus(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        current_height: u64,
    ) -> Result<&ParliamentBallotStateV1, ParliamentReducerErrorV1> {
        self.precheck_ballot_checkpoint(
            governance_attempt_id,
            ballot_attempt_id,
            BallotAttemptStatusV1::TimedCommitment,
            |ballot| {
                timed_commitment_height_is_in_window(ballot, current_height)
                    && ballot.survivors_frozen_at_height == Some(ballot.survivor_freeze_height)
            },
        )
    }

    fn precheck_ballot_checkpoint(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        expected_status: BallotAttemptStatusV1,
        height_is_exact: impl FnOnce(&ParliamentBallotStateV1) -> bool,
    ) -> Result<&ParliamentBallotStateV1, ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
        if ballot.attempt.status != expected_status {
            return Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt,
            ));
        }
        if !height_is_exact(ballot) {
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
        Ok(ballot)
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
        let ballot = self.precheck_close_ballot_registration(
            governance_attempt_id,
            ballot_attempt_id,
            current_height,
        )?;
        let body = self
            .bodies
            .get(&ballot.attempt.body_instance_id)
            .expect("checkpoint precheck verified the body");
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
        let ballot = self.precheck_freeze_ballot_survivors(
            governance_attempt_id,
            ballot_attempt_id,
            current_height,
        )?;
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
    /// ballot, zero roots, unknown ballot, a completion outside the commitment
    /// window, or wrong attempt.
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
        let ballot = self.precheck_freeze_timed_ovn_corpus(
            governance_attempt_id,
            ballot_attempt_id,
            current_height,
        )?;
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
                || at_height > ballot.opening_deadline_height
                || !timed_commitment_completed_in_window(ballot)
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

    /// Mark an objectively expired ballot phase as `NoResult`.
    ///
    /// There is no manual or plaintext fallback. A retry must register a fresh
    /// ballot attempt and, if it reaches timed sealing, a fresh TLE session.
    ///
    /// # Errors
    /// Returns an error for an unknown/wrong attempt or a terminal/replayed
    /// ballot transition.
    pub fn fail_ballot_no_result(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        ballot_attempt_id: BallotAttemptId,
        release_pulse_available: bool,
        current_height: u64,
    ) -> Result<(), ParliamentReducerErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let ballot =
            self.ballots
                .get(&ballot_attempt_id)
                .ok_or(ParliamentReducerErrorV1::UnknownEntity(
                    ParliamentReducerEntityV1::BallotAttempt,
                ))?;
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
        let failure_kind = classify_ballot_failure(ballot, release_pulse_available, current_height)
            .ok_or(ParliamentReducerErrorV1::BallotFailureKindMismatch)?;
        let failure_root = parliament_ballot_failure_root_v1(
            governance_attempt_id,
            ballot_attempt_id,
            failure_kind,
            current_height,
        );
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
        let retry_budget_exhausted = ballot.attempt.sequence == ballot.max_ballot_retries;
        ballot.failure_root = Some(failure_root);
        ballot.failure_kind = Some(failure_kind);
        ballot.failure_height = Some(current_height);
        ballot.attempt.status = BallotAttemptStatusV1::NoResult;
        self.bodies
            .get_mut(&body_id)
            .expect("body checked above")
            .instance
            .status = BodyInstanceStatusV1::NoResult;
        if retry_budget_exhausted {
            self.attempt.status = GovernanceAttemptStatusV1::Rejected;
        }
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
        let opening_deadline_height = ballot.opening_deadline_height;
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
        if opening_height < release_height
            || opening_height > opening_deadline_height
            || result_height < opening_height
            || result_height > opening_deadline_height
        {
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
            opening_deadline_height,
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
                .cloned()
                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
            if binding.body != requirement.body {
                return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
            }
            match requirement.decision_mode {
                ParliamentDecisionModeV1::PublicFinding
                    if binding.public_finding.is_none() || binding.ballot.is_some() =>
                {
                    return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                }
                ParliamentDecisionModeV1::HiddenBindingBallot
                    if binding.public_finding.is_some() || binding.ballot.is_none() =>
                {
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
        if at_height != certificate.enact_at_height {
            return Err(ParliamentReducerErrorV1::WrongEnactmentHeight);
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

    /// Mark deterministic execution failure for the exact due certificate.
    ///
    /// The failure transcript root is derived entirely from the retained
    /// certificate and its immutable enactment height. Callers cannot supply
    /// either an effect binding or a failure root.
    ///
    /// # Errors
    /// Returns an error before or after the exact due height, for a wrong
    /// attempt, or for any replay/noncertified transition.
    pub fn mark_execution_failed(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        at_height: u64,
    ) -> Result<[u8; 32], ParliamentReducerErrorV1> {
        let certificate = self.ensure_certified_for_execution(governance_attempt_id, at_height)?;
        let failure_root = parliament_execution_failure_root_v1(certificate, at_height);
        self.attempt.status = GovernanceAttemptStatusV1::ExecutionFailed;
        self.terminal_height = Some(at_height);
        self.execution_failure_root = Some(failure_root);
        Ok(failure_root)
    }
}

impl ParliamentAttemptStateV1 {
    fn expected_completed_body_count_v1(&self) -> Result<usize, ParliamentReducerErrorV1> {
        let body_index = || {
            self.required_bodies
                .iter()
                .position(|required| stage_for_body(required.body) == self.attempt.stage)
                .ok_or(ParliamentReducerErrorV1::InvalidRequiredBodyPipeline)
        };
        match self.attempt.status {
            GovernanceAttemptStatusV1::Active => match self.attempt.stage {
                GovernanceStageV1::Qualification => Ok(0),
                GovernanceStageV1::Certification | GovernanceStageV1::Enactment => {
                    Ok(self.required_bodies.len())
                }
                _ => body_index(),
            },
            GovernanceAttemptStatusV1::Rejected => {
                let index = body_index()?;
                let current_body = self
                    .sealed_body_for_role(self.required_bodies[index].body)
                    .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
                if current_body.instance.status == BodyInstanceStatusV1::NoResult {
                    Ok(index)
                } else {
                    Ok(index + 1)
                }
            }
            GovernanceAttemptStatusV1::Certified
            | GovernanceAttemptStatusV1::Enacted
            | GovernanceAttemptStatusV1::Superseded
            | GovernanceAttemptStatusV1::ExecutionFailed => {
                if self.attempt.stage != GovernanceStageV1::Enactment {
                    return Err(ParliamentReducerErrorV1::IncompleteCertificate);
                }
                Ok(self.required_bodies.len())
            }
        }
    }

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
            || !expected_head_is_valid(self.expected_head)
            || !self.attempt.has_canonical_id()
        {
            return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
        }
        if !persisted_pipeline_is_canonical(&self.required_bodies) {
            return Err(ParliamentReducerErrorV1::InvalidRequiredBodyPipeline);
        }
        let expected_completed_bodies = self.expected_completed_body_count_v1()?;
        if self.body_bindings.len() != expected_completed_bodies
            || self.required_bodies[..expected_completed_bodies]
                .iter()
                .any(|required| !self.body_bindings.contains_key(&required.body))
        {
            return Err(ParliamentReducerErrorV1::IncompleteCertificate);
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
        let initial_required_bodies: BTreeSet<_> = self
            .required_bodies
            .iter()
            .filter_map(|requirement| {
                (requirement.body != ParliamentBody::ConfirmationJury).then_some(requirement.body)
            })
            .collect();
        let initial_drawn: Vec<_> = self
            .elections
            .values()
            .filter(|election| {
                election.attempt.sequence == 0
                    && election.attempt.request.body != ParliamentBody::ConfirmationJury
                    && election.pulse_id.is_some()
            })
            .collect();
        if let Some(first) = initial_drawn.first() {
            let drawn_bodies: BTreeSet<_> = initial_drawn
                .iter()
                .map(|election| election.attempt.request.body)
                .collect();
            if drawn_bodies != initial_required_bodies
                || initial_drawn.iter().any(|election| {
                    election.pulse_id != first.pulse_id
                        || election.pulse_output != first.pulse_output
                        || election.attempt.request.beacon_session_id
                            != first.attempt.request.beacon_session_id
                        || election.attempt.request.pulse_height
                            != first.attempt.request.pulse_height
                        || election.cross_body_assignment_cap != first.cross_body_assignment_cap
                        || election.candidate_snapshot != first.candidate_snapshot
                })
            {
                return Err(ParliamentReducerErrorV1::InvalidAssignmentPlan);
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
            let decision_mode = self
                .required_bodies
                .iter()
                .find(|required| required.body == body.instance.body)
                .map(|required| required.decision_mode)
                .ok_or(ParliamentReducerErrorV1::InvalidRequiredBodyPipeline)?;
            if body
                .public_finding_endorsements
                .iter()
                .any(|(assignment_id, result_root)| {
                    root_is_zero(result_root)
                        || body.excluded_assignments.contains(assignment_id)
                        || !body
                            .assignments
                            .iter()
                            .any(|assignment| assignment.assignment_id == *assignment_id)
                })
            {
                return Err(ParliamentReducerErrorV1::InvalidRoster);
            }
            match decision_mode {
                ParliamentDecisionModeV1::HiddenBindingBallot => {
                    if !body.public_finding_endorsements.is_empty()
                        || body.public_finding_binding.is_some()
                        || body.public_finding_opened_at_height.is_some()
                        || body.public_finding_phase_blocks.is_some()
                        || body.public_finding_deadline_height.is_some()
                        || body.public_finding_no_result_kind.is_some()
                        || body.public_finding_no_result_height.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                ParliamentDecisionModeV1::PublicFinding => {
                    if body.ballot_binding.is_some() {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                    let quorum = parliament_quorum_seats_v1(body.instance.original_seats);
                    let quorum_usize = usize::try_from(quorum)
                        .map_err(|_| ParliamentReducerErrorV1::InvalidRoster)?;
                    let mut endorsements_by_root = BTreeMap::<[u8; 32], Vec<AssignmentId>>::new();
                    for (assignment_id, result_root) in &body.public_finding_endorsements {
                        endorsements_by_root
                            .entry(*result_root)
                            .or_default()
                            .push(*assignment_id);
                    }
                    match body.public_finding_binding.as_ref() {
                        None => {
                            if endorsements_by_root
                                .values()
                                .any(|endorsers| endorsers.len() >= quorum_usize)
                                || body.result_root.is_some()
                                || body.result_height.is_some()
                            {
                                return Err(ParliamentReducerErrorV1::IncompleteCertificate);
                            }
                            let quorum_unreachable = public_finding_quorum_is_unreachable(body)?;
                            match body.public_finding_no_result_kind {
                                Some(ParliamentNoResultKindV1::PublicFindingQuorumUnreachable) => {
                                    if !quorum_unreachable
                                        || body.instance.status != BodyInstanceStatusV1::NoResult
                                        || self.attempt.status
                                            != GovernanceAttemptStatusV1::Rejected
                                        || body.public_finding_no_result_height.is_none()
                                    {
                                        return Err(
                                            ParliamentReducerErrorV1::PublicFindingFailureKindMismatch,
                                        );
                                    }
                                }
                                Some(ParliamentNoResultKindV1::PublicFindingDeadlineExpired) => {
                                    if quorum_unreachable
                                        || body.instance.status != BodyInstanceStatusV1::NoResult
                                        || self.attempt.status
                                            != GovernanceAttemptStatusV1::Rejected
                                        || body.public_finding_no_result_height.is_none()
                                    {
                                        return Err(
                                            ParliamentReducerErrorV1::PublicFindingFailureKindMismatch,
                                        );
                                    }
                                }
                                Some(_) => {
                                    return Err(
                                        ParliamentReducerErrorV1::PublicFindingFailureKindMismatch,
                                    );
                                }
                                None => {
                                    if quorum_unreachable
                                        || body.instance.status == BodyInstanceStatusV1::NoResult
                                        || body.public_finding_no_result_height.is_some()
                                    {
                                        return Err(
                                            ParliamentReducerErrorV1::PublicFindingFailureKindMismatch,
                                        );
                                    }
                                }
                            }
                        }
                        Some(binding) => {
                            let result_root = body
                                .result_root
                                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
                            let endorsers = endorsements_by_root
                                .get(&result_root)
                                .ok_or(ParliamentReducerErrorV1::CertificateBindingMismatch)?;
                            let endorsements = u32::try_from(endorsers.len())
                                .map_err(|_| ParliamentReducerErrorV1::InvalidRoster)?;
                            if body.instance.status != BodyInstanceStatusV1::Approved
                                || binding.quorum != quorum
                                || binding.endorsements != endorsements
                                || endorsements != quorum
                                || binding.endorsing_assignments.as_slice() != endorsers.as_slice()
                                || body.public_finding_no_result_kind.is_some()
                                || body.public_finding_no_result_height.is_some()
                                || binding.endorsement_root
                                    != parliament_public_finding_endorsement_root_v1(
                                        self.attempt.id,
                                        body.instance.id,
                                        result_root,
                                        endorsers,
                                    )
                            {
                                return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                            }
                        }
                    }
                }
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
            if decision_mode == ParliamentDecisionModeV1::PublicFinding {
                if body
                    .public_finding_no_result_height
                    .is_some_and(|height| height <= election.attempt.request.pulse_height)
                {
                    return Err(ParliamentReducerErrorV1::InvalidPublicFindingSchedule);
                }
                let schedule = match (
                    body.public_finding_opened_at_height,
                    body.public_finding_phase_blocks,
                    body.public_finding_deadline_height,
                ) {
                    (None, None, None) => None,
                    (Some(opened_at), Some(phase_blocks), Some(deadline))
                        if phase_blocks != 0
                            && opened_at > election.attempt.request.pulse_height
                            && opened_at.checked_add(phase_blocks) == Some(deadline) =>
                    {
                        Some((opened_at, deadline))
                    }
                    _ => return Err(ParliamentReducerErrorV1::InvalidPublicFindingSchedule),
                };
                let schedule_required = !body.public_finding_endorsements.is_empty()
                    || body.public_finding_binding.is_some()
                    || body.instance.status
                        == BodyInstanceStatusV1::Deliberating(DeliberationPhaseV1::Reflection)
                    || body.public_finding_no_result_kind
                        == Some(ParliamentNoResultKindV1::PublicFindingDeadlineExpired);
                if schedule_required && schedule.is_none() {
                    return Err(ParliamentReducerErrorV1::InvalidPublicFindingSchedule);
                }
                if schedule.is_some()
                    && !matches!(
                        body.instance.status,
                        BodyInstanceStatusV1::Deliberating(DeliberationPhaseV1::Reflection)
                            | BodyInstanceStatusV1::Approved
                            | BodyInstanceStatusV1::NoResult
                    )
                {
                    return Err(ParliamentReducerErrorV1::InvalidPublicFindingSchedule);
                }
                if let Some((opened_at, deadline)) = schedule {
                    if body
                        .result_height
                        .is_some_and(|height| height < opened_at || height > deadline)
                    {
                        return Err(ParliamentReducerErrorV1::InvalidCertificateHeight);
                    }
                    match body.public_finding_no_result_kind {
                        Some(ParliamentNoResultKindV1::PublicFindingDeadlineExpired)
                            if body
                                .public_finding_no_result_height
                                .is_none_or(|height| height <= deadline) =>
                        {
                            return Err(ParliamentReducerErrorV1::PublicFindingFailureKindMismatch);
                        }
                        Some(ParliamentNoResultKindV1::PublicFindingQuorumUnreachable)
                            if body
                                .public_finding_no_result_height
                                .is_none_or(|height| height < opened_at || height > deadline) =>
                        {
                            return Err(ParliamentReducerErrorV1::PublicFindingFailureKindMismatch);
                        }
                        _ => {}
                    }
                }
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
        let mut frozen_ballot_policy = None;
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
            let policy = ballot_policy(ballot);
            if frozen_ballot_policy.is_some_and(|expected| expected != policy) {
                return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
            }
            frozen_ballot_policy = Some(policy);
            let (
                registration_close_height,
                survivor_freeze_height,
                commitment_close_height,
                expected_release_height,
                opening_deadline_height,
            ) = timed_ballot_schedule(ballot.registered_at_height, policy)?;
            if ballot.registration_close_height != registration_close_height
                || ballot.survivor_freeze_height != survivor_freeze_height
                || ballot.commitment_close_height != commitment_close_height
                || ballot.release_height != Some(expected_release_height)
                || ballot.opening_deadline_height != opening_deadline_height
            {
                return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
            }
            if ballot.attempt.sequence > ballot.max_ballot_retries {
                return Err(ParliamentReducerErrorV1::BallotRetryLimitExceeded);
            }
            let body = self
                .bodies
                .get(&ballot.attempt.body_instance_id)
                .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
            if ballot.attempt.original_seats != body.instance.original_seats
                || ballot.max_corpus_entries < ballot.attempt.original_seats
            {
                return Err(ParliamentReducerErrorV1::InvalidBallotCount);
            }
            let election = self
                .elections
                .get(&body.instance.election_attempt_id)
                .ok_or(ParliamentReducerErrorV1::ImmutableBindingMismatch)?;
            if election
                .invitation_close_height
                .is_none_or(|close_height| ballot.registered_at_height <= close_height)
            {
                return Err(ParliamentReducerErrorV1::InvalidBallotSchedule);
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
                if registered > ballot.attempt.original_seats.saturating_sub(excluded)
                    || registered > ballot.max_corpus_entries
                {
                    return Err(ParliamentReducerErrorV1::InvalidBallotCount);
                }
            }
            if let Some(survivors) = ballot.survivors
                && (survivors == 0
                    || survivors > ballot.registered_voters.unwrap_or(0)
                    || survivors > ballot.max_corpus_entries)
            {
                return Err(ParliamentReducerErrorV1::InvalidBallotCount);
            }
            if let Some(accepted) = ballot.accepted_ballots {
                if accepted > ballot.registered_voters.unwrap_or(0)
                    || accepted > ballot.max_corpus_entries
                    || ballot.survivors != Some(accepted)
                {
                    return Err(ParliamentReducerErrorV1::InvalidBallotCount);
                }
            }
            let terminal_failure = matches!(
                ballot.attempt.status,
                BallotAttemptStatusV1::NoResult | BallotAttemptStatusV1::Superseded
            );
            if terminal_failure {
                if !ballot_failure_matches_state(self.attempt.id, *id, ballot) {
                    return Err(ParliamentReducerErrorV1::BallotFailureKindMismatch);
                }
            } else if ballot.failure_root.is_some()
                || ballot.failure_kind.is_some()
                || ballot.failure_height.is_some()
            {
                return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
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
                        || ballot.registration_closed_at_height.is_some()
                        || ballot.survivors_frozen_at_height.is_some()
                        || ballot.commitment_closed_at_height.is_some()
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
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::SurvivorFreeze => {
                    if ballot.registration_closed_at_height
                        != Some(ballot.registration_close_height)
                        || ballot.survivors_frozen_at_height.is_some()
                        || ballot.commitment_closed_at_height.is_some()
                        || ballot.registration_root.is_none()
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
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::TimedCommitment => {
                    if ballot.registration_closed_at_height
                        != Some(ballot.registration_close_height)
                        || ballot.survivors_frozen_at_height != Some(ballot.survivor_freeze_height)
                        || ballot.commitment_closed_at_height.is_some()
                        || ballot.registration_root.is_none()
                        || ballot.registered_voters.is_none()
                        || ballot.dropout_root.is_none()
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
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::AwaitingRelease => {
                    if ballot.registration_closed_at_height
                        != Some(ballot.registration_close_height)
                        || ballot.survivors_frozen_at_height != Some(ballot.survivor_freeze_height)
                        || !timed_commitment_completed_in_window(ballot)
                        || ballot.corpus_root.is_none()
                        || ballot.accepted_ballots.is_none()
                        || ballot.timed_commitment_root.is_none()
                        || ballot.tle_session_id.is_none()
                        || ballot.tle_key_session_id.is_none()
                        || ballot.release_height.is_none()
                        || ballot.release_beacon_session_id.is_none()
                        || ballot.release_pulse_id.is_some()
                        || ballot.opening_height.is_some()
                        || ballot.opening_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::Opening => {
                    if ballot.registration_closed_at_height
                        != Some(ballot.registration_close_height)
                        || ballot.survivors_frozen_at_height != Some(ballot.survivor_freeze_height)
                        || !timed_commitment_completed_in_window(ballot)
                        || ballot.release_pulse_id.is_none()
                        || ballot.opening_height.is_none()
                        || ballot.opening_height < ballot.release_height
                        || ballot.opening_height > Some(ballot.opening_deadline_height)
                        || ballot.opening_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::ImmutableBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::Finalized => {
                    if ballot.registration_closed_at_height
                        != Some(ballot.registration_close_height)
                        || ballot.survivors_frozen_at_height != Some(ballot.survivor_freeze_height)
                        || !timed_commitment_completed_in_window(ballot)
                        || ballot.opening_root.is_none()
                        || ballot.opening_height.is_none()
                        || ballot.opening_height < ballot.release_height
                        || ballot.opening_height > Some(ballot.opening_deadline_height)
                        || body.ballot_binding.is_none()
                        || ballot.tally.is_none()
                        || ballot.outcome.is_none()
                        || body.result_height.is_none_or(|height| {
                            ballot
                                .opening_height
                                .is_none_or(|opening_height| height < opening_height)
                        })
                        || body
                            .result_height
                            .is_none_or(|height| height > ballot.opening_deadline_height)
                    {
                        return Err(ParliamentReducerErrorV1::IncompleteCertificate);
                    }
                    let tally = ballot
                        .tally
                        .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
                    let outcome = ballot
                        .outcome
                        .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
                    let opening_root = ballot
                        .opening_root
                        .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
                    let result_height = body
                        .result_height
                        .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?;
                    if tally
                        .decision()
                        .map_err(|_| ParliamentReducerErrorV1::InvalidTally)?
                        != outcome
                        || body.result_root
                            != Some(parliament_ballot_result_root_v1(
                                self.attempt.id,
                                body.instance.id,
                                *id,
                                opening_root,
                                tally,
                                outcome,
                                result_height,
                            ))
                        || self.build_ballot_binding(*id)?
                            != body
                                .ballot_binding
                                .ok_or(ParliamentReducerErrorV1::IncompleteCertificate)?
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                BallotAttemptStatusV1::NoResult => {
                    // The exact terminal phase and frozen field set were checked above.
                }
                BallotAttemptStatusV1::Superseded => {
                    // Supersession preserves the exact validated no-result transcript.
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
            let mut previous_failure_height = None;
            for id in sequences.values() {
                let ballot = self
                    .ballots
                    .get(id)
                    .ok_or(ParliamentReducerErrorV1::RetrySequenceMismatch)?;
                let status = ballot.attempt.status;
                if (*id == latest_id && status == BallotAttemptStatusV1::Superseded)
                    || (*id != latest_id && status != BallotAttemptStatusV1::Superseded)
                    || previous_failure_height
                        .is_some_and(|failure_height| ballot.registered_at_height < failure_height)
                {
                    return Err(ParliamentReducerErrorV1::RetrySequenceMismatch);
                }
                previous_failure_height = ballot.failure_height;
            }
            let latest = self
                .ballots
                .get(&latest_id)
                .ok_or(ParliamentReducerErrorV1::RetrySequenceMismatch)?;
            if latest.attempt.status == BallotAttemptStatusV1::NoResult {
                let retry_budget_exhausted = latest.attempt.sequence == latest.max_ballot_retries;
                if retry_budget_exhausted
                    != (self.attempt.status == GovernanceAttemptStatusV1::Rejected)
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
            let rebuilt = self.build_body_binding(binding.body_instance_id)?;
            if binding.body != *body_role || &rebuilt != binding {
                return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
            }
        }
        let policy_binding = self.body_bindings.get(&ParliamentBody::PolicyJury);
        let requires_confirmation = policy_binding
            .and_then(|binding| binding.ballot)
            .map(|policy_ballot| {
                policy_ballot
                    .tally
                    .requires_confirmation()
                    .map_err(|_| ParliamentReducerErrorV1::InvalidTally)
            })
            .transpose()?
            .unwrap_or(false);
        if requires_confirmation != confirmation_required {
            return Err(ParliamentReducerErrorV1::IncompleteCertificate);
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
                    if self.terminal_height != Some(certificate.enact_at_height)
                        || self.superseding_head.is_some()
                        || self.execution_failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                GovernanceAttemptStatusV1::Superseded => {
                    if self.terminal_height != Some(certificate.enact_at_height)
                        || self
                            .superseding_head
                            .is_none_or(|head| head == certificate.expected_head)
                        || self.execution_failure_root.is_some()
                    {
                        return Err(ParliamentReducerErrorV1::CertificateBindingMismatch);
                    }
                }
                GovernanceAttemptStatusV1::ExecutionFailed => {
                    if self.terminal_height != Some(certificate.enact_at_height)
                        || self.superseding_head.is_some()
                        || self.execution_failure_root
                            != Some(parliament_execution_failure_root_v1(
                                certificate,
                                certificate.enact_at_height,
                            ))
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

#[cfg(any(test, feature = "iroha-core-tests"))]
fn enacted_fixture_governance(
    requirements: &[RequiredParliamentBodyV1],
) -> iroha_config::parameters::actual::Governance {
    let mut governance = iroha_config::parameters::actual::Governance {
        parliament_alternate_size: Some(0),
        ..iroha_config::parameters::actual::Governance::default()
    };
    for requirement in requirements {
        match requirement.body {
            ParliamentBody::RulesCommittee => governance.rules_committee_size = 3,
            ParliamentBody::AgendaCouncil => governance.agenda_council_size = 3,
            ParliamentBody::InterestPanel => governance.interest_panel_size = 3,
            ParliamentBody::ReviewPanel => governance.review_panel_size = 3,
            ParliamentBody::CoordinationCouncil => governance.coordination_council_size = 3,
            ParliamentBody::MpcCommittee => governance.mpc_committee_size = 3,
            ParliamentBody::FmaCommittee => governance.fma_committee_size = 3,
            ParliamentBody::OversightCommittee => governance.oversight_committee_size = 3,
            ParliamentBody::PolicyJury => governance.policy_jury_size = 3,
            ParliamentBody::ConfirmationJury => governance.confirmation_jury_size = 3,
        }
    }
    governance
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn complete_enacted_fixture_body(
    attempt: &mut ParliamentAttemptStateV1,
    requirement: RequiredParliamentBodyV1,
    election_attempt_id: BodyElectionAttemptId,
    result_tag: u8,
) {
    let governance_attempt_id = attempt.attempt().id;
    attempt
        .begin_invitation_acceptance(governance_attempt_id, election_attempt_id, 2, 1)
        .expect("open enacted-attempt fixture invitation window");
    let members = attempt
        .election(&election_attempt_id)
        .expect("drawn enacted-attempt fixture election")
        .primary_assignments()
        .iter()
        .map(|assignment| assignment.member.clone())
        .collect::<Vec<_>>();
    for member in &members {
        attempt
            .record_invitation_response(governance_attempt_id, election_attempt_id, member, true, 2)
            .expect("accept enacted-attempt fixture invitation");
    }
    let body_instance_id = attempt
        .seal_body_roster(governance_attempt_id, election_attempt_id, 3)
        .expect("seal enacted-attempt fixture roster");
    let mut phases = vec![
        DeliberationPhaseV1::Orientation,
        DeliberationPhaseV1::Evidence,
        DeliberationPhaseV1::Questions,
        DeliberationPhaseV1::Responses,
        DeliberationPhaseV1::Deliberation,
        DeliberationPhaseV1::Reflection,
    ];
    if requirement.decision_mode == ParliamentDecisionModeV1::HiddenBindingBallot {
        phases.push(DeliberationPhaseV1::Vote);
    }
    for phase in phases {
        attempt
            .advance_body_phase(governance_attempt_id, body_instance_id, phase, 3, 1)
            .expect("advance enacted-attempt fixture deliberation");
    }
    match requirement.decision_mode {
        ParliamentDecisionModeV1::PublicFinding => {
            let result_root = [result_tag.max(1); 32];
            let mut finalized = false;
            for member in &members {
                finalized = attempt
                    .endorse_public_finding(
                        governance_attempt_id,
                        body_instance_id,
                        result_root,
                        member,
                        3,
                    )
                    .expect("endorse enacted-attempt fixture public finding");
                if finalized {
                    break;
                }
            }
            assert!(
                finalized,
                "fixture seats must reach the public-finding quorum"
            );
        }
        ParliamentDecisionModeV1::HiddenBindingBallot => {
            let root = |tag: u8| [tag.max(1); 32];
            let ballot_attempt_id = BallotAttemptId::derive_v1(body_instance_id, 0);
            let release_beacon_session_id = BeaconSessionId::new(root(0xD0));
            let tle_key_session_id = TleKeySessionId::new(root(0xD1));
            let release_height = 12;
            let tle_session_id = TleSessionId::derive_v1(
                ballot_attempt_id,
                tle_key_session_id,
                release_beacon_session_id,
                release_height,
            );
            attempt
                .register_ballot_attempt(
                    governance_attempt_id,
                    body_instance_id,
                    ballot_attempt_id,
                    0,
                    tle_session_id,
                    tle_key_session_id,
                    release_beacon_session_id,
                    3,
                    ParliamentTimedOvn {
                        registration_phase_blocks: 4,
                        survivor_freeze_phase_blocks: 3,
                        commitment_phase_blocks: 1,
                        release_delay_blocks: 1,
                        opening_phase_blocks: 1,
                        max_ballot_retries: 2,
                        max_corpus_entries: 3,
                    },
                    release_height,
                )
                .expect("register enacted-attempt fixture ballot");
            let registration_root = root(0xD2);
            let dropout_root = root(0xD3);
            let survivor_root = root(0xD4);
            let no_recovery_root = root(0xD5);
            let corpus_root = root(0xD6);
            let timed_commitment_root = root(0xD7);
            attempt
                .close_ballot_registration(
                    governance_attempt_id,
                    ballot_attempt_id,
                    registration_root,
                    3,
                    7,
                )
                .expect("close enacted-attempt fixture ballot registration");
            attempt
                .freeze_ballot_survivors(
                    governance_attempt_id,
                    ballot_attempt_id,
                    dropout_root,
                    survivor_root,
                    3,
                    no_recovery_root,
                    10,
                )
                .expect("freeze enacted-attempt fixture ballot survivors");
            attempt
                .freeze_timed_ovn_corpus(
                    governance_attempt_id,
                    ballot_attempt_id,
                    corpus_root,
                    survivor_root,
                    3,
                    timed_commitment_root,
                    11,
                )
                .expect("freeze enacted-attempt fixture timed corpus");
            attempt
                .begin_ballot_opening_batch(
                    governance_attempt_id,
                    vec![ballot_attempt_id],
                    release_beacon_session_id,
                    release_height,
                    release_height,
                    BeaconPulseId::new(root(0xD8)),
                )
                .expect("open enacted-attempt fixture ballot");
            let outcome = attempt
                .finalize_opened_ballot(
                    governance_attempt_id,
                    ballot_attempt_id,
                    corpus_root,
                    no_recovery_root,
                    tle_session_id,
                    root(0xD9),
                    3,
                    ParliamentAggregateTallyV1 {
                        original_seats: 3,
                        accepted_ballots: 3,
                        aye: 2,
                        nay: 1,
                        abstain: 0,
                    },
                    release_height,
                )
                .expect("finalize enacted-attempt fixture ballot");
            assert_eq!(outcome, ParliamentAggregateOutcomeV1::Approved);
        }
    }
}

/// Build one complete, proposal-bound enacted Parliament attempt for integration fixtures.
///
/// This helper is available only to Core's explicit test corridor. It deliberately exercises the
/// reducer instead of manufacturing certificate-only compatibility state.
#[cfg(any(test, feature = "iroha-core-tests"))]
#[doc(hidden)]
pub fn enacted_parliament_attempt_for_testing(
    proposal: &ProposalKind,
    mut candidates: Vec<AccountId>,
    network_id: &NetworkId,
    enact_at_height: u64,
) -> ParliamentAttemptStateV1 {
    assert!(
        enact_at_height > 9,
        "fixture enactment must follow the complete reducer transcript"
    );
    candidates.sort_unstable();
    candidates.dedup();
    assert!(candidates.len() >= 3, "fixture requires three candidates");
    let proposal_content_id = ProposalContentId::new(proposal.fingerprint());
    let governance_attempt_id = GovernanceAttemptId::derive_v1(proposal_content_id, 0);
    let (risk_tier, requirements) = parliament_attempt_policy_v1(proposal);
    let mut attempt = ParliamentAttemptStateV1::try_new(
        GovernanceAttemptV1 {
            id: governance_attempt_id,
            proposal_content_id,
            sequence: 0,
            risk_tier,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        },
        PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
        proposal.effect_preimage_hash_v1(),
        GovernanceExpectedHeadV1::Absent(
            iroha_data_model::governance::types::GovernanceExpectedHeadAbsentV1 {
                subject_id: proposal
                    .governed_subject_id_v1()
                    .expect("derive fixture proposal subject"),
            },
        ),
        requirements.clone(),
    )
    .expect("create proposal-bound enacted-attempt fixture");
    attempt
        .complete_qualification(governance_attempt_id)
        .expect("complete enacted-attempt fixture qualification");
    let candidate_count = u32::try_from(candidates.len()).expect("candidate count fits u32");
    let sortition_session = BeaconSessionId::new([0xB0; 32]);
    let mut request_ids = Vec::with_capacity(requirements.len());
    for requirement in &requirements {
        let election_attempt_id =
            BodyElectionAttemptId::derive_v1(governance_attempt_id, requirement.body, 0);
        let request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            requirement.body,
            parliament_candidate_root_v1(governance_attempt_id, requirement.body, &candidates),
            candidate_count,
            3,
            1,
            2,
            sortition_session,
            None,
        )
        .expect("construct enacted-attempt fixture sortition request");
        request_ids.push(request.id);
        attempt
            .register_sortition_request(governance_attempt_id, 0, request, candidates.clone())
            .expect("register enacted-attempt fixture sortition request");
    }
    request_ids.sort_unstable();
    let sortition_pulse_id = BeaconPulseId::new([0xB1; 32]);
    attempt
        .consume_sortition_pulse_batch(
            governance_attempt_id,
            request_ids,
            sortition_session,
            2,
            sortition_pulse_id,
            *sortition_pulse_id.as_bytes(),
            network_id,
            &enacted_fixture_governance(&requirements),
        )
        .expect("consume enacted-attempt fixture sortition pulse");
    for (index, requirement) in requirements.iter().copied().enumerate() {
        complete_enacted_fixture_body(
            &mut attempt,
            requirement,
            BodyElectionAttemptId::derive_v1(governance_attempt_id, requirement.body, 0),
            0xC0_u8
                .checked_add(u8::try_from(index).expect("body index fits u8"))
                .expect("result tag does not overflow"),
        );
    }
    attempt
        .construct_certificate(governance_attempt_id, enact_at_height - 1, enact_at_height)
        .expect("construct enacted-attempt fixture certificate");
    attempt
        .mark_enacted(governance_attempt_id, enact_at_height)
        .expect("mark enacted-attempt fixture enacted");
    attempt
        .validate_proposal_bindings_v1(proposal)
        .expect("enacted-attempt fixture retains exact proposal bindings");
    attempt
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use iroha_crypto::{
        Algorithm, Hash, HashOf, KeyPair,
        threshold_bls::{
            AdaptiveThresholdBlsParameters, DasRenDealerSecret, ThresholdBlsSession,
            TleReleasePurpose,
        },
        timed_ovn::TimedOvnRegistrationSecretV1,
    };
    use iroha_data_model::{
        block::BlockHeader,
        governance::types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal,
            GovernanceExpectedHeadAbsentV1, parliament_ballot_participant_hash_v1,
        },
    };
    use rand::{SeedableRng as _, rngs::StdRng};

    use crate::{
        governance::timed_ovn::{
            TimedOvnLifecyclePhaseV1, TimedOvnLifecycleStateV1, TimedOvnSessionPublicV1,
            timed_ovn_parameter_hash_v1,
        },
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
        tle_release::{
            ParliamentTimedOvnCastingPhaseV1, TimedOvnCastingAuthorizationErrorV1,
            ValidatedTleKeySessionV1, authorize_parliament_timed_ovn_casting_context_v1,
            derive_parliament_timed_ovn_casting_snapshot_v1,
        },
    };

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

    fn casting_tle_key(network_id: [u8; 32], tag: u8) -> ValidatedTleKeySessionV1 {
        let threshold_session = ThresholdBlsSession::<TleReleasePurpose>::new(
            network_id,
            root(tag),
            root(tag.wrapping_add(1)),
            4,
            2,
        )
        .expect("casting fixture threshold session");
        let parameters =
            AdaptiveThresholdBlsParameters::derive(&threshold_session).expect("parameters");
        let mut rng = StdRng::from_seed([tag.wrapping_add(2); 32]);
        let dealers = (1_u16..=3)
            .map(|index| {
                DasRenDealerSecret::generate_with_rng(&parameters, index, &mut rng)
                    .expect("dealer")
                    .1
            })
            .collect::<Vec<_>>();
        ValidatedTleKeySessionV1::from_qualified_dealers(
            threshold_session,
            &dealers,
            &[1, 2, 3],
            root(tag.wrapping_add(3)),
        )
        .expect("casting fixture TLE session")
    }

    fn casting_state_at_height(
        attempt: ParliamentAttemptStateV1,
        lifecycle: TimedOvnLifecycleStateV1,
        tle_key: Option<&ValidatedTleKeySessionV1>,
        stored_key: Option<TleKeySessionId>,
        height: u64,
    ) -> State {
        let mut world = World::new();
        world
            .parliament_attempts
            .insert(attempt.attempt().id, attempt);
        if let (Some(key), Some(storage_id)) = (tle_key, stored_key) {
            world
                .tle_key_sessions
                .insert(storage_id, key.public_state().clone());
        }
        world.timed_ovn_evidence.insert(
            BallotAttemptId::new(lifecycle.ballot_attempt_id()),
            lifecycle,
        );
        let mut state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        for index in 0..height {
            state.push_block_hash_for_testing(HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::new(index.to_be_bytes()),
            ));
        }
        state
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

    fn deploy_contract_proposal() -> ProposalKind {
        ProposalKind::DeployContract(DeployContractProposal {
            contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse()
                .expect("canonical contract address"),
            code_hash: ContractCodeHash::new(root(41)),
            abi_hash: ContractAbiHash::new(root(42)),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        })
    }

    fn proposal_bound_state(proposal: &ProposalKind) -> ParliamentAttemptStateV1 {
        let proposal_content_id = ProposalContentId::new(proposal.fingerprint());
        let (risk_tier, required_bodies) = parliament_attempt_policy_v1(proposal);
        ParliamentAttemptStateV1::try_new(
            GovernanceAttemptV1 {
                id: GovernanceAttemptId::derive_v1(proposal_content_id, 0),
                proposal_content_id,
                sequence: 0,
                risk_tier,
                stage: GovernanceStageV1::Qualification,
                status: GovernanceAttemptStatusV1::Active,
            },
            PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
            proposal.effect_preimage_hash_v1(),
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: proposal
                    .governed_subject_id_v1()
                    .expect("derive proposal subject"),
            }),
            required_bodies,
        )
        .expect("canonical proposal-bound reducer state")
    }

    #[test]
    fn proposal_binding_validation_rejects_weakened_persisted_policy() {
        let proposal = deploy_contract_proposal();
        let state = proposal_bound_state(&proposal);
        state
            .validate_proposal_bindings_v1(&proposal)
            .expect("canonical proposal bindings");

        let mut escalated = state.clone();
        escalated.attempt.risk_tier = RiskTierV1::Constitutional;
        escalated
            .validate_proposal_bindings_v1(&proposal)
            .expect("upward-only risk escalation remains valid");

        let mut downgraded = state.clone();
        downgraded.attempt.risk_tier = RiskTierV1::Routine;
        assert_eq!(
            downgraded.validate_proposal_bindings_v1(&proposal),
            Err(ParliamentReducerErrorV1::ProposalBindingMismatch)
        );

        let mut substituted_effect = state.clone();
        substituted_effect.effect_preimage_hash = root(99);
        substituted_effect
            .validate()
            .expect("an internally valid effect still needs its proposal binding");
        assert_eq!(
            substituted_effect.validate_proposal_bindings_v1(&proposal),
            Err(ParliamentReducerErrorV1::ProposalBindingMismatch)
        );

        let mut substituted_subject = state.clone();
        substituted_subject.expected_head =
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: root(100),
            });
        substituted_subject
            .validate()
            .expect("an internally valid subject still needs its proposal binding");
        assert_eq!(
            substituted_subject.validate_proposal_bindings_v1(&proposal),
            Err(ParliamentReducerErrorV1::ProposalBindingMismatch)
        );

        let mut weakened_pipeline = state;
        weakened_pipeline.required_bodies.remove(0);
        weakened_pipeline
            .validate()
            .expect("an ordered subset still needs the proposal's exact base policy");
        assert_eq!(
            weakened_pipeline.validate_proposal_bindings_v1(&proposal),
            Err(ParliamentReducerErrorV1::ProposalBindingMismatch)
        );
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
                .advance_body_phase(attempt_id, body_id, phase, 22, 10)
                .expect("advance one exact deliberation phase");
        }
    }

    fn timed_ovn_policy() -> ParliamentTimedOvn {
        ParliamentTimedOvn {
            registration_phase_blocks: 4,
            survivor_freeze_phase_blocks: 3,
            commitment_phase_blocks: 2,
            release_delay_blocks: 4,
            opening_phase_blocks: 2,
            max_ballot_retries: 2,
            max_corpus_entries: 3,
        }
    }

    #[test]
    fn timed_ovn_schedule_reserves_one_maximum_chunk_block_per_corpus_slice() {
        let maximum_policy = ParliamentTimedOvn {
            registration_phase_blocks: 1_001,
            survivor_freeze_phase_blocks: 1_000,
            commitment_phase_blocks: 32,
            max_corpus_entries: 1_000,
            ..timed_ovn_policy()
        };
        assert!(timed_ballot_schedule(10, maximum_policy).is_ok());
        assert_eq!(
            timed_ballot_schedule(
                10,
                ParliamentTimedOvn {
                    commitment_phase_blocks: 31,
                    ..maximum_policy
                },
            ),
            Err(ParliamentReducerErrorV1::InvalidBallotSchedule)
        );
        assert_eq!(
            timed_ballot_schedule(
                10,
                ParliamentTimedOvn {
                    registration_phase_blocks: 1_000,
                    ..maximum_policy
                },
            ),
            Err(ParliamentReducerErrorV1::InvalidBallotSchedule)
        );
        assert_eq!(
            timed_ballot_schedule(
                10,
                ParliamentTimedOvn {
                    survivor_freeze_phase_blocks: 999,
                    ..maximum_policy
                },
            ),
            Err(ParliamentReducerErrorV1::InvalidBallotSchedule)
        );
    }

    #[test]
    fn beacon_demand_tracks_sortition_timeout_and_frozen_ballot_release_slot() {
        let mut sortition = policy_only_state();
        let governance_attempt_id = sortition.attempt.id;
        sortition
            .complete_qualification(governance_attempt_id)
            .expect("enter Policy Jury stage");
        let session_id = beacon_session(81);
        let (request, candidate_snapshot) = sortition_request(
            governance_attempt_id,
            0,
            ParliamentBody::PolicyJury,
            82,
            3,
            3,
            10,
            20,
            session_id,
            None,
        );
        let election_attempt_id = request.body_election_attempt_id;
        sortition
            .register_sortition_request(governance_attempt_id, 0, request, candidate_snapshot)
            .expect("register immutable sortition pulse slot");
        assert!(sortition.requires_beacon_pulse_at(session_id, 20));
        assert!(!sortition.requires_beacon_pulse_at(beacon_session(83), 20));
        assert_eq!(
            sortition.fail_body_election_no_roster(governance_attempt_id, election_attempt_id, 20,),
            Err(ParliamentReducerErrorV1::SortitionPulseStillPending)
        );
        sortition
            .fail_body_election_no_roster(governance_attempt_id, election_attempt_id, 21)
            .expect("missing sortition pulse becomes an objective retryable failure");
        assert!(!sortition.requires_beacon_pulse_at(session_id, 20));

        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        advance_to_vote(&mut state, body_id);
        let governance_attempt_id = state.attempt.id;
        let ballot_id = BallotAttemptId::derive_v1(body_id, 0);
        let release_session_id = beacon_session(84);
        let release_height = 40;
        let tle_key_session_id = tle_key_session(85);
        let tle_session_id = TleSessionId::derive_v1(
            ballot_id,
            tle_key_session_id,
            release_session_id,
            release_height,
        );
        state
            .register_ballot_attempt(
                governance_attempt_id,
                body_id,
                ballot_id,
                0,
                tle_session_id,
                tle_key_session_id,
                release_session_id,
                27,
                timed_ovn_policy(),
                release_height,
            )
            .expect("register arbitrary frozen ballot release slot");
        assert!(state.requires_beacon_pulse_at(release_session_id, release_height));
        assert!(state.requires_beacon_pulse_at(release_session_id, release_height));
        assert!(!state.requires_beacon_pulse_at(release_session_id, release_height - 1));
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
        let max_corpus_entries = seats.max(accepted);
        let policy = ParliamentTimedOvn {
            registration_phase_blocks: u64::from(max_corpus_entries)
                .checked_add(1)
                .expect("fixture corpus capacity fits the height domain"),
            survivor_freeze_phase_blocks: u64::from(max_corpus_entries),
            commitment_phase_blocks: parliament_timed_ovn_required_chunk_blocks_v1(
                max_corpus_entries,
            )
            .max(2),
            max_corpus_entries,
            ..timed_ovn_policy()
        };
        let registered_at_height = 27;
        let (
            registration_close_height,
            survivor_freeze_height,
            commitment_close_height,
            release_height,
            _,
        ) = timed_ballot_schedule(registered_at_height, policy).expect("valid fixture schedule");
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
                registered_at_height,
                policy,
                release_height,
            )
            .expect("register private ballot");
        state
            .close_ballot_registration(
                attempt_id,
                ballot_id,
                root(19),
                accepted,
                registration_close_height,
            )
            .expect("freeze registration");
        state
            .freeze_ballot_survivors(
                attempt_id,
                ballot_id,
                root(21),
                root(29),
                accepted,
                root(22),
                survivor_freeze_height,
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
                commitment_close_height,
            )
            .expect("freeze complete timed OVN corpus");
        assert_eq!(
            state.begin_ballot_opening_batch(
                attempt_id,
                vec![ballot_id],
                beacon_session(24),
                release_height,
                release_height - 1,
                pulse_id(26),
            ),
            Err(ParliamentReducerErrorV1::ReleaseHeightNotReached)
        );
        state
            .begin_ballot_opening_batch(
                attempt_id,
                vec![ballot_id],
                beacon_session(24),
                release_height,
                release_height,
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
        let result_height = fixture
            .state
            .ballot(&fixture.ballot_id)
            .and_then(|ballot| ballot.opening_height)
            .expect("fixture ballot opening height");
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
                result_height,
            )
            .expect("finalize policy aggregate")
    }

    #[test]
    fn sealed_and_released_cross_store_bindings_fail_closed_on_substitution() {
        let mut fixture = opened_policy_ballot(3, 3);
        let governance_attempt_id = fixture.state.attempt.id;
        let expected_sealed = TimedOvnParliamentReducerBindingV1 {
            proposal_content_id: *fixture.state.attempt.proposal_content_id.as_bytes(),
            governance_attempt_id: *governance_attempt_id.as_bytes(),
            body_instance_id: *fixture.body_id.as_bytes(),
            ballot_attempt_id: *fixture.ballot_id.as_bytes(),
            tle_key_session_id: Some(tle_key_session(23)),
            registration_opened_at_finalized_height: None,
            release_height: Some(40),
            registration_root: Some(root(19)),
            registered_voters: Some(3),
            dropout_root: Some(root(21)),
            survivor_root: Some(root(29)),
            survivors: Some(3),
            no_recovery_root: Some(root(22)),
            corpus_root: Some(root(20)),
            accepted_ballots: Some(3),
            timed_commitment_root: Some(root(25)),
            opening_root: None,
            tally_counts: None,
        };
        assert_eq!(
            fixture.state.timed_ovn_reducer_binding(&fixture.ballot_id),
            Some(expected_sealed)
        );
        assert!(
            fixture
                .state
                .timed_ovn_reducer_binding_matches(&fixture.ballot_id, &expected_sealed)
        );

        let mut substituted_sealed = expected_sealed;
        substituted_sealed.corpus_root = Some(root(99));
        assert!(
            !fixture
                .state
                .timed_ovn_reducer_binding_matches(&fixture.ballot_id, &substituted_sealed),
            "a separately self-consistent sealed lifecycle cannot substitute its corpus root"
        );
        substituted_sealed = expected_sealed;
        substituted_sealed.timed_commitment_root = Some(root(100));
        assert!(
            !fixture
                .state
                .timed_ovn_reducer_binding_matches(&fixture.ballot_id, &substituted_sealed),
            "a separately self-consistent sealed lifecycle cannot substitute its transcript root"
        );

        assert_eq!(
            finalize_policy(&mut fixture, 2, 1, 0),
            ParliamentAggregateOutcomeV1::Approved
        );
        let expected_released = TimedOvnParliamentReducerBindingV1 {
            opening_root: Some(root(27)),
            tally_counts: Some([2, 1, 0]),
            ..expected_sealed
        };
        assert_eq!(
            fixture.state.timed_ovn_reducer_binding(&fixture.ballot_id),
            Some(expected_released)
        );
        assert!(
            fixture
                .state
                .timed_ovn_reducer_binding_matches(&fixture.ballot_id, &expected_released)
        );

        let mut substituted_released = expected_released;
        substituted_released.opening_root = Some(root(101));
        assert!(
            !fixture
                .state
                .timed_ovn_reducer_binding_matches(&fixture.ballot_id, &substituted_released),
            "a separately self-consistent released lifecycle cannot substitute its opening root"
        );
        substituted_released = expected_released;
        substituted_released.tally_counts = Some([1, 2, 0]);
        assert!(
            !fixture
                .state
                .timed_ovn_reducer_binding_matches(&fixture.ballot_id, &substituted_released),
            "a separately self-consistent released lifecycle cannot substitute its tally"
        );
    }

    #[test]
    fn hidden_ballot_corpus_bound_covers_every_original_seat() {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        advance_to_vote(&mut state, body_id);
        let governance_attempt_id = state.attempt.id;
        let ballot_id = BallotAttemptId::derive_v1(body_id, 0);
        let release_beacon_session_id = beacon_session(24);
        let tle_key_session_id = tle_key_session(23);
        let release_height = 40;
        let tle_session_id = TleSessionId::derive_v1(
            ballot_id,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        );
        let undersized = ParliamentTimedOvn {
            max_corpus_entries: 2,
            ..timed_ovn_policy()
        };
        assert_eq!(
            state.register_ballot_attempt(
                governance_attempt_id,
                body_id,
                ballot_id,
                0,
                tle_session_id,
                tle_key_session_id,
                release_beacon_session_id,
                27,
                undersized,
                release_height,
            ),
            Err(ParliamentReducerErrorV1::InvalidBallotSchedule)
        );

        state
            .register_ballot_attempt(
                governance_attempt_id,
                body_id,
                ballot_id,
                0,
                tle_session_id,
                tle_key_session_id,
                release_beacon_session_id,
                27,
                timed_ovn_policy(),
                release_height,
            )
            .expect("register ballot with capacity for every original seat");
        let mut registration_window_too_short = state.clone();
        registration_window_too_short
            .ballots
            .get_mut(&ballot_id)
            .expect("registered ballot")
            .registration_phase_blocks = 3;
        assert_eq!(
            registration_window_too_short.validate(),
            Err(ParliamentReducerErrorV1::InvalidBallotSchedule),
            "snapshot validation reserves one admission-slack block plus every registration slot"
        );
        let mut survivor_window_too_short = state.clone();
        survivor_window_too_short
            .ballots
            .get_mut(&ballot_id)
            .expect("registered ballot")
            .survivor_freeze_phase_blocks = 2;
        assert_eq!(
            survivor_window_too_short.validate(),
            Err(ParliamentReducerErrorV1::InvalidBallotSchedule),
            "snapshot validation reserves one authenticated dropout slot per corpus entry"
        );
        state
            .ballots
            .get_mut(&ballot_id)
            .expect("registered ballot")
            .max_corpus_entries = 2;
        assert_eq!(
            state.validate(),
            Err(ParliamentReducerErrorV1::InvalidBallotCount),
            "snapshot validation must reject an undersized persisted corpus bound"
        );
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
    fn attempt_rejects_an_inert_compare_and_set_subject() {
        let required = vec![RequiredParliamentBodyV1 {
            body: ParliamentBody::PolicyJury,
            decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
        }];
        assert_eq!(
            ParliamentAttemptStateV1::try_new(
                attempt(),
                7,
                root(3),
                GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                    subject_id: [0; 32],
                }),
                required.clone(),
            ),
            Err(ParliamentReducerErrorV1::ImmutableBindingMismatch)
        );
        assert_eq!(
            ParliamentAttemptStateV1::try_new(
                attempt(),
                7,
                root(3),
                GovernanceExpectedHeadV1::Present(
                    iroha_data_model::governance::types::GovernanceExpectedHeadPresentV1 {
                        subject_id: root(4),
                        version: 1,
                        head_root: [0; 32],
                    },
                ),
                required,
            ),
            Err(ParliamentReducerErrorV1::ImmutableBindingMismatch)
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
            let (request, candidate_snapshot) =
                sortition_request(id, 0, body, 12, 3, 3, 10, 20, beacon_session(30), None);
            request_ids.push(request.id);
            state
                .register_sortition_request(id, 0, request, candidate_snapshot)
                .expect("register simultaneous request");
            if body == ParliamentBody::InterestPanel {
                assert_eq!(
                    consume_sortition(
                        &mut state,
                        id,
                        request_ids.clone(),
                        beacon_session(30),
                        20,
                        pulse_id(31),
                    ),
                    Err(ParliamentReducerErrorV1::InvalidAssignmentPlan),
                    "the first draw must cover every initial body in one future-pulse batch"
                );
            }
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
                let result = probe.advance_body_phase(id, body_id, candidate, 22, 10);
                assert_eq!(
                    result.is_ok(),
                    candidate == expected,
                    "phase row {index:?}, candidate {candidate:?}"
                );
            }
            cursor
                .advance_body_phase(id, body_id, expected, 22, 10)
                .expect("exact next phase succeeds");
        }
        assert_eq!(
            cursor.advance_body_phase(id, body_id, DeliberationPhaseV1::Vote, 22, 10),
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
        let assignments = state.body(&body_id).expect("body").assignments().to_vec();
        let absent = assignments.first().expect("fixture has a seat");
        let other_member = &assignments
            .get(1)
            .expect("fixture has a second seat")
            .member;
        assert_eq!(
            state.record_attempt_absence(id, body_id, absent.assignment_id, other_member, 22),
            Err(ParliamentReducerErrorV1::UnauthorizedBodyMember)
        );
        state
            .record_attempt_absence(id, body_id, absent.assignment_id, &absent.member, 22)
            .expect("the exact seated member may declare their own absence");
        assert_eq!(
            state.record_attempt_absence(id, body_id, absent.assignment_id, &absent.member, 22),
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
                27,
                timed_ovn_policy(),
                release_height,
            )
            .expect("register ballot");
        assert_eq!(
            state.close_ballot_registration(id, ballot, root(51), 3, 31),
            Err(ParliamentReducerErrorV1::InvalidBallotCount)
        );
        state
            .close_ballot_registration(id, ballot, root(51), 2, 31)
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
    fn public_finding_requires_authority_bound_two_thirds_endorsement() {
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
        let attempt_id = state.attempt.id;
        state
            .complete_qualification(attempt_id)
            .expect("enter first public body");
        let mut request_ids = Vec::new();
        let mut interest_election_id = None;
        for body in [ParliamentBody::InterestPanel, ParliamentBody::PolicyJury] {
            let (request, candidate_snapshot) = sortition_request(
                attempt_id,
                0,
                body,
                12,
                3,
                3,
                10,
                20,
                beacon_session(90),
                None,
            );
            if body == ParliamentBody::InterestPanel {
                interest_election_id = Some(request.body_election_attempt_id);
            }
            request_ids.push(request.id);
            state
                .register_sortition_request(attempt_id, 0, request, candidate_snapshot)
                .expect("register simultaneous body request");
        }
        request_ids.sort_unstable();
        consume_sortition(
            &mut state,
            attempt_id,
            request_ids,
            beacon_session(90),
            20,
            pulse_id(91),
        )
        .expect("consume complete simultaneous draw");
        let election_id = interest_election_id.expect("interest election id");
        state
            .begin_invitation_acceptance(attempt_id, election_id, 20, 1)
            .expect("open interest invitations");
        let members = state
            .election(&election_id)
            .expect("interest election")
            .primary_assignments()
            .iter()
            .map(|assignment| assignment.member.clone())
            .collect::<Vec<_>>();
        for member in &members {
            state
                .record_invitation_response(attempt_id, election_id, member, true, 20)
                .expect("selected interest member accepts");
        }
        let body_id = state
            .seal_body_roster(attempt_id, election_id, 21)
            .expect("seal public body");
        for phase in [
            DeliberationPhaseV1::Orientation,
            DeliberationPhaseV1::Evidence,
            DeliberationPhaseV1::Questions,
            DeliberationPhaseV1::Responses,
            DeliberationPhaseV1::Deliberation,
            DeliberationPhaseV1::Reflection,
        ] {
            state
                .advance_body_phase(attempt_id, body_id, phase, 22, 10)
                .expect("advance public deliberation");
        }
        assert_eq!(
            state
                .body(&body_id)
                .expect("public body")
                .public_finding_deadline_height(),
            Some(32)
        );

        let mut expired = state.clone();
        assert_eq!(
            expired.fail_public_finding_no_result(attempt_id, body_id, 32),
            Err(ParliamentReducerErrorV1::PublicFindingWindowStillOpen)
        );
        assert_eq!(
            expired.endorse_public_finding(attempt_id, body_id, root(100), &members[0], 33),
            Err(ParliamentReducerErrorV1::PublicFindingWindowClosed)
        );
        expired
            .fail_public_finding_no_result(attempt_id, body_id, 33)
            .expect("the permissionless trigger closes an expired public finding");
        assert_eq!(
            expired
                .body(&body_id)
                .expect("expired public body")
                .public_finding_no_result_kind(),
            Some(ParliamentNoResultKindV1::PublicFindingDeadlineExpired)
        );
        expired
            .validate()
            .expect("deadline-expired public finding persists canonically");

        let mut irreconcilable = state.clone();
        for (member, tag) in members.iter().zip([101_u8, 102, 103]) {
            assert!(
                !irreconcilable
                    .endorse_public_finding(attempt_id, body_id, root(tag), member, 22)
                    .expect("distinct seated endorsement is accepted")
            );
        }
        assert_eq!(
            irreconcilable.attempt.status,
            GovernanceAttemptStatusV1::Rejected
        );
        assert_eq!(
            irreconcilable
                .body(&body_id)
                .expect("irreconcilable public body")
                .instance
                .status,
            BodyInstanceStatusV1::NoResult
        );
        irreconcilable
            .validate()
            .expect("a mathematically unreachable public quorum is terminal after restore");

        let mut absent_quorum = state.clone();
        for member in &members[..2] {
            absent_quorum
                .record_attempt_absence(
                    attempt_id,
                    body_id,
                    AssignmentId::derive_v1(election_id, member),
                    member,
                    22,
                )
                .expect("seated member records their own absence");
        }
        assert_eq!(
            absent_quorum.attempt.status,
            GovernanceAttemptStatusV1::Rejected
        );
        assert_eq!(
            absent_quorum
                .body(&body_id)
                .expect("absence-terminal public body")
                .instance
                .status,
            BodyInstanceStatusV1::NoResult
        );
        absent_quorum
            .validate()
            .expect("insufficient eligible public seats are terminal after restore");

        assert_eq!(
            state.endorse_public_finding(attempt_id, body_id, root(92), &account(99), 22),
            Err(ParliamentReducerErrorV1::UnauthorizedBodyMember)
        );
        assert!(
            !state
                .endorse_public_finding(attempt_id, body_id, root(92), &members[0], 22)
                .expect("first seated endorsement")
        );
        assert_eq!(
            state.endorse_public_finding(attempt_id, body_id, root(93), &members[0], 22),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance
            ))
        );
        assert!(
            !state
                .endorse_public_finding(attempt_id, body_id, root(93), &members[1], 22)
                .expect("dissenting seated endorsement")
        );
        state
            .validate()
            .expect("split sub-quorum endorsements persist canonically");
        assert!(
            state
                .endorse_public_finding(attempt_id, body_id, root(92), &members[2], 22)
                .expect("second matching endorsement reaches two-thirds")
        );
        let body = state.body(&body_id).expect("final public body");
        assert_eq!(body.result_root(), Some(root(92)));
        let binding = body
            .public_finding_binding
            .as_ref()
            .expect("quorum binding retained");
        assert_eq!(binding.endorsements, 2);
        assert_eq!(binding.quorum, 2);
        assert_eq!(binding.endorsing_assignments.len(), 2);
        state
            .validate()
            .expect("authority-bound public-finding quorum persists canonically");

        let mut forged = state.clone();
        forged
            .bodies
            .get_mut(&body_id)
            .expect("public body")
            .public_finding_binding
            .as_mut()
            .expect("public binding")
            .endorsement_root = root(94);
        assert_eq!(
            forged.validate(),
            Err(ParliamentReducerErrorV1::CertificateBindingMismatch)
        );

        let mut substituted_endorsers = state.clone();
        substituted_endorsers
            .bodies
            .get_mut(&body_id)
            .expect("public body")
            .public_finding_binding
            .as_mut()
            .expect("public binding")
            .endorsing_assignments[0] = AssignmentId::derive_v1(election_id, &members[1]);
        assert_eq!(
            substituted_endorsers.validate(),
            Err(ParliamentReducerErrorV1::CertificateBindingMismatch)
        );

        let mut surplus_endorsement = state;
        let dissenting_assignment = AssignmentId::derive_v1(election_id, &members[1]);
        let body = surplus_endorsement
            .bodies
            .get_mut(&body_id)
            .expect("public body");
        body.public_finding_endorsements
            .insert(dissenting_assignment, root(92));
        let endorsing_assignments = body
            .public_finding_endorsements
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let binding = body
            .public_finding_binding
            .as_mut()
            .expect("public binding");
        binding.endorsements = 3;
        binding.endorsing_assignments = endorsing_assignments;
        binding.endorsement_root = parliament_public_finding_endorsement_root_v1(
            attempt_id,
            body_id,
            root(92),
            &binding.endorsing_assignments,
        );
        assert_eq!(
            surplus_endorsement.validate(),
            Err(ParliamentReducerErrorV1::CertificateBindingMismatch)
        );
    }

    #[test]
    fn casting_context_authorization_replays_all_prefix_phases_and_rejects_tampering() {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        advance_to_vote(&mut state, body_id);
        let governance_attempt_id = state.attempt.id;
        let ballot_attempt_id = BallotAttemptId::derive_v1(body_id, 0);
        let network_id = network_id();
        let network_binding = *network_id.as_bytes();
        let tle_key = casting_tle_key(network_binding, 0xA0);
        let tle_key_session_id = tle_key.public_state().key_session_id;
        let release_beacon_session_id = beacon_session(0xA4);
        let release_height = 40;
        let tle_session_id = TleSessionId::derive_v1(
            ballot_attempt_id,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        );
        state
            .register_ballot_attempt(
                governance_attempt_id,
                body_id,
                ballot_attempt_id,
                0,
                tle_session_id,
                tle_key_session_id,
                release_beacon_session_id,
                27,
                timed_ovn_policy(),
                release_height,
            )
            .expect("register casting-context ballot");
        let session = TimedOvnSessionPublicV1 {
            network_id: network_binding,
            proposal_content_id: *state.proposal_content_id().as_bytes(),
            governance_attempt_id: *governance_attempt_id.as_bytes(),
            body_instance_id: *body_id.as_bytes(),
            ballot_attempt_id: *ballot_attempt_id.as_bytes(),
            parameter_hash: timed_ovn_parameter_hash_v1(),
            tle_key_session_id,
            tle_key_transcript_hash: tle_key.public_state().transcript_hash,
            tle_master_public_key: *tle_key.master_public_key().as_bytes(),
        };
        let mut lifecycle =
            TimedOvnLifecycleStateV1::open_registration(session, 27, release_height, &tle_key)
                .expect("open casting-context registration");
        let mut rng = StdRng::from_seed([0xA5; 32]);
        for assignment in state.body(&body_id).expect("fixture body").assignments() {
            let participant_hash =
                parliament_ballot_participant_hash_v1(ballot_attempt_id, &assignment.member);
            let (_, registration) = TimedOvnRegistrationSecretV1::generate_with_rng(
                &session.rebuild(&tle_key).expect("timed session"),
                participant_hash,
                &mut rng,
            )
            .expect("registration");
            lifecycle = lifecycle
                .register_participant(participant_hash, registration.to_bytes(), &tle_key)
                .expect("authenticated registration");
        }

        let registered_state = casting_state_at_height(
            state.clone(),
            lifecycle.clone(),
            Some(&tle_key),
            Some(tle_key_session_id),
            30,
        );
        let registered = authorize_parliament_timed_ovn_casting_context_v1(
            &registered_state.query_view(),
            ballot_attempt_id,
        )
        .expect("registered casting context");
        assert_eq!(
            registered.phase(),
            ParliamentTimedOvnCastingPhaseV1::Registered
        );
        assert_eq!(registered.registration_records().len(), 3);
        assert!(registered.survivor_participant_hashes().is_none());
        let registered_archive = registered.archive_v1();
        let validated_registered_archive = registered_archive
            .validate_v1()
            .expect("registered archive replays independently");
        let registered_view = registered_state.query_view();
        let (registered_snapshot, registered_bindings) =
            derive_parliament_timed_ovn_casting_snapshot_v1(registered_view.world(), 30)
                .expect("derive authenticated registered casting snapshot");
        assert_eq!(registered_snapshot.count, 1);
        assert_eq!(registered_bindings.len(), 1);
        assert!(validated_registered_archive.matches_compact_binding_v1(&registered_bindings[0]));
        assert_eq!(
            derive_parliament_timed_ovn_casting_snapshot_v1(registered_view.world(), 30)
                .expect("repeat deterministic registered casting snapshot"),
            (registered_snapshot, registered_bindings)
        );

        for stale_height in [26, 31] {
            let stale_state = casting_state_at_height(
                state.clone(),
                lifecycle.clone(),
                Some(&tle_key),
                Some(tle_key_session_id),
                stale_height,
            );
            assert_eq!(
                authorize_parliament_timed_ovn_casting_context_v1(
                    &stale_state.query_view(),
                    ballot_attempt_id,
                )
                .expect_err("out-of-window registered context must be rejected"),
                TimedOvnCastingAuthorizationErrorV1::PhaseWindowInactive
            );
        }

        let mut malformed_schedule = state.clone();
        malformed_schedule
            .ballots
            .get_mut(&ballot_attempt_id)
            .expect("casting-context ballot")
            .registration_close_height = 27;
        let malformed_schedule_state = casting_state_at_height(
            malformed_schedule,
            lifecycle.clone(),
            Some(&tle_key),
            Some(tle_key_session_id),
            30,
        );
        assert_eq!(
            authorize_parliament_timed_ovn_casting_context_v1(
                &malformed_schedule_state.query_view(),
                ballot_attempt_id,
            )
            .expect_err("malformed casting schedule must be rejected"),
            TimedOvnCastingAuthorizationErrorV1::InvalidPhaseSchedule
        );

        let missing_key_state =
            casting_state_at_height(state.clone(), lifecycle.clone(), None, None, 30);
        assert!(matches!(
            authorize_parliament_timed_ovn_casting_context_v1(
                &missing_key_state.query_view(),
                ballot_attempt_id,
            ),
            Err(TimedOvnCastingAuthorizationErrorV1::MissingKeySession)
        ));

        let mut tampered_registration = lifecycle.clone();
        tampered_registration.corrupt_first_registration_record_for_testing();
        let tampered_state = casting_state_at_height(
            state.clone(),
            tampered_registration,
            Some(&tle_key),
            Some(tle_key_session_id),
            30,
        );
        assert!(matches!(
            authorize_parliament_timed_ovn_casting_context_v1(
                &tampered_state.query_view(),
                ballot_attempt_id,
            ),
            Err(TimedOvnCastingAuthorizationErrorV1::TimedOvn(_))
        ));

        let wrong_key = casting_tle_key(network_binding, 0xB0);
        let mismatched_key_state = casting_state_at_height(
            state.clone(),
            lifecycle.clone(),
            Some(&wrong_key),
            Some(tle_key_session_id),
            30,
        );
        assert!(matches!(
            authorize_parliament_timed_ovn_casting_context_v1(
                &mismatched_key_state.query_view(),
                ballot_attempt_id,
            ),
            Err(TimedOvnCastingAuthorizationErrorV1::TimedOvn(_))
                | Err(TimedOvnCastingAuthorizationErrorV1::KeySession(_))
        ));

        let lifecycle = lifecycle
            .close_registration(&tle_key)
            .expect("close registration evidence");
        let TimedOvnLifecycleStateV1::RegistrationClosed(closed) = &lifecycle else {
            panic!("expected closed registration");
        };
        let (_, roster) = closed.validate(&tle_key).expect("replay closed roster");
        state
            .close_ballot_registration(
                governance_attempt_id,
                ballot_attempt_id,
                *roster.roster_root(),
                3,
                31,
            )
            .expect("advance reducer registration close");
        let closed_state = casting_state_at_height(
            state.clone(),
            lifecycle.clone(),
            Some(&tle_key),
            Some(tle_key_session_id),
            32,
        );
        let closed_context = authorize_parliament_timed_ovn_casting_context_v1(
            &closed_state.query_view(),
            ballot_attempt_id,
        )
        .expect("registration-closed casting context");
        assert_eq!(
            closed_context.phase(),
            ParliamentTimedOvnCastingPhaseV1::RegistrationClosed
        );
        assert!(closed_context.release_identity().is_none());
        let stale_closed_state = casting_state_at_height(
            state.clone(),
            lifecycle.clone(),
            Some(&tle_key),
            Some(tle_key_session_id),
            34,
        );
        assert_eq!(
            authorize_parliament_timed_ovn_casting_context_v1(
                &stale_closed_state.query_view(),
                ballot_attempt_id,
            )
            .expect_err("expired registration-closed context must be rejected"),
            TimedOvnCastingAuthorizationErrorV1::PhaseWindowInactive
        );

        let lifecycle = lifecycle
            .freeze_survivors(&tle_key)
            .expect("freeze survivor evidence");
        let TimedOvnLifecycleStateV1::SurvivorsFrozen(frozen) = &lifecycle else {
            panic!("expected survivor-frozen evidence");
        };
        state
            .freeze_ballot_survivors(
                governance_attempt_id,
                ballot_attempt_id,
                *frozen.dropout_root(),
                frozen.release_identity().survivor_corpus_root,
                u32::try_from(frozen.survivor_participant_hashes().len()).expect("survivor count"),
                frozen.release_identity().no_recovery_root,
                34,
            )
            .expect("advance reducer survivor freeze");
        let frozen_state = casting_state_at_height(
            state.clone(),
            lifecycle.clone(),
            Some(&tle_key),
            Some(tle_key_session_id),
            34,
        );
        let frozen_context = authorize_parliament_timed_ovn_casting_context_v1(
            &frozen_state.query_view(),
            ballot_attempt_id,
        )
        .expect("survivor-frozen casting context");
        assert_eq!(
            frozen_context.phase(),
            ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen
        );
        assert_eq!(
            frozen_context
                .survivor_participant_hashes()
                .expect("frozen survivors")
                .len(),
            3
        );
        assert!(frozen_context.release_identity().is_some());
        assert!(
            frozen_context
                .archive_v1()
                .validate_v1()
                .expect("frozen archive replays")
                .prepared_attempt()
                .is_some()
        );
        let stale_frozen_state = casting_state_at_height(
            state,
            lifecycle,
            Some(&tle_key),
            Some(tle_key_session_id),
            36,
        );
        assert_eq!(
            authorize_parliament_timed_ovn_casting_context_v1(
                &stale_frozen_state.query_view(),
                ballot_attempt_id,
            )
            .expect_err("expired survivor-frozen context must be rejected"),
            TimedOvnCastingAuthorizationErrorV1::PhaseWindowInactive
        );

        assert_eq!(
            ParliamentTimedOvnCastingPhaseV1::try_from(TimedOvnLifecyclePhaseV1::Sealed),
            Err(TimedOvnCastingAuthorizationErrorV1::PhaseNotCastable)
        );
        assert_eq!(
            ParliamentTimedOvnCastingPhaseV1::try_from(TimedOvnLifecyclePhaseV1::Released),
            Err(TimedOvnCastingAuthorizationErrorV1::PhaseNotCastable)
        );
    }

    #[test]
    fn timed_ovn_checkpoint_prechecks_reject_phase_and_height_before_replay() {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        advance_to_vote(&mut state, body_id);
        let attempt_id = state.attempt.id;
        let ballot_id = BallotAttemptId::derive_v1(body_id, 0);
        let release_beacon_session_id = beacon_session(24);
        let tle_key_session_id = tle_key_session(23);
        let release_height = 40;
        state
            .register_ballot_attempt(
                attempt_id,
                body_id,
                ballot_id,
                0,
                TleSessionId::derive_v1(
                    ballot_id,
                    tle_key_session_id,
                    release_beacon_session_id,
                    release_height,
                ),
                tle_key_session_id,
                release_beacon_session_id,
                27,
                timed_ovn_policy(),
                release_height,
            )
            .expect("register timed ballot");

        assert_eq!(
            state.precheck_close_ballot_registration(attempt_id, ballot_id, 30),
            Err(ParliamentReducerErrorV1::WrongBallotPhaseHeight)
        );
        state
            .precheck_close_ballot_registration(attempt_id, ballot_id, 31)
            .expect("exact registration deadline passes the cheap guard");
        state
            .close_ballot_registration(attempt_id, ballot_id, root(19), 3, 31)
            .expect("close registration");
        assert_eq!(
            state.precheck_close_ballot_registration(attempt_id, ballot_id, 31),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
        );

        assert_eq!(
            state.precheck_freeze_ballot_survivors(attempt_id, ballot_id, 33),
            Err(ParliamentReducerErrorV1::WrongBallotPhaseHeight)
        );
        state
            .precheck_freeze_ballot_survivors(attempt_id, ballot_id, 34)
            .expect("exact survivor deadline passes the cheap guard");
        state
            .freeze_ballot_survivors(attempt_id, ballot_id, root(21), root(29), 3, root(22), 34)
            .expect("freeze survivors");
        assert_eq!(
            state.precheck_freeze_ballot_survivors(attempt_id, ballot_id, 34),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
        );

        assert_eq!(
            state.precheck_freeze_timed_ovn_corpus(attempt_id, ballot_id, 34),
            Err(ParliamentReducerErrorV1::WrongBallotPhaseHeight)
        );
        state
            .precheck_freeze_timed_ovn_corpus(attempt_id, ballot_id, 35)
            .expect("first commitment-window height passes the cheap guard");
        state
            .precheck_freeze_timed_ovn_corpus(attempt_id, ballot_id, 36)
            .expect("last commitment-window height passes the cheap guard");
        assert_eq!(
            state.precheck_freeze_timed_ovn_corpus(attempt_id, ballot_id, 37),
            Err(ParliamentReducerErrorV1::WrongBallotPhaseHeight)
        );

        let mut early_completion = state.clone();
        early_completion
            .freeze_timed_ovn_corpus(attempt_id, ballot_id, root(20), root(29), 3, root(25), 35)
            .expect("a complete corpus may seal at the first window height");
        assert_eq!(
            early_completion
                .ballot(&ballot_id)
                .expect("early-completed ballot")
                .commitment_closed_at_height,
            Some(35)
        );

        let mut incomplete_at_close = state.clone();
        incomplete_at_close
            .fail_ballot_no_result(attempt_id, ballot_id, false, 37)
            .expect("an incomplete corpus prefix becomes objectively fail-able after close");
        assert_eq!(
            incomplete_at_close
                .ballot(&ballot_id)
                .expect("failed incomplete ballot")
                .failure_kind,
            Some(ParliamentBallotFailureKindV1::CommitmentDeadlineExpired)
        );

        state
            .freeze_timed_ovn_corpus(attempt_id, ballot_id, root(20), root(29), 3, root(25), 36)
            .expect("freeze ballot corpus");
        assert_eq!(
            state.precheck_freeze_timed_ovn_corpus(attempt_id, ballot_id, 36),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
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
                27,
                timed_ovn_policy(),
                first_release_height,
            )
            .expect("registration");
        assert_eq!(
            state.freeze_ballot_survivors(id, ballot, root(21), root(29), 3, root(22), 34),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
        );
        state
            .close_ballot_registration(id, ballot, root(19), 3, 31)
            .expect("commitment");
        assert_eq!(
            state.close_ballot_registration(id, ballot, root(19), 3, 31),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
        );
        state
            .freeze_ballot_survivors(id, ballot, root(21), root(29), 2, root(22), 34)
            .expect("freeze nonempty survivor roster");
        assert_eq!(
            state.freeze_timed_ovn_corpus(id, ballot, root(20), root(28), 2, root(25), 36),
            Err(ParliamentReducerErrorV1::AcceptedCorpusMutation)
        );
        state
            .freeze_timed_ovn_corpus(id, ballot, root(20), root(29), 2, root(25), 36)
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
                41,
            ),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BallotAttempt
            ))
        );
        state
            .fail_ballot_no_result(id, ballot, false, 41)
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
                timed_ovn_policy(),
                54,
            ),
            Err(ParliamentReducerErrorV1::TleSessionAlreadyConsumed)
        );
        let retry_release_beacon_session_id = beacon_session(33);
        let retry_tle_key_session_id = tle_key_session(32);
        let retry_release_height = 54;
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
                timed_ovn_policy(),
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

    /// Build a validated attempt retaining `key_session_id` through two ballot deadlines.
    pub(crate) fn tle_key_session_retention_attempt_fixture_v1(
        key_session_id: TleKeySessionId,
    ) -> ParliamentAttemptStateV1 {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        advance_to_vote(&mut state, body_id);
        let governance_attempt_id = state.attempt.id;
        let policy = timed_ovn_policy();

        let first_ballot = BallotAttemptId::derive_v1(body_id, 0);
        let first_beacon = beacon_session(87);
        let first_release_height = 40;
        let first_tle_session = TleSessionId::derive_v1(
            first_ballot,
            key_session_id,
            first_beacon,
            first_release_height,
        );
        state
            .register_ballot_attempt(
                governance_attempt_id,
                body_id,
                first_ballot,
                0,
                first_tle_session,
                key_session_id,
                first_beacon,
                27,
                policy,
                first_release_height,
            )
            .expect("register first ballot");
        assert_eq!(
            state.tle_key_session_retention_deadline(key_session_id),
            Some(42)
        );

        state
            .fail_ballot_no_result(governance_attempt_id, first_ballot, false, 41)
            .expect("objectively fail first ballot");
        let retry_ballot = BallotAttemptId::derive_v1(body_id, 1);
        let retry_beacon = beacon_session(88);
        let retry_release_height = 60;
        let retry_tle_session = TleSessionId::derive_v1(
            retry_ballot,
            key_session_id,
            retry_beacon,
            retry_release_height,
        );
        state
            .register_ballot_attempt(
                governance_attempt_id,
                body_id,
                retry_ballot,
                1,
                retry_tle_session,
                key_session_id,
                retry_beacon,
                47,
                policy,
                retry_release_height,
            )
            .expect("register retry with rotating key still retained");

        state
    }

    #[test]
    fn tle_custody_retention_uses_maximum_deadline_across_ballot_retries() {
        let key_session_id = tle_key_session(86);
        let state = tle_key_session_retention_attempt_fixture_v1(key_session_id);
        assert_eq!(
            state.tle_key_session_retention_deadline(key_session_id),
            Some(62)
        );
        assert_eq!(
            state.tle_key_session_retention_deadline(tle_key_session(89)),
            None
        );
    }

    #[test]
    fn final_private_ballot_retry_failure_rejects_the_governance_attempt() {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        advance_to_vote(&mut state, body_id);
        let attempt_id = state.attempt.id;
        let policy = timed_ovn_policy();
        let mut registered_at_height = 30_u64;
        let mut final_ballot = None;
        let mut final_failure_height = None;

        for sequence in 0..=policy.max_ballot_retries {
            let ballot_id = BallotAttemptId::derive_v1(body_id, sequence);
            let tle_key_session_id =
                tle_key_session(u8::try_from(110 + sequence).expect("test sequence fits in u8"));
            let release_beacon_session_id =
                beacon_session(u8::try_from(120 + sequence).expect("test sequence fits in u8"));
            let release_height = registered_at_height + 13;
            let tle_session_id = TleSessionId::derive_v1(
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
                    sequence,
                    tle_session_id,
                    tle_key_session_id,
                    release_beacon_session_id,
                    registered_at_height,
                    policy,
                    release_height,
                )
                .expect("register the exact next private ballot attempt");
            let failure_height = registered_at_height + 5;
            state
                .fail_ballot_no_result(attempt_id, ballot_id, false, failure_height)
                .expect("registration timeout is objectively derived");
            final_ballot = Some(ballot_id);
            final_failure_height = Some(failure_height);
            if sequence < policy.max_ballot_retries {
                assert_eq!(state.attempt.status, GovernanceAttemptStatusV1::Active);
            }
            registered_at_height = failure_height;
        }

        assert_eq!(state.attempt.status, GovernanceAttemptStatusV1::Rejected);
        assert_eq!(
            state.body(&body_id).expect("policy body").instance.status,
            BodyInstanceStatusV1::NoResult
        );
        let active_ballot = state
            .active_ballot_for_body(&body_id)
            .expect("the final failed ballot remains the active body transcript");
        assert_eq!(
            active_ballot.attempt().id,
            final_ballot.expect("final ballot id")
        );
        assert_eq!(
            active_ballot.failure_kind(),
            Some(ParliamentBallotFailureKindV1::RegistrationDeadlineExpired)
        );
        assert_eq!(active_ballot.failure_height(), final_failure_height);
        state
            .validate()
            .expect("exhausted private-ballot retry rejection persists canonically");
    }

    #[test]
    fn ballot_failure_reason_is_derived_from_the_frozen_phase() {
        let BodyFixture {
            mut state, body_id, ..
        } = sealed_policy_body(3);
        advance_to_vote(&mut state, body_id);
        let attempt_id = state.attempt.id;
        let ballot_id = BallotAttemptId::derive_v1(body_id, 0);
        let release_beacon_session_id = beacon_session(73);
        let tle_key_session_id = tle_key_session(72);
        let release_height = 40;
        let tle_session_id = TleSessionId::derive_v1(
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
                tle_session_id,
                tle_key_session_id,
                release_beacon_session_id,
                27,
                timed_ovn_policy(),
                release_height,
            )
            .expect("register private ballot");

        assert_eq!(
            state.fail_ballot_no_result(attempt_id, ballot_id, false, 31),
            Err(ParliamentReducerErrorV1::BallotFailureKindMismatch)
        );
        let mut registration_expired = state.clone();
        registration_expired
            .fail_ballot_no_result(attempt_id, ballot_id, false, 32)
            .expect("registration expiry is derived after its boundary");
        assert_eq!(
            registration_expired
                .ballot(&ballot_id)
                .expect("failed ballot")
                .failure_kind,
            Some(ParliamentBallotFailureKindV1::RegistrationDeadlineExpired)
        );
        let expected_failure_root = parliament_ballot_failure_root_v1(
            attempt_id,
            ballot_id,
            ParliamentBallotFailureKindV1::RegistrationDeadlineExpired,
            32,
        );
        assert_eq!(
            registration_expired
                .ballot(&ballot_id)
                .expect("failed ballot")
                .failure_root,
            Some(expected_failure_root)
        );
        registration_expired
            .validate()
            .expect("derived registration failure persists canonically");
        registration_expired
            .ballots
            .get_mut(&ballot_id)
            .expect("failed ballot")
            .failure_root = Some(root(70));
        assert_eq!(
            registration_expired.validate(),
            Err(ParliamentReducerErrorV1::BallotFailureKindMismatch)
        );

        state
            .close_ballot_registration(attempt_id, ballot_id, root(71), 3, 31)
            .expect("freeze registration");
        let mut survivor_expired = state.clone();
        survivor_expired
            .fail_ballot_no_result(attempt_id, ballot_id, false, 35)
            .expect("survivor expiry is derived after its boundary");
        assert_eq!(
            survivor_expired
                .ballot(&ballot_id)
                .expect("failed ballot")
                .failure_kind,
            Some(ParliamentBallotFailureKindV1::SurvivorDeadlineExpired)
        );

        state
            .freeze_ballot_survivors(attempt_id, ballot_id, root(74), root(75), 3, root(76), 34)
            .expect("freeze survivors");
        let mut commitment_expired = state.clone();
        commitment_expired
            .fail_ballot_no_result(attempt_id, ballot_id, false, 37)
            .expect("commitment expiry is derived after its boundary");
        assert_eq!(
            commitment_expired
                .ballot(&ballot_id)
                .expect("failed ballot")
                .failure_kind,
            Some(ParliamentBallotFailureKindV1::CommitmentDeadlineExpired)
        );

        state
            .freeze_timed_ovn_corpus(attempt_id, ballot_id, root(77), root(75), 3, root(78), 36)
            .expect("freeze timed corpus");
        let mut release_expired = state.clone();
        release_expired
            .fail_ballot_no_result(attempt_id, ballot_id, false, 41)
            .expect("release expiry is derived after its boundary");
        assert_eq!(
            release_expired
                .ballot(&ballot_id)
                .expect("failed ballot")
                .failure_kind,
            Some(ParliamentBallotFailureKindV1::ReleasePulseUnavailable)
        );

        let mut finalized_pulse_before_deadline = state.clone();
        assert_eq!(
            finalized_pulse_before_deadline.fail_ballot_no_result(
                attempt_id,
                ballot_id,
                true,
                release_height + 1,
            ),
            Err(ParliamentReducerErrorV1::BallotFailureKindMismatch)
        );
        finalized_pulse_before_deadline
            .fail_ballot_no_result(attempt_id, ballot_id, true, release_height + 3)
            .expect("an unconsumed finalized pulse cannot strand a ballot past opening deadline");
        assert_eq!(
            finalized_pulse_before_deadline
                .ballot(&ballot_id)
                .expect("failed ballot")
                .failure_kind,
            Some(ParliamentBallotFailureKindV1::OpeningDeadlineExpired)
        );
        finalized_pulse_before_deadline
            .validate()
            .expect("objective opening-deadline failure persists canonically");

        let mut late_opening = state.clone();
        assert_eq!(
            late_opening.begin_ballot_opening_batch(
                attempt_id,
                vec![ballot_id],
                release_beacon_session_id,
                release_height,
                release_height + 3,
                pulse_id(80),
            ),
            Err(ParliamentReducerErrorV1::PulseBindingMismatch)
        );

        state
            .begin_ballot_opening_batch(
                attempt_id,
                vec![ballot_id],
                release_beacon_session_id,
                release_height,
                release_height,
                pulse_id(79),
            )
            .expect("consume exact release pulse");
        assert_eq!(
            state.fail_ballot_no_result(attempt_id, ballot_id, true, release_height),
            Err(ParliamentReducerErrorV1::BallotFailureKindMismatch)
        );
        let mut opening_expired = state.clone();
        opening_expired
            .fail_ballot_no_result(attempt_id, ballot_id, true, release_height + 3)
            .expect("an incomplete aggregate opening expires objectively");
        assert_eq!(
            opening_expired
                .ballot(&ballot_id)
                .expect("failed opening")
                .failure_kind,
            Some(ParliamentBallotFailureKindV1::OpeningDeadlineExpired)
        );
        opening_expired
            .validate()
            .expect("expired opening transcript remains canonical");
        assert_eq!(
            state
                .ballot(&ballot_id)
                .expect("opening ballot")
                .attempt
                .status,
            BallotAttemptStatusV1::Opening
        );
        state
            .validate()
            .expect("a rejected caller-selected opening failure leaves canonical state");
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
            250,
            260,
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
            150,
            3,
            3,
            250,
            260,
            beacon_session(104),
            None,
        );
        let confirmation_request_id = request.id;
        let confirmation_election_id = request.body_election_attempt_id;
        let all_policy_members = narrow
            .state
            .sealed_body_for_role(ParliamentBody::PolicyJury)
            .expect("completed policy body")
            .assignments
            .iter()
            .map(|assignment| assignment.member.clone())
            .collect::<BTreeSet<_>>();
        assert!(
            candidate_snapshot
                .iter()
                .all(|candidate| !all_policy_members.contains(candidate)),
            "fresh confirmation fixture candidates must exclude every Policy Jury member"
        );
        narrow
            .state
            .register_sortition_request(id, 0, request, candidate_snapshot)
            .expect("register fresh confirmation draw");
        consume_sortition(
            &mut narrow.state,
            id,
            vec![confirmation_request_id],
            beacon_session(104),
            260,
            pulse_id(105),
        )
        .expect("consume confirmation pulse");
        narrow
            .state
            .begin_invitation_acceptance(id, confirmation_election_id, 260, 1)
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
                .record_invitation_response(id, confirmation_election_id, &member, true, 260)
                .expect("accept confirmation invitation");
        }
        narrow
            .state
            .seal_body_roster(id, confirmation_election_id, 261)
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
        fixture
            .state
            .validate()
            .expect("the reducer's pre-certificate state is internally consistent");
        assert_eq!(
            fixture.state.validate_restored_height_v1(41),
            Err(ParliamentReducerErrorV1::IncompleteCertificate),
            "the atomic pre-certificate transient must never survive restart"
        );
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
            fixture.state.validate_restored_height_v1(49),
            Err(ParliamentReducerErrorV1::InvalidCertificateHeight)
        );
        fixture
            .state
            .validate_restored_height_v1(59)
            .expect("a certified effect remains future before its due height");
        assert_eq!(
            fixture.state.validate_restored_height_v1(60),
            Err(ParliamentReducerErrorV1::WrongEnactmentHeight)
        );
        assert_eq!(
            fixture.state.mark_enacted(id, 59),
            Err(ParliamentReducerErrorV1::WrongEnactmentHeight)
        );

        let mut late = fixture.state.clone();
        assert_eq!(
            late.mark_enacted(id, 61),
            Err(ParliamentReducerErrorV1::WrongEnactmentHeight)
        );

        let mut enacted = fixture.state.clone();
        enacted.mark_enacted(id, 60).expect("enact due certificate");
        assert_eq!(
            enacted.validate_restored_height_v1(59),
            Err(ParliamentReducerErrorV1::InvalidCertificateHeight)
        );
        enacted
            .validate_restored_height_v1(60)
            .expect("terminal outcome is committed at the restored boundary");
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
            failed.mark_execution_failed(id, 59),
            Err(ParliamentReducerErrorV1::WrongEnactmentHeight)
        );
        let expected_failure_root = parliament_execution_failure_root_v1(&certificate, 60);
        assert_eq!(
            failed
                .mark_execution_failed(id, 60)
                .expect("exact due certificate records execution failure"),
            expected_failure_root
        );
        assert_eq!(
            failed.attempt.status,
            GovernanceAttemptStatusV1::ExecutionFailed
        );
        assert_eq!(failed.terminal_height(), Some(60));
        assert_eq!(failed.execution_failure_root(), Some(expected_failure_root));
        failed
            .validate()
            .expect("execution-failed terminal state validates");
        let encoded_failure = norito::to_bytes(&failed).expect("encode execution failure state");
        let decoded_failure =
            norito::decode_from_bytes::<ParliamentAttemptStateV1>(&encoded_failure)
                .expect("decode execution failure state");
        assert_eq!(decoded_failure, failed);
        decoded_failure
            .validate()
            .expect("decoded execution failure state validates");
        assert!(matches!(
            failed.mark_execution_failed(id, 60),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(_))
        ));

        let mut corrupted_failure = failed;
        let mut corrupted_root = expected_failure_root;
        corrupted_root[0] ^= 1;
        corrupted_failure.execution_failure_root = Some(corrupted_root);
        assert_eq!(
            corrupted_failure.validate(),
            Err(ParliamentReducerErrorV1::CertificateBindingMismatch)
        );
    }

    #[test]
    fn persistence_rejects_future_actions_and_body_stage_skips() {
        let mut fixture = opened_policy_ballot(3, 3);
        fixture
            .state
            .validate()
            .expect("opened ballot fixture validates structurally");
        for restored_height in [9, 19, 20, 29, 31, 33, 35, 39] {
            assert_eq!(
                fixture.state.validate_restored_height_v1(restored_height),
                Err(ParliamentReducerErrorV1::FuturePersistedHeight),
                "realized lifecycle state must not come from after restored height {restored_height}"
            );
        }
        fixture
            .state
            .validate_restored_height_v1(40)
            .expect("every realized opening fixture height is committed by height 40");

        assert_eq!(
            finalize_policy(&mut fixture, 2, 1, 0),
            ParliamentAggregateOutcomeV1::Approved
        );
        assert_eq!(
            fixture.state.validate_restored_height_v1(39),
            Err(ParliamentReducerErrorV1::FuturePersistedHeight),
            "the body result was not realized until height 40"
        );
        assert_eq!(
            fixture.state.validate_restored_height_v1(40),
            Err(ParliamentReducerErrorV1::IncompleteCertificate),
            "Certification is an in-transaction transient until Core constructs the certificate"
        );

        let mut skipped = fixture.state.clone();
        skipped.attempt.stage = GovernanceStageV1::PolicyJury;
        assert_eq!(
            skipped.validate(),
            Err(ParliamentReducerErrorV1::IncompleteCertificate),
            "a current-body stage cannot retain that body's completed binding"
        );
        let mut missing = fixture.state;
        missing.body_bindings.clear();
        assert_eq!(
            missing.validate(),
            Err(ParliamentReducerErrorV1::IncompleteCertificate),
            "Certification requires the exact completed required-body prefix"
        );
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
            norito::json::from_json(&json).unwrap_or_else(|error| {
                let bytes = json.as_bytes();
                let start = 12_200.min(bytes.len());
                let end = 12_340.min(bytes.len());
                panic!(
                    "decode reducer state from Norito JSON: {error}; nearby bytes: {}",
                    String::from_utf8_lossy(&bytes[start..end]).escape_debug()
                )
            });
        json_decoded
            .validate()
            .expect("JSON-decoded state validates");
        assert_eq!(json_decoded, decoded);
    }
}
