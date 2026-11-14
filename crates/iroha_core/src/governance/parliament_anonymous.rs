//! Reducer of one attempt of the anonymous Parliament definition family
//! (`specs/parliament_private_ballot_design.md` §12.3).
//!
//! An anonymous attempt is created through its own interface and keeps this State from
//! creation. It shares no State with the frozen baseline reducer ([`super::parliament`]):
//! its identifiers come from the family's own derivations, every body is sealed under the
//! 64-byte roster root, a binding jury contributes the closure record of its one ballot,
//! and there is no ballot retry, timed-OVN phase or TLE session.
//!
//! The attempt runs on the governance clock of design §8.1: transactions record facts and
//! intents, and the positioned block-start pass applies every transition at its anchor.
//!
//! - **Purpose.** The attempt records why it may exist, from creation, and the purpose
//!   travels into its certificate. A candidate-testing attempt ends at its certificate:
//!   every execution outcome requires production authority, so a candidate certificate is
//!   never enacted, superseded or failed as a production effect.
//! - **Policy.** The policy snapshot frozen at admission is the only policy the attempt
//!   reads. The initial sortition registration locks the risk tier, and both ballot
//!   policies derive from the snapshot and the locked tier.
//! - **Elections** ([`election`]). Sortition generations are registered by transactions,
//!   drawn by the pass from the exact-slot pulse, answered by the invited accounts and
//!   closed by the pass into a sealed body or `NoRoster`; retries and the proposal-wide
//!   redraw budget bound them.
//! - **Deliberation** ([`deliberation`]). Qualification and every phase have one
//!   millisecond deadline each; authorized intents only shorten them. A public body is
//!   decided by its endorsements and absences and concluded by the pass.
//! - **Results.** Bodies are sealed in any order and produce results in pipeline order. A
//!   public body's quorum-endorsed finding and a binding jury's approved closure advance
//!   the attempt; any other closure outcome rejects it. A narrow Policy Jury approval
//!   registers the Confirmation Jury's generation atomically, or ends the attempt when no
//!   fresh capacity or redraw unit remains.
//! - **Certificate and execution.** The certificate is built from the recorded results
//!   alone and checked as a whole. Enactment is due on the governance clock once it
//!   reaches the certifying closure's time plus the frozen enactment delay.
//! - **Schedule** ([`schedule`]). The attempt reports its live occurrences with their due
//!   times, keys and costs.
//!
//! - **Credentials** ([`credentials`]). A binding seat is accepted only with its verified
//!   credential, and a binding jury seals exactly its seated credentials, built before the
//!   close changes anything; the planner trial-applies due items with the same seal step
//!   the pass applies, and the restore check binds the stored credentials and boards to
//!   the State.
//!
//! - **Ballots** ([`ballot_input`]). A signerless anonymous-ballot input is decided against
//!   the board World keeps for its scope: an invalid one makes its block invalid, a valid one
//!   outside its window or under a taken nullifier is recorded as rejected, and an accepted
//!   one joins the board's corpus, which the closure counts.
//!
//! World keeps the attempts, the credentials, the boards and the due index in the tables of
//! the `iroha.governance.parliament.anonymous.v1` release addition
//! (`crate::state::parliament_anonymous_tables`). `CreateAnonymousParliamentAttemptV1`
//! creates an attempt, `SubmitAnonymousParliamentTransitionV1` records its facts and intents,
//! `RegisterParliamentCredentialV1` drives
//! [`AnonymousParliamentAttemptStateV1::register_credential`], and the positioned block-start
//! pass ([`world_pass`]) applies the scheduled items, the ballot openings and the closures
//! ([`AnonymousParliamentAttemptStateV1::close_ballot`]). No selection activates the addition
//! on a live network: binding production use stays disabled until the owner records the
//! proof profile's qualification.
//!
//! TODO(design §12.3, slice 5): production enactment (G3) of a certified production attempt.
//! This release certifies no production attempt.

use std::collections::BTreeMap;

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            AnonymousBallotBindingErrorV1, AnonymousBallotCertificateBindingV1,
            AnonymousBodyResultV1, AnonymousGovernanceAttemptV1,
            AnonymousGovernanceCertificateErrorV1, AnonymousGovernanceCertificateV1,
            AnonymousParliamentBodyBindingV1, AnonymousPublicFindingResultV1,
            AnonymousSealedBodyV1, BallotClosureRecordV1, BallotOutcomeV1, BodyDecisionModeV1,
            Digest512, FrozenBallotPolicyErrorV1, FrozenParliamentAttemptPolicyV1,
            ParliamentBallotPurposeErrorV1, ParliamentBallotPurposeV1,
            anonymous_execution_failure_root_v1,
        },
        types::{
            DeliberationPhaseV1, GovernanceAttemptId, GovernanceAttemptStatusV1,
            GovernanceExpectedHeadV1, GovernanceStageV1, ParliamentBody, RiskTierV1,
        },
    },
};
use iroha_parliament_ballot::board::{BoardErrorV1, RegistrationLedgerErrorV1, SealErrorV1};
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

pub(crate) mod ballot_input;
pub mod ballot_lifecycle;
pub mod credentials;
pub mod deliberation;
pub mod election;
pub(crate) mod pass;
mod restore;
pub mod schedule;
pub(crate) mod world_pass;

pub use self::{
    credentials::{
        AnonymousElectionCredentialsV1, AnonymousItemEffectV1, AnonymousItemOutcomeV1,
        AnonymousPlannedItemV1, AnonymousSealedBoardV1,
    },
    deliberation::{
        AnonymousBodyStateV1, AnonymousFindingDecisionV1, AnonymousFindingV1,
        AnonymousFindingVerdictV1, AnonymousPhaseV1, AnonymousQualificationV1,
    },
    election::{
        AnonymousDrawOutcomeV1, AnonymousDrawV1, AnonymousElectionClosedV1, AnonymousElectionV1,
        AnonymousGenerationStatusV1, AnonymousGenerationV1, AnonymousInvitationCloseV1,
        AnonymousNoRosterKindV1, AnonymousRegistrationV1, AnonymousSeatingV1,
        AnonymousSortitionPulseV1,
    },
    schedule::{AnonymousDueItemV1, AnonymousLifecycleItemV1},
};

/// Why an anonymous attempt refuses a transition, or a restored State is not one this
/// reducer could have produced.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnonymousParliamentErrorV1 {
    /// The purpose is not one this release recognizes, or carries no production authority
    /// where execution requires it.
    #[error(transparent)]
    Purpose(#[from] ParliamentBallotPurposeErrorV1),
    /// The frozen policy is not a valid V1 snapshot, or yields no ballot policy.
    #[error(transparent)]
    Policy(#[from] FrozenBallotPolicyErrorV1),
    /// A sealed body, a result or the certificate fails its own rules.
    #[error(transparent)]
    Certificate(#[from] AnonymousGovernanceCertificateErrorV1),
    /// A closure record fails its policy.
    #[error(transparent)]
    Ballot(#[from] AnonymousBallotBindingErrorV1),
    /// The attempt identifier is not the family's derivation, or a binding is inert.
    #[error("the attempt's immutable bindings are not canonical")]
    ImmutableBinding,
    /// The retry sequence exceeds the frozen limit.
    #[error("the attempt's retry sequence exceeds the frozen limit")]
    RetryLimitExceeded,
    /// A new attempt is not active at qualification.
    #[error("a new attempt starts active at qualification")]
    NotInitial,
    /// The transition names another attempt.
    #[error("the transition names another attempt")]
    WrongAttempt,
    /// The attempt is no longer active.
    #[error("the attempt is {status:?}, not active")]
    NotActive {
        /// Its status.
        status: GovernanceAttemptStatusV1,
    },
    /// The transition belongs to another stage.
    #[error("the attempt is at {actual:?}, not {expected:?}")]
    Stage {
        /// Stage the transition belongs to.
        expected: GovernanceStageV1,
        /// Stage the attempt is at.
        actual: GovernanceStageV1,
    },
    /// The risk tier is already locked.
    #[error("the risk tier is locked")]
    RiskTierLocked,
    /// The risk tier is not locked yet.
    #[error("the risk tier is not locked yet")]
    RiskTierNotLocked,
    /// The escalation names the current tier.
    #[error("the attempt is already at this risk tier")]
    RiskEscalationReplay,
    /// The escalation lowers the tier.
    #[error("the risk tier only rises")]
    RiskDowngrade,
    /// The body already has a sealed instance.
    #[error("{body:?} is already sealed")]
    BodySealed {
        /// The body.
        body: ParliamentBody,
    },
    /// The body has no sealed instance.
    #[error("{body:?} is not sealed")]
    BodyNotSealed {
        /// The body.
        body: ParliamentBody,
    },
    /// A Confirmation Jury is sealed without a narrow Policy Jury approval.
    #[error("no narrow Policy Jury approval requires a Confirmation Jury")]
    ConfirmationNotRequired,
    /// A binding jury is sealed below its anonymity floor.
    #[error("a binding jury seals at least {minimum} seats, not {seats}")]
    BelowSealFloor {
        /// Sealed seats.
        seats: u16,
        /// The frozen floor.
        minimum: u16,
    },
    /// The result is of the other decision mode.
    #[error("{body:?} does not decide in this mode")]
    DecisionMode {
        /// The body.
        body: ParliamentBody,
    },
    /// There is no certificate to execute.
    #[error("the attempt has no certificate due for execution")]
    NotCertified,
    /// The governance clock has not reached the enactment time.
    #[error("enactment is due at {due_ms} ms, not at {now_ms} ms")]
    EnactmentNotDue {
        /// Time from which enactment is due.
        due_ms: u64,
        /// Time of the executing block.
        now_ms: u64,
    },
    /// The executing item does not follow the certifying closure: in the certifying block
    /// it runs later in the same pass, at the same block time.
    #[error("the executing item does not follow the certifying closure")]
    ExecutionAnchor,
    /// A public finding fails before any pulse could have drawn its body.
    #[error("a public finding fails after its body's sortition pulse")]
    FailureBeforeSortition,
    /// The observed head is the certificate's own.
    #[error("the observed head is the certificate's expected head")]
    ExpectedHeadUnchanged,
    /// The observed head is inert or of another subject.
    #[error("the superseding head is invalid")]
    InvalidSupersedingHead,
    /// The redraw prefix inherited from the predecessor attempt is outside the lineage.
    #[error("the inherited redraw count is outside the proposal's lineage")]
    RedrawLineage,
    /// The candidate snapshot is unordered, over its bounds, or empty where it must not be.
    #[error("the candidate snapshot is not canonical or exceeds its bounds")]
    CandidateSnapshot,
    /// A height, slot or deadline overflows or is inert.
    #[error("the lifecycle schedule cannot be expressed")]
    Schedule,
    /// The proposal-wide redraw budget is spent.
    #[error("the proposal-wide redraw budget is spent")]
    RedrawBudgetExhausted,
    /// The body's sortition retries are spent.
    #[error("the body's sortition retries are spent")]
    SortitionRetryLimitExceeded,
    /// The initial generation is already registered.
    #[error("the initial sortition generation is already registered")]
    SortitionRegistered,
    /// Capacity evidence left no retry; `RejectExhausted` awaits its pass.
    #[error("the attempt awaits its exhaustion")]
    ExhaustionPending,
    /// No body awaits a retry.
    #[error("no body's election ended without a roster")]
    NoRetryDue,
    /// A retry is registered no later than a generation it retries.
    #[error("a retry must follow the registration of the generation it retries")]
    RetryTooEarly,
    /// The body is not in the frozen pipeline.
    #[error("{body:?} is not a body of the frozen pipeline")]
    UnknownBody {
        /// The body.
        body: ParliamentBody,
    },
    /// No sealed body has the instance.
    #[error("no sealed body has this instance")]
    UnknownBodyInstance,
    /// No generation awaits the slot.
    #[error("no generation awaits this slot")]
    UnknownGeneration,
    /// The election is not the body's latest one.
    #[error("the election is not the body's latest")]
    UnknownElection,
    /// The pulse is of another session, inert, or not yet at its slot.
    #[error("the pulse does not bind the generation's slot")]
    Pulse,
    /// The allocation does not fit the generation's requests.
    #[error("the allocation does not fit its requests")]
    Allocation,
    /// The item is not due at this anchor.
    #[error("the item is not due")]
    NotDue,
    /// The item names an occurrence that has passed.
    #[error("the item's occurrence has passed")]
    StaleItem,
    /// The election has no open invitations.
    #[error("the election's invitations are not open")]
    InvitationsNotOpen,
    /// The invitation window closed.
    #[error("the invitation window closed")]
    InvitationWindowClosed,
    /// The authority holds no invitation in the election.
    #[error("the authority holds no invitation")]
    UnknownInvitation,
    /// The assignment already responded.
    #[error("the assignment already responded")]
    ResponseReplay,
    /// A binding seat is accepted only with its credential.
    #[error("a binding seat is accepted only with its credential")]
    CredentialRequired,
    /// A roster binding is missing, unexpected, or not of the seated roster.
    #[error("the roster binding does not fit the seated roster")]
    RosterBinding,
    /// Qualification already started.
    #[error("qualification already started")]
    QualificationStarted,
    /// The intent names another phase, one that never closes early, or a body without one.
    #[error("the phase does not match the body's current phase")]
    PhaseMismatch,
    /// An early-close intent was already recorded for this wait.
    #[error("an early-close intent is already recorded")]
    EarlyCloseReplay,
    /// The public body is already decided.
    #[error("the public body is already decided")]
    FindingDecided,
    /// An absence follows an endorsement of the body.
    #[error("an absence cannot follow an endorsement")]
    AbsenceAfterEndorsement,
    /// The finding window closed.
    #[error("the finding window closed")]
    FindingWindowClosed,
    /// The authority holds no seat of the body, or not this one.
    #[error("the authority holds no such seat")]
    UnauthorizedMember,
    /// The seat's absence is already recorded.
    #[error("the seat's absence is already recorded")]
    AbsenceReplay,
    /// The seat is excluded by its absence.
    #[error("the seat is excluded")]
    SeatExcluded,
    /// The seat already endorsed.
    #[error("the seat already endorsed")]
    EndorsementReplay,
    /// The endorsed root is inert.
    #[error("the endorsed root is inert")]
    InertFindingRoot,
    /// The ballot's chronology does not follow the jury's lifecycle.
    #[error("the ballot opened before its jury's Reflection and notice")]
    BallotChronology,
    /// A credential registration is refused: a seat or commitment already registered, or a
    /// registration that does not verify or that the node could not verify.
    #[error(transparent)]
    Credential(Box<RegistrationLedgerErrorV1>),
    /// The credentials cannot be sealed.
    #[error(transparent)]
    Seal(Box<SealErrorV1>),
    /// The board of a sealed jury cannot be built.
    #[error(transparent)]
    Board(Box<BoardErrorV1>),
    /// A credential's source is not a block of finalized history.
    #[error("a credential source must name a nonzero block height")]
    CredentialSource,
    /// The credentials World keeps for an election are not exactly one per accepted
    /// assignment, or repeat a seat or commitment.
    #[error("the stored credentials do not match their election's accepted assignments")]
    CredentialStore,
    /// A restored State is inconsistent.
    #[error("restored attempt state is inconsistent: {field}")]
    Restore {
        /// The inconsistent part.
        field: &'static str,
    },
}

/// What a recorded ballot closure decided for the attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnonymousBallotDecisionV1 {
    /// A narrow Policy Jury approval: the attempt now needs its Confirmation Jury.
    ConfirmationRequired,
    /// The last required approval: the attempt is certified.
    Certified,
    /// The jury did not approve: the attempt is rejected.
    Rejected(BallotOutcomeV1),
    /// A narrow Policy Jury approval found no fresh Confirmation capacity or redraw unit:
    /// the attempt ends `ConfirmationJuryCapacityUnavailable`.
    ConfirmationCapacityUnavailable,
}

/// Why a public body ended without a finding.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "reason", content = "details", deny_unknown_fields)]
#[norito_schema(
    name = "iroha_core::governance::parliament_anonymous::AnonymousPublicFindingFailureKindV1"
)]
pub enum AnonymousPublicFindingFailureKindV1 {
    /// Authenticated absences or split endorsements made the quorum unreachable.
    #[codec(index = 0)]
    QuorumUnreachable,
    /// The body stayed below quorum past its frozen endorsement window.
    #[codec(index = 1)]
    WindowExpired,
}

/// The objective failure of a public body to produce a finding.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_core::governance::parliament_anonymous::AnonymousPublicFindingFailureV1"
)]
pub struct AnonymousPublicFindingFailureV1 {
    /// The public body.
    pub body: ParliamentBody,
    /// Why it has no finding.
    pub kind: AnonymousPublicFindingFailureKindV1,
    /// Anchor of the `ConcludeFinding` item that recorded the failure.
    pub at: GovernanceAnchorV1,
}

/// The election that left the attempt without a retry.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_core::governance::parliament_anonymous::AnonymousSortitionExhaustionV1"
)]
pub struct AnonymousSortitionExhaustionV1 {
    /// The body whose election ended without a roster; the first body of a generation that
    /// ended as a whole.
    pub body: ParliamentBody,
    /// Why it ended without a roster.
    pub kind: AnonymousNoRosterKindV1,
    /// Anchor of the item that rejected the attempt.
    pub at: GovernanceAnchorV1,
}

/// What rejected an attempt.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "cause", content = "details", deny_unknown_fields)]
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousAttemptRejectionV1")]
pub enum AnonymousAttemptRejectionV1 {
    /// A binding jury closed without approving: its closure record.
    #[codec(index = 0)]
    Ballot(Box<BallotClosureRecordV1>),
    /// A public body ended without a finding.
    #[codec(index = 1)]
    PublicFinding(AnonymousPublicFindingFailureV1),
    /// A body's election ended without a roster and no retry was left.
    #[codec(index = 2)]
    SortitionExhausted(AnonymousSortitionExhaustionV1),
    /// The Policy Jury approved narrowly, and no fresh Confirmation capacity or redraw unit
    /// remained: `ConfirmationJuryCapacityUnavailable`, with that closure record.
    #[codec(index = 3)]
    ConfirmationCapacityUnavailable(Box<BallotClosureRecordV1>),
}

/// How the certified effect of a production attempt ended.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "outcome", content = "details", deny_unknown_fields)]
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousExecutionOutcomeV1")]
pub enum AnonymousExecutionOutcomeV1 {
    /// The exact certified effect was enacted.
    #[codec(index = 0)]
    Enacted,
    /// Another compare-and-set head was authoritative when execution became due.
    #[codec(index = 1)]
    Superseded(GovernanceExpectedHeadV1),
    /// The exact certified effect failed and every partial write was discarded; the root
    /// commits the certificate and the failing pass.
    #[codec(index = 2)]
    ExecutionFailed([u8; 32]),
}

/// The execution of a certified effect: its block-start anchor and its outcome.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousExecutionV1")]
pub struct AnonymousExecutionV1 {
    /// Anchor of the block-start item that executed.
    pub at: GovernanceAnchorV1,
    /// What it decided.
    pub outcome: AnonymousExecutionOutcomeV1,
}

/// Deterministic State of one anonymous Parliament attempt.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_core::governance::parliament_anonymous::AnonymousParliamentAttemptStateV1"
)]
pub struct AnonymousParliamentAttemptStateV1 {
    purpose: ParliamentBallotPurposeV1,
    attempt: AnonymousGovernanceAttemptV1,
    policy: FrozenParliamentAttemptPolicyV1,
    /// `proposal_digest` of the full canonical proposal, fixed at creation; every board and
    /// closure record of the attempt carries it.
    proposal_digest: Digest512,
    effect_preimage_hash: [u8; 32],
    expected_head: GovernanceExpectedHeadV1,
    /// Proposal-wide redraw units its predecessor attempts consumed.
    randomness_redraws_before_attempt: u32,
    risk_locked: bool,
    /// Creation, start and end of qualification.
    qualification: AnonymousQualificationV1,
    /// Every sortition generation, in registration order.
    generations: Vec<election::AnonymousGenerationV1>,
    /// Each sealed body with its phases and finding record.
    bodies: BTreeMap<ParliamentBody, AnonymousBodyStateV1>,
    /// The immutable result of each body that produced an advancing one.
    results: BTreeMap<ParliamentBody, AnonymousParliamentBodyBindingV1>,
    /// What rejected the attempt.
    rejection: Option<AnonymousAttemptRejectionV1>,
    certificate: Option<AnonymousGovernanceCertificateV1>,
    execution: Option<AnonymousExecutionV1>,
}

impl AnonymousParliamentErrorV1 {
    /// Whether the node could not decide (its proof profile, a pinned frame or a verifier
    /// backend), as opposed to the transition being refused. A local error is never a
    /// committed verdict: the node defers or retries instead.
    #[must_use]
    pub fn is_local(&self) -> bool {
        match self {
            Self::Credential(error) => error.is_local(),
            Self::Seal(error) => error.is_local(),
            Self::Board(error) => error.is_local(),
            _ => false,
        }
    }
}

/// Whether `body` decides by an anonymous ballot.
fn is_binding(body: ParliamentBody) -> bool {
    BodyDecisionModeV1::of(body) == BodyDecisionModeV1::AnonymousBallot
}

const fn stage_for_body(body: ParliamentBody) -> GovernanceStageV1 {
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

const fn expected_head_subject(head: GovernanceExpectedHeadV1) -> [u8; 32] {
    match head {
        GovernanceExpectedHeadV1::Absent(head) => head.subject_id,
        GovernanceExpectedHeadV1::Present(head) => head.subject_id,
    }
}

fn expected_head_is_valid(head: GovernanceExpectedHeadV1) -> bool {
    match head {
        GovernanceExpectedHeadV1::Absent(head) => head.subject_id != [0; 32],
        GovernanceExpectedHeadV1::Present(head) => {
            head.subject_id != [0; 32] && head.version != 0 && head.head_root != [0; 32]
        }
    }
}

/// Whether the effect of `certificate` is due in the block-start item anchored `at`: the
/// item follows the certifying closure, and the governance clock reached that closure's
/// time plus the frozen delay.
///
/// A block has one time and its pass processes closures before execution, so an item of
/// the certifying block follows the closure only at the same time and a later position.
fn execution_is_due(
    certificate: &AnonymousGovernanceCertificateV1,
    at: GovernanceAnchorV1,
) -> Result<(), AnonymousParliamentErrorV1> {
    let (Some(certified_at), Some(due_ms)) =
        (certificate.certified_at(), certificate.enact_at_ms())
    else {
        return Err(AnonymousParliamentErrorV1::NotCertified);
    };
    let follows = at.height > certified_at.height
        || (at.height == certified_at.height
            && at.timestamp_ms == certified_at.timestamp_ms
            && at.position > certified_at.position);
    if !follows {
        return Err(AnonymousParliamentErrorV1::ExecutionAnchor);
    }
    if at.timestamp_ms < due_ms {
        return Err(AnonymousParliamentErrorV1::EnactmentNotDue {
            due_ms,
            now_ms: at.timestamp_ms,
        });
    }
    Ok(())
}

/// Whether `observed` supersedes `certificate`: another valid head of the same subject.
fn head_supersedes(
    certificate: &AnonymousGovernanceCertificateV1,
    observed: GovernanceExpectedHeadV1,
) -> Result<(), AnonymousParliamentErrorV1> {
    if observed == certificate.expected_head {
        return Err(AnonymousParliamentErrorV1::ExpectedHeadUnchanged);
    }
    if !expected_head_is_valid(observed)
        || expected_head_subject(observed) != expected_head_subject(certificate.expected_head)
    {
        return Err(AnonymousParliamentErrorV1::InvalidSupersedingHead);
    }
    Ok(())
}

/// The immutable creation inputs of an attempt that the reducer cannot derive itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnonymousAttemptOriginV1 {
    /// Time of the creating block: `StartQualification` is due at it.
    pub created_at_ms: u64,
    /// Proposal-wide redraw units the attempt's predecessors consumed; zero for a
    /// sequence-zero attempt.
    pub randomness_redraws_before_attempt: u32,
}

impl AnonymousParliamentAttemptStateV1 {
    /// Create the State of a new attempt.
    ///
    /// `purpose` is what Core selected for an attempt created now
    /// ([`ParliamentBallotPurposeV1::select_v1`]) and `policy` the snapshot it froze from
    /// committed parameters; the caller of the instruction chooses neither. `origin` is the
    /// creating block's time and the redraw count the predecessor attempt ended with.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for an unrecognized purpose, an invalid snapshot, a
    /// noncanonical or over-limit attempt, an inert binding, a redraw prefix outside the
    /// lineage, or a noninitial snapshot.
    pub fn try_new(
        purpose: ParliamentBallotPurposeV1,
        attempt: AnonymousGovernanceAttemptV1,
        policy: FrozenParliamentAttemptPolicyV1,
        proposal_digest: Digest512,
        effect_preimage_hash: [u8; 32],
        expected_head: GovernanceExpectedHeadV1,
        origin: AnonymousAttemptOriginV1,
    ) -> Result<Self, AnonymousParliamentErrorV1> {
        let state = Self {
            purpose,
            attempt,
            policy,
            proposal_digest,
            effect_preimage_hash,
            expected_head,
            randomness_redraws_before_attempt: origin.randomness_redraws_before_attempt,
            risk_locked: false,
            qualification: AnonymousQualificationV1::created(origin.created_at_ms),
            generations: Vec::new(),
            bodies: BTreeMap::new(),
            results: BTreeMap::new(),
            rejection: None,
            certificate: None,
            execution: None,
        };
        state.validate_bindings()?;
        if attempt.status != GovernanceAttemptStatusV1::Active
            || attempt.stage != GovernanceStageV1::Qualification
        {
            return Err(AnonymousParliamentErrorV1::NotInitial);
        }
        Ok(state)
    }

    /// The purpose, the snapshot and the bindings fixed at creation.
    fn validate_bindings(&self) -> Result<(), AnonymousParliamentErrorV1> {
        self.purpose.validate()?;
        self.policy.validate()?;
        // The family exists for the binding juries: every attempt ends in a Policy Jury.
        if self.policy.binding.is_none() {
            return Err(AnonymousParliamentErrorV1::Policy(
                FrozenBallotPolicyErrorV1::MissingBindingPolicy,
            ));
        }
        let attempt = &self.attempt;
        if attempt.id.as_bytes() == &[0; 32]
            || attempt.proposal_content_id.as_bytes() == &[0; 32]
            || !attempt.has_canonical_id()
            || self.effect_preimage_hash == [0; 32]
            || self.proposal_digest == Digest512::new([0; 64])
            || !expected_head_is_valid(self.expected_head)
        {
            return Err(AnonymousParliamentErrorV1::ImmutableBinding);
        }
        if attempt.sequence > self.policy.sortition_policy.max_governance_attempt_retries {
            return Err(AnonymousParliamentErrorV1::RetryLimitExceeded);
        }
        // A first attempt inherits nothing; a successor inherits a count that leaves it at
        // least one generation (design §8.1).
        let before = self.randomness_redraws_before_attempt;
        let ceiling = self.policy.sortition_policy.max_randomness_redraws;
        if (attempt.sequence == 0 && before != 0) || (attempt.sequence > 0 && before >= ceiling) {
            return Err(AnonymousParliamentErrorV1::RedrawLineage);
        }
        Ok(())
    }

    /// Why the attempt may exist.
    #[must_use]
    pub const fn purpose(&self) -> &ParliamentBallotPurposeV1 {
        &self.purpose
    }

    /// The attempt snapshot.
    #[must_use]
    pub const fn attempt(&self) -> &AnonymousGovernanceAttemptV1 {
        &self.attempt
    }

    /// The policy frozen at admission.
    #[must_use]
    pub const fn policy(&self) -> &FrozenParliamentAttemptPolicyV1 {
        &self.policy
    }

    /// `proposal_digest` of the full canonical proposal, fixed at creation.
    #[must_use]
    pub const fn proposal_digest(&self) -> &Digest512 {
        &self.proposal_digest
    }

    /// Hash of the exact deterministic effect preimage.
    #[must_use]
    pub const fn effect_preimage_hash(&self) -> [u8; 32] {
        self.effect_preimage_hash
    }

    /// Compare-and-set head required when the effect executes.
    #[must_use]
    pub const fn expected_head(&self) -> GovernanceExpectedHeadV1 {
        self.expected_head
    }

    /// Whether the risk tier is locked.
    #[must_use]
    pub const fn risk_locked(&self) -> bool {
        self.risk_locked
    }

    /// Proposal-wide redraw units the attempt's predecessors consumed.
    #[must_use]
    pub const fn randomness_redraws_before_attempt(&self) -> u32 {
        self.randomness_redraws_before_attempt
    }

    /// The sealed instance of `body`.
    #[must_use]
    pub fn sealed_body(&self, body: ParliamentBody) -> Option<&AnonymousSealedBodyV1> {
        self.bodies.get(&body).map(|state| &state.sealed)
    }

    /// The recorded result of `body`.
    #[must_use]
    pub fn result(&self, body: ParliamentBody) -> Option<&AnonymousParliamentBodyBindingV1> {
        self.results.get(&body)
    }

    /// What rejected the attempt.
    #[must_use]
    pub const fn rejection(&self) -> Option<&AnonymousAttemptRejectionV1> {
        self.rejection.as_ref()
    }

    /// The certificate of a certified attempt.
    #[must_use]
    pub const fn certificate(&self) -> Option<&AnonymousGovernanceCertificateV1> {
        self.certificate.as_ref()
    }

    /// The execution of a production attempt's certified effect.
    #[must_use]
    pub const fn execution(&self) -> Option<&AnonymousExecutionV1> {
        self.execution.as_ref()
    }

    /// Whether no transition can change this attempt any more. A candidate-testing attempt
    /// is final at its certificate: nothing executes it.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        match self.attempt.status {
            GovernanceAttemptStatusV1::Active => false,
            GovernanceAttemptStatusV1::Certified => !self.purpose.is_production(),
            GovernanceAttemptStatusV1::Rejected
            | GovernanceAttemptStatusV1::Enacted
            | GovernanceAttemptStatusV1::Superseded
            | GovernanceAttemptStatusV1::ExecutionFailed => true,
        }
    }

    /// Whether a narrow Policy Jury approval requires a Confirmation Jury.
    #[must_use]
    pub fn confirmation_required(&self) -> bool {
        self.results
            .get(&ParliamentBody::PolicyJury)
            .and_then(AnonymousParliamentBodyBindingV1::ballot)
            .is_some_and(AnonymousBallotCertificateBindingV1::requires_confirmation)
    }

    fn ensure_active(
        &self,
        governance_attempt_id: GovernanceAttemptId,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        if governance_attempt_id != self.attempt.id {
            return Err(AnonymousParliamentErrorV1::WrongAttempt);
        }
        if self.attempt.status != GovernanceAttemptStatusV1::Active {
            return Err(AnonymousParliamentErrorV1::NotActive {
                status: self.attempt.status,
            });
        }
        Ok(())
    }

    fn ensure_stage(&self, expected: GovernanceStageV1) -> Result<(), AnonymousParliamentErrorV1> {
        if self.attempt.stage == expected {
            Ok(())
        } else {
            Err(AnonymousParliamentErrorV1::Stage {
                expected,
                actual: self.attempt.stage,
            })
        }
    }

    /// The first stage of the pipeline: the stage of its first required body.
    fn first_stage(&self) -> Result<GovernanceStageV1, AnonymousParliamentErrorV1> {
        self.policy
            .required_bodies
            .first()
            .map(|required| stage_for_body(required.body))
            .ok_or(AnonymousParliamentErrorV1::Policy(
                FrozenBallotPolicyErrorV1::NoRequiredBodies,
            ))
    }

    /// The stage after `body` produced an advancing result. The conditional Confirmation
    /// Jury is entered only after a narrow Policy Jury approval; after the last required
    /// approval the attempt is certified.
    fn stage_after(&self, body: ParliamentBody) -> GovernanceStageV1 {
        match body {
            ParliamentBody::PolicyJury if self.confirmation_required() => {
                GovernanceStageV1::ConfirmationJury
            }
            ParliamentBody::PolicyJury | ParliamentBody::ConfirmationJury => {
                GovernanceStageV1::Certification
            }
            _ => self
                .policy
                .required_bodies
                .iter()
                .map(|required| required.body)
                .find(|next| *next > body)
                .map_or(GovernanceStageV1::Certification, stage_for_body),
        }
    }

    /// Escalate the risk tier before it is locked.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another or an inactive attempt, a locked tier, a
    /// replay or a downgrade.
    pub fn escalate_risk(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        target: RiskTierV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if self.risk_locked {
            return Err(AnonymousParliamentErrorV1::RiskTierLocked);
        }
        if target == self.attempt.risk_tier {
            return Err(AnonymousParliamentErrorV1::RiskEscalationReplay);
        }
        if !self.attempt.risk_tier.can_escalate_to(target) {
            return Err(AnonymousParliamentErrorV1::RiskDowngrade);
        }
        self.attempt.risk_tier = target;
        Ok(())
    }

    /// The rules of one sealed body within this attempt, beyond its own
    /// ([`AnonymousSealedBodyV1::validate`]): a binding jury seals at or above its floor
    /// under a locked tier.
    fn check_seal(&self, sealed: &AnonymousSealedBodyV1) -> Result<(), AnonymousParliamentErrorV1> {
        sealed.validate(self.attempt.id, &self.policy)?;
        if BodyDecisionModeV1::of(sealed.body) == BodyDecisionModeV1::AnonymousBallot {
            if !self.risk_locked {
                return Err(AnonymousParliamentErrorV1::RiskTierNotLocked);
            }
            let minimum = self
                .policy
                .ballot_policy(self.attempt.risk_tier, sealed.body)?
                .minimum_sealed_seats;
            if sealed.seats < minimum {
                return Err(AnonymousParliamentErrorV1::BelowSealFloor {
                    seats: sealed.seats,
                    minimum,
                });
            }
        }
        if sealed.body == ParliamentBody::ConfirmationJury {
            self.check_confirmation(sealed)?;
        }
        Ok(())
    }

    /// A Confirmation Jury is sealed only for a narrow Policy Jury approval, as the fresh
    /// draw that approval admitted ([`AnonymousSealedBodyV1::confirms`]).
    fn check_confirmation(
        &self,
        sealed: &AnonymousSealedBodyV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        let policy = self
            .results
            .get(&ParliamentBody::PolicyJury)
            .filter(|_| self.confirmation_required())
            .ok_or(AnonymousParliamentErrorV1::ConfirmationNotRequired)?;
        if sealed.confirms(&policy.sealed, policy.result_height) {
            Ok(())
        } else {
            Err(AnonymousParliamentErrorV1::Certificate(
                AnonymousGovernanceCertificateErrorV1::ConfirmationJuryMismatch,
            ))
        }
    }

    /// The sealed instance of `body` when the attempt is at that body's stage.
    fn sealed_at_stage(
        &self,
        body: ParliamentBody,
    ) -> Result<&AnonymousSealedBodyV1, AnonymousParliamentErrorV1> {
        self.ensure_stage(stage_for_body(body))?;
        self.sealed_body(body)
            .ok_or(AnonymousParliamentErrorV1::BodyNotSealed { body })
    }

    /// Record the quorum-endorsed finding of the public body at the current stage,
    /// concluded at `at`, and advance to the next required body.
    fn record_public_finding(
        &mut self,
        body: ParliamentBody,
        result: AnonymousPublicFindingResultV1,
        at: GovernanceAnchorV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::PublicFinding {
            return Err(AnonymousParliamentErrorV1::DecisionMode { body });
        }
        let binding = AnonymousParliamentBodyBindingV1 {
            sealed: *self.sealed_at_stage(body)?,
            result_height: at.height,
            result: AnonymousBodyResultV1::PublicFinding(Box::new(result)),
        };
        self.validate_result(&binding)?;
        self.results.insert(body, binding);
        let next = self.stage_after(body);
        self.enter_stage(next, at);
        Ok(())
    }

    /// Record that the public body at the current stage ended without a finding, which
    /// rejects the attempt: no later body can decide for it.
    fn fail_public_finding(
        &mut self,
        failure: AnonymousPublicFindingFailureV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        self.check_public_finding_failure(&failure)?;
        self.attempt.status = GovernanceAttemptStatusV1::Rejected;
        self.rejection = Some(AnonymousAttemptRejectionV1::PublicFinding(failure));
        Ok(())
    }

    /// A failure belongs to the sealed public body at the attempt's stage and follows that
    /// body's sortition pulse.
    fn check_public_finding_failure(
        &self,
        failure: &AnonymousPublicFindingFailureV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        let body = failure.body;
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::PublicFinding {
            return Err(AnonymousParliamentErrorV1::DecisionMode { body });
        }
        let sealed = self.sealed_at_stage(body)?;
        if failure.at.height <= sealed.sortition_request.pulse_height {
            return Err(AnonymousParliamentErrorV1::FailureBeforeSortition);
        }
        Ok(())
    }

    fn validate_result(
        &self,
        binding: &AnonymousParliamentBodyBindingV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        binding.validate(
            self.attempt.proposal_content_id,
            self.attempt.id,
            self.attempt.risk_tier,
            &self.policy,
        )?;
        Ok(())
    }

    /// G2b `CloseBallot` of the binding jury at the current stage: record its closure.
    ///
    /// An approval is the jury's immutable result. A narrow Policy Jury approval registers
    /// the Confirmation Jury's first generation atomically and moves the attempt to it; its
    /// snapshot is `eligible`, the canonical citizens eligible at the closure, less every
    /// sealed Policy Jury member. When fewer than three citizens remain or the redraw budget
    /// is spent, the attempt ends `ConfirmationJuryCapacityUnavailable` with no result
    /// recorded. Core computes `eligible` only for that registration. Any other approval
    /// certifies the attempt. A rejection or an inquorate closure rejects it: a binding jury
    /// has exactly one ballot.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another or an inactive attempt, a jury that is
    /// not at its stage or not sealed, a public body, a record of another sealed body, a
    /// ballot that opened before the jury's Reflection and notice, a record that fails its
    /// policy, an invalid Confirmation registration, or a certificate that fails as a whole.
    pub fn close_ballot<F>(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        closure: BallotClosureRecordV1,
        network_id: &NetworkId,
        eligible: F,
    ) -> Result<AnonymousBallotDecisionV1, AnonymousParliamentErrorV1>
    where
        F: FnOnce() -> Vec<AccountId>,
    {
        self.ensure_active(governance_attempt_id)?;
        self.check_anchor(closure.closure_anchor)?;
        let body = closure.body_role;
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::AnonymousBallot {
            return Err(AnonymousParliamentErrorV1::DecisionMode { body });
        }
        let sealed = *self.sealed_at_stage(body)?;
        self.check_closure(&sealed, &closure, network_id)?;
        let policy = self.policy.ballot_policy(self.attempt.risk_tier, body)?;
        if closure.outcome != BallotOutcomeV1::Approved {
            self.attempt.status = GovernanceAttemptStatusV1::Rejected;
            self.rejection = Some(AnonymousAttemptRejectionV1::Ballot(Box::new(closure)));
            return Ok(AnonymousBallotDecisionV1::Rejected(closure.outcome));
        }

        let at = closure.closure_anchor;
        let binding = AnonymousParliamentBodyBindingV1 {
            sealed,
            result_height: at.height,
            result: AnonymousBodyResultV1::Ballot(Box::new(
                AnonymousBallotCertificateBindingV1::approve(closure, &policy)?,
            )),
        };
        self.validate_result(&binding)?;
        // Every effect is all or nothing: a Confirmation registration or a certificate that
        // fails leaves the attempt exactly as it was.
        let mut next = self.clone();
        next.results.insert(body, binding);
        let stage = next.stage_after(body);
        if stage == GovernanceStageV1::ConfirmationJury {
            if !next.register_confirmation(network_id, at, eligible())? {
                self.attempt.status = GovernanceAttemptStatusV1::Rejected;
                self.rejection = Some(
                    AnonymousAttemptRejectionV1::ConfirmationCapacityUnavailable(Box::new(closure)),
                );
                return Ok(AnonymousBallotDecisionV1::ConfirmationCapacityUnavailable);
            }
            next.enter_stage(stage, at);
            *self = next;
            return Ok(AnonymousBallotDecisionV1::ConfirmationRequired);
        }
        let certificate = next.build_certificate();
        certificate.validate()?;
        next.certificate = Some(certificate);
        next.attempt.status = GovernanceAttemptStatusV1::Certified;
        next.attempt.stage = GovernanceStageV1::Enactment;
        *self = next;
        Ok(AnonymousBallotDecisionV1::Certified)
    }

    /// The jury's ballot opened in a pass after its Reflection entry and no earlier than its
    /// opening time (design §3.4, §8.1).
    fn check_ballot_chronology(
        &self,
        body: ParliamentBody,
        closure: &BallotClosureRecordV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        let state = self
            .bodies
            .get(&body)
            .ok_or(AnonymousParliamentErrorV1::BodyNotSealed { body })?;
        let reflection = state
            .entered(DeliberationPhaseV1::Reflection)
            .ok_or(AnonymousParliamentErrorV1::BallotChronology)?;
        let opening_at = self
            .opening_at_ms(body)
            .ok_or(AnonymousParliamentErrorV1::BallotChronology)?;
        let opened = closure.opening_anchor;
        if (opened.height, opened.position) <= (reflection.height, reflection.position)
            || opened.timestamp_ms < opening_at
        {
            return Err(AnonymousParliamentErrorV1::BallotChronology);
        }
        Ok(())
    }

    /// A closure record, whatever its outcome, is the record of `sealed` in this attempt on
    /// `network_id`, over the list its seal bound, opened after the body was drawn and after
    /// its Reflection and notice, and valid under the locked ballot policy.
    fn check_closure(
        &self,
        sealed: &AnonymousSealedBodyV1,
        closure: &BallotClosureRecordV1,
        network_id: &NetworkId,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        let context = |field| {
            AnonymousParliamentErrorV1::Certificate(
                AnonymousGovernanceCertificateErrorV1::BallotContext { field },
            )
        };
        if let Some(field) =
            sealed.closure_mismatch(self.attempt.proposal_content_id, self.attempt.id, closure)
        {
            return Err(context(field));
        }
        if closure.network_id != *network_id {
            return Err(context("network_id"));
        }
        if closure.proposal_digest != self.proposal_digest {
            return Err(context("proposal_digest"));
        }
        let binding = self
            .bodies
            .get(&sealed.body)
            .and_then(|state| state.roster_binding)
            .ok_or(AnonymousParliamentErrorV1::BodyNotSealed { body: sealed.body })?;
        if closure.list_digest != binding.list_digest {
            return Err(context("list_digest"));
        }
        if closure.profile_id != binding.profile_id {
            return Err(context("profile_id"));
        }
        self.check_ballot_chronology(sealed.body, closure)?;
        let policy = self
            .policy
            .ballot_policy(self.attempt.risk_tier, sealed.body)?;
        closure
            .validate(&policy)
            .map_err(AnonymousBallotBindingErrorV1::Closure)?;
        Ok(())
    }

    /// The certificate the recorded results give.
    fn build_certificate(&self) -> AnonymousGovernanceCertificateV1 {
        AnonymousGovernanceCertificateV1 {
            purpose: self.purpose,
            proposal_content_id: self.attempt.proposal_content_id,
            governance_attempt_id: self.attempt.id,
            governance_attempt_sequence: self.attempt.sequence,
            risk_tier: self.attempt.risk_tier,
            policy: self.policy.clone(),
            body_bindings: self.results.values().cloned().collect(),
            effect_preimage_hash: self.effect_preimage_hash,
            expected_head: self.expected_head,
        }
    }

    /// The governance-clock time from which the certified effect of a production attempt
    /// is due. A candidate-testing certificate is never due.
    #[must_use]
    pub fn enactment_due_ms(&self) -> Option<u64> {
        if self.attempt.status != GovernanceAttemptStatusV1::Certified {
            return None;
        }
        let certificate = self.certificate.as_ref()?;
        certificate.require_production().ok()?;
        certificate.enact_at_ms()
    }

    /// The certificate when its effect may execute in the block-start item anchored `at`.
    ///
    /// This is the gate of production enactment: a candidate-testing certificate is
    /// refused here, before any effect is considered.
    fn certificate_for_execution(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        at: GovernanceAnchorV1,
    ) -> Result<&AnonymousGovernanceCertificateV1, AnonymousParliamentErrorV1> {
        if governance_attempt_id != self.attempt.id {
            return Err(AnonymousParliamentErrorV1::WrongAttempt);
        }
        let certificate = match (&self.certificate, self.attempt.status) {
            (Some(certificate), GovernanceAttemptStatusV1::Certified) => certificate,
            _ => return Err(AnonymousParliamentErrorV1::NotCertified),
        };
        certificate.require_production()?;
        execution_is_due(certificate, at)?;
        Ok(certificate)
    }

    fn finish_execution(
        &mut self,
        status: GovernanceAttemptStatusV1,
        at: GovernanceAnchorV1,
        outcome: AnonymousExecutionOutcomeV1,
    ) {
        self.attempt.status = status;
        self.execution = Some(AnonymousExecutionV1 { at, outcome });
    }

    /// Mark the due certified effect enacted.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another attempt, an attempt without a certificate
    /// awaiting execution, a certificate without production authority, or an early pass.
    pub fn mark_enacted(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        at: GovernanceAnchorV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        self.certificate_for_execution(governance_attempt_id, at)?;
        self.finish_execution(
            GovernanceAttemptStatusV1::Enacted,
            at,
            AnonymousExecutionOutcomeV1::Enacted,
        );
        Ok(())
    }

    /// Mark the due certificate superseded by another compare-and-set head.
    ///
    /// # Errors
    /// As [`Self::mark_enacted`], and for an unchanged, inert or cross-subject head.
    pub fn mark_superseded(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        at: GovernanceAnchorV1,
        observed_head: GovernanceExpectedHeadV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        let certificate = self.certificate_for_execution(governance_attempt_id, at)?;
        head_supersedes(certificate, observed_head)?;
        self.finish_execution(
            GovernanceAttemptStatusV1::Superseded,
            at,
            AnonymousExecutionOutcomeV1::Superseded(observed_head),
        );
        Ok(())
    }

    /// Mark deterministic execution failure of the due certified effect and return the
    /// failure root, which commits the certificate and the failing pass.
    ///
    /// # Errors
    /// As [`Self::mark_enacted`].
    pub fn mark_execution_failed(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        at: GovernanceAnchorV1,
    ) -> Result<[u8; 32], AnonymousParliamentErrorV1> {
        let certificate = self.certificate_for_execution(governance_attempt_id, at)?;
        let failure_root = anonymous_execution_failure_root_v1(certificate, at);
        self.finish_execution(
            GovernanceAttemptStatusV1::ExecutionFailed,
            at,
            AnonymousExecutionOutcomeV1::ExecutionFailed(failure_root),
        );
        Ok(failure_root)
    }

    /// Check a restored State of an attempt on the network `network_id`: it is exactly one
    /// this reducer produces.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for the first rule a restored State violates.
    pub fn validate(&self, network_id: &NetworkId) -> Result<(), AnonymousParliamentErrorV1> {
        use AnonymousParliamentErrorV1 as Error;
        use GovernanceAttemptStatusV1 as Status;

        self.validate_bindings()?;
        self.validate_restored_bodies(network_id)?;

        // The results are exactly the bodies the pipeline passed.
        let passed = self.passed_bodies()?;
        if !self.results.keys().copied().eq(passed.iter().copied()) {
            return Err(Error::Restore { field: "results" });
        }
        if self.rejection.is_some() != (self.attempt.status == Status::Rejected) {
            return Err(Error::Restore { field: "rejection" });
        }
        if let Some(rejection) = &self.rejection {
            self.validate_restored_rejection(rejection, network_id)?;
        }

        let certified = !matches!(self.attempt.status, Status::Active | Status::Rejected);
        if certified != self.certificate.is_some()
            || self
                .certificate
                .as_ref()
                .is_some_and(|certificate| *certificate != self.build_certificate())
        {
            return Err(Error::Restore {
                field: "certificate",
            });
        }
        if let Some(certificate) = &self.certificate {
            certificate.validate()?;
        }
        self.validate_restored_execution()?;
        self.validate_lifecycle(network_id)
    }

    /// Every sealed body, and every result against the sealed body it belongs to.
    fn validate_restored_bodies(
        &self,
        network_id: &NetworkId,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        use AnonymousParliamentErrorV1 as Error;

        for (body, state) in &self.bodies {
            if *body != state.sealed.body {
                return Err(Error::Restore { field: "bodies" });
            }
            self.check_seal(&state.sealed)?;
        }
        for (body, binding) in &self.results {
            if self.sealed_body(*body) != Some(&binding.sealed) {
                return Err(Error::Restore { field: "results" });
            }
            self.validate_result(binding)?;
            // A ballot result opened after its jury's Reflection and notice.
            if let Some(ballot) = binding.ballot() {
                self.check_closure(&binding.sealed, &ballot.closure, network_id)?;
            }
        }
        Ok(())
    }

    /// The bodies whose results the attempt's stage implies, in pipeline order.
    fn passed_bodies(&self) -> Result<Vec<ParliamentBody>, AnonymousParliamentErrorV1> {
        use AnonymousParliamentErrorV1 as Error;
        use GovernanceAttemptStatusV1 as Status;

        let stage = self.attempt.stage;
        let stage_is_coherent = match self.attempt.status {
            Status::Active => {
                stage == GovernanceStageV1::Qualification
                    || self
                        .policy
                        .required_bodies
                        .iter()
                        .any(|required| stage_for_body(required.body) == stage)
            }
            // The stage of the body that rejected the attempt; the rejection names it.
            Status::Rejected => self
                .policy
                .required_bodies
                .iter()
                .any(|required| stage_for_body(required.body) == stage),
            Status::Certified | Status::Enacted | Status::Superseded | Status::ExecutionFailed => {
                stage == GovernanceStageV1::Enactment
            }
        };
        if !stage_is_coherent {
            return Err(Error::Restore { field: "stage" });
        }
        let passed: Vec<_> = self
            .policy
            .required_bodies
            .iter()
            .map(|required| required.body)
            .filter(|body| {
                stage_for_body(*body) < stage
                    && (*body != ParliamentBody::ConfirmationJury
                        || self.results.contains_key(body))
            })
            .collect();
        // A Confirmation result exists exactly when the narrow approval it confirms does;
        // the attempt is at the Confirmation stage only for such an approval.
        let confirmation_passed = passed.contains(&ParliamentBody::ConfirmationJury);
        let at_confirmation = stage == GovernanceStageV1::ConfirmationJury;
        if (confirmation_passed || at_confirmation) && !self.confirmation_required() {
            return Err(Error::ConfirmationNotRequired);
        }
        if stage == GovernanceStageV1::Enactment
            && self.confirmation_required()
            && !confirmation_passed
        {
            return Err(Error::Restore { field: "results" });
        }
        Ok(passed)
    }

    /// The rejection is the verdict of the body at the stage the attempt stopped at: a
    /// valid, non-approving closure of that jury, or the failure of that public body.
    fn validate_restored_rejection(
        &self,
        rejection: &AnonymousAttemptRejectionV1,
        network_id: &NetworkId,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        use AnonymousParliamentErrorV1 as Error;

        match rejection {
            AnonymousAttemptRejectionV1::Ballot(closure) => {
                let body = closure.body_role;
                if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::AnonymousBallot
                    || closure.outcome == BallotOutcomeV1::Approved
                {
                    return Err(Error::Restore { field: "rejection" });
                }
                let sealed = self.sealed_at_stage(body)?;
                self.check_closure(sealed, closure, network_id)
            }
            AnonymousAttemptRejectionV1::PublicFinding(failure) => {
                self.check_public_finding_failure(failure)
            }
            AnonymousAttemptRejectionV1::SortitionExhausted(exhaustion) => {
                self.validate_restored_exhaustion(exhaustion)
            }
            AnonymousAttemptRejectionV1::ConfirmationCapacityUnavailable(closure) => {
                let policy = self
                    .policy
                    .ballot_policy(self.attempt.risk_tier, ParliamentBody::PolicyJury)?;
                let approval = AnonymousBallotCertificateBindingV1::approve(**closure, &policy)?;
                let sealed = self.sealed_at_stage(ParliamentBody::PolicyJury)?;
                if closure.body_role != ParliamentBody::PolicyJury
                    || !approval.requires_confirmation()
                {
                    return Err(Error::Restore { field: "rejection" });
                }
                self.check_closure(sealed, closure, network_id)
            }
        }
    }

    /// The execution record against the status: present exactly for an executed attempt,
    /// of the matching outcome, due, and only with production authority.
    fn validate_restored_execution(&self) -> Result<(), AnonymousParliamentErrorV1> {
        use AnonymousExecutionOutcomeV1 as Outcome;
        use AnonymousParliamentErrorV1 as Error;
        use GovernanceAttemptStatusV1 as Status;

        let mismatch = Error::Restore { field: "execution" };
        let (Some(execution), Some(certificate)) = (&self.execution, &self.certificate) else {
            let executed = matches!(
                self.attempt.status,
                Status::Enacted | Status::Superseded | Status::ExecutionFailed
            );
            return if executed || self.execution.is_some() {
                Err(mismatch)
            } else {
                Ok(())
            };
        };
        certificate.require_production()?;
        let due = execution_is_due(certificate, execution.at).is_ok();
        let outcome_matches = match (self.attempt.status, execution.outcome) {
            (Status::Enacted, Outcome::Enacted) => true,
            (Status::Superseded, Outcome::Superseded(observed)) => {
                head_supersedes(certificate, observed).is_ok()
            }
            (Status::ExecutionFailed, Outcome::ExecutionFailed(root)) => {
                root == anonymous_execution_failure_root_v1(certificate, execution.at)
            }
            _ => false,
        };
        if due && outcome_matches {
            Ok(())
        } else {
            Err(mismatch)
        }
    }
}

#[cfg(test)]
mod credentials_tests;
#[cfg(test)]
mod interface_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod test_driver;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod world_tests;
