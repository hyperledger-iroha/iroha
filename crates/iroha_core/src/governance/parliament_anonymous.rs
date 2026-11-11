//! Reducer of one attempt of the anonymous Parliament definition family
//! (`specs/parliament_private_ballot_design.md` §12.3).
//!
//! An anonymous attempt is created through its own interface and keeps this State from
//! creation. It shares no State with the frozen baseline reducer ([`super::parliament`]):
//! its identifiers come from the family's own derivations, every body is sealed under the
//! 64-byte roster root, a binding jury contributes the closure record of its one ballot,
//! and there is no ballot retry, timed-OVN phase or TLE session.
//!
//! This module is the attempt's spine:
//!
//! - **Purpose.** The attempt records why it may exist, from creation, and the purpose
//!   travels into its certificate. A candidate-testing attempt ends at its certificate:
//!   every execution outcome requires production authority, so a candidate certificate is
//!   never enacted, superseded or failed as a production effect.
//! - **Policy.** The policy snapshot frozen at admission is the only policy the attempt
//!   reads. The risk tier is locked before the first binding jury is sealed, and both
//!   ballot policies derive from the snapshot and the locked tier.
//! - **Results.** Bodies are sealed in any order and produce results in pipeline order. A
//!   public body's quorum-endorsed finding and a binding jury's approved closure advance
//!   the attempt; any other closure outcome rejects it. A narrow Policy Jury approval
//!   requires the approval of a Confirmation Jury before certification.
//! - **Certificate and execution.** The certificate is built from the recorded results
//!   alone and checked as a whole. Enactment is due on the governance clock once it
//!   reaches the certifying closure's time plus the frozen enactment delay.
//!
//! Nothing dispatches this reducer yet: no instruction is registered and no State table
//! holds it.
//!
//! TODO(E2, design §12.3 slice 1): the election machine (sortition requests and pulses,
//! invitations, `NoRoster` retry generations, the proposal-wide randomness budget and the
//! atomic Confirmation capacity check) and the deliberation machine (phases, endorsements
//! and the public-finding expiry) are not here yet. They produce the sealed bodies and
//! findings this spine records, and lock the risk tier at the first Policy Jury request.
//! TODO(E2, slice 2): the credential-aware seal checks a sealed body's roster-root
//! preimage and credential list before [`AnonymousParliamentAttemptStateV1::seal_body`].
//! TODO(E2, slice 3): closures reach
//! [`AnonymousParliamentAttemptStateV1::record_ballot_closure`] only from the positioned
//! block-start pass, and execution outcomes only from its enactment item.

use std::collections::BTreeMap;

use iroha_data_model::governance::{
    GovernanceAnchorV1,
    anonymous_ballot::{
        AnonymousBallotBindingErrorV1, AnonymousBallotCertificateBindingV1, AnonymousBodyResultV1,
        AnonymousGovernanceAttemptV1, AnonymousGovernanceCertificateErrorV1,
        AnonymousGovernanceCertificateV1, AnonymousParliamentBodyBindingV1,
        AnonymousPublicFindingResultV1, AnonymousSealedBodyV1, BallotClosureRecordV1,
        BallotOutcomeV1, BodyDecisionModeV1, FrozenBallotPolicyErrorV1,
        FrozenParliamentAttemptPolicyV1, ParliamentBallotPurposeErrorV1, ParliamentBallotPurposeV1,
        anonymous_execution_failure_root_v1,
    },
    types::{
        GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceExpectedHeadV1,
        GovernanceStageV1, ParliamentBody, RiskTierV1,
    },
};
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
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
    /// Height of the block that recorded the failure.
    pub height: u64,
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
    effect_preimage_hash: [u8; 32],
    expected_head: GovernanceExpectedHeadV1,
    risk_locked: bool,
    /// The sealed instance of each body that has one.
    sealed: BTreeMap<ParliamentBody, AnonymousSealedBodyV1>,
    /// The immutable result of each body that produced an advancing one.
    results: BTreeMap<ParliamentBody, AnonymousParliamentBodyBindingV1>,
    /// What rejected the attempt.
    rejection: Option<AnonymousAttemptRejectionV1>,
    certificate: Option<AnonymousGovernanceCertificateV1>,
    execution: Option<AnonymousExecutionV1>,
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

impl AnonymousParliamentAttemptStateV1 {
    /// Create the State of a new attempt.
    ///
    /// `purpose` is what Core selected for an attempt created now
    /// ([`ParliamentBallotPurposeV1::select_v1`]) and `policy` the snapshot it froze from
    /// committed parameters; the caller of the instruction chooses neither.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for an unrecognized purpose, an invalid snapshot, a
    /// noncanonical or over-limit attempt, an inert binding, or a noninitial snapshot.
    pub fn try_new(
        purpose: ParliamentBallotPurposeV1,
        attempt: AnonymousGovernanceAttemptV1,
        policy: FrozenParliamentAttemptPolicyV1,
        effect_preimage_hash: [u8; 32],
        expected_head: GovernanceExpectedHeadV1,
    ) -> Result<Self, AnonymousParliamentErrorV1> {
        let state = Self {
            purpose,
            attempt,
            policy,
            effect_preimage_hash,
            expected_head,
            risk_locked: false,
            sealed: BTreeMap::new(),
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
            || !expected_head_is_valid(self.expected_head)
        {
            return Err(AnonymousParliamentErrorV1::ImmutableBinding);
        }
        if attempt.sequence > self.policy.sortition_policy.max_governance_attempt_retries {
            return Err(AnonymousParliamentErrorV1::RetryLimitExceeded);
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

    /// The sealed instance of `body`.
    #[must_use]
    pub fn sealed_body(&self, body: ParliamentBody) -> Option<&AnonymousSealedBodyV1> {
        self.sealed.get(&body)
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

    /// Finish qualification and enter the first required body.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another or an inactive attempt, or a replay.
    pub fn complete_qualification(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        self.ensure_stage(GovernanceStageV1::Qualification)?;
        self.attempt.stage = self.first_stage()?;
        Ok(())
    }

    /// Lock the risk tier: the first Policy Jury sortition request freezes it, and both
    /// ballot policies are the snapshot under that tier from then on.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another or an inactive attempt, a tier already
    /// locked, or a snapshot that yields no ballot policy under the tier.
    pub fn lock_risk(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if self.risk_locked {
            return Err(AnonymousParliamentErrorV1::RiskTierLocked);
        }
        self.policy
            .binding_ballot_policies(self.attempt.risk_tier)?;
        self.risk_locked = true;
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

    /// Record the sealed instance of a body. Bodies seal in any order; each seals once.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another or an inactive attempt, a body already
    /// sealed, a sealed body that fails its own rules, a binding jury below its floor or
    /// before the risk lock, or a Confirmation Jury no narrow approval admitted.
    pub fn seal_body(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        sealed: AnonymousSealedBodyV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let body = sealed.body;
        if self.sealed.contains_key(&body) {
            return Err(AnonymousParliamentErrorV1::BodySealed { body });
        }
        self.check_seal(&sealed)?;
        self.sealed.insert(body, sealed);
        Ok(())
    }

    /// The sealed instance of `body` when the attempt is at that body's stage.
    fn sealed_at_stage(
        &self,
        body: ParliamentBody,
    ) -> Result<&AnonymousSealedBodyV1, AnonymousParliamentErrorV1> {
        self.ensure_stage(stage_for_body(body))?;
        self.sealed
            .get(&body)
            .ok_or(AnonymousParliamentErrorV1::BodyNotSealed { body })
    }

    /// Record the quorum-endorsed finding of the public body at the current stage and
    /// advance to the next required body.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another or an inactive attempt, a body that is
    /// not at its stage or not sealed, a binding jury, or a finding that fails its rules.
    pub fn record_public_finding(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body: ParliamentBody,
        result: AnonymousPublicFindingResultV1,
        result_height: u64,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::PublicFinding {
            return Err(AnonymousParliamentErrorV1::DecisionMode { body });
        }
        let binding = AnonymousParliamentBodyBindingV1 {
            sealed: *self.sealed_at_stage(body)?,
            result_height,
            result: AnonymousBodyResultV1::PublicFinding(Box::new(result)),
        };
        self.validate_result(&binding)?;
        self.results.insert(body, binding);
        self.attempt.stage = self.stage_after(body);
        Ok(())
    }

    /// Record that the public body at the current stage ended without a finding, which
    /// rejects the attempt: no later body can decide for it.
    ///
    /// The deliberation machine decides when a quorum is unreachable or the endorsement
    /// window expired; this records its objective verdict.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another or an inactive attempt, a body that is
    /// not at its stage or not sealed, a binding jury, or a height at which no pulse could
    /// have drawn the body.
    pub fn fail_public_finding(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        failure: AnonymousPublicFindingFailureV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        self.ensure_active(governance_attempt_id)?;
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
        if failure.height <= sealed.sortition_request.pulse_height {
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

    /// Record the closure of the binding jury at the current stage.
    ///
    /// An approval is the jury's immutable result. A narrow Policy Jury approval moves the
    /// attempt to its Confirmation Jury; any other approval certifies it. A rejection or an
    /// inquorate closure rejects the attempt: a binding jury has exactly one ballot.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for another or an inactive attempt, a jury that is
    /// not at its stage or not sealed, a public body, a record of another sealed body, a
    /// record that fails its policy, or a certificate that fails as a whole.
    pub fn record_ballot_closure(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        closure: BallotClosureRecordV1,
    ) -> Result<AnonymousBallotDecisionV1, AnonymousParliamentErrorV1> {
        self.ensure_active(governance_attempt_id)?;
        let body = closure.body_role;
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::AnonymousBallot {
            return Err(AnonymousParliamentErrorV1::DecisionMode { body });
        }
        let sealed = *self.sealed_at_stage(body)?;
        self.check_closure(&sealed, &closure)?;
        let policy = self.policy.ballot_policy(self.attempt.risk_tier, body)?;
        if closure.outcome != BallotOutcomeV1::Approved {
            self.attempt.status = GovernanceAttemptStatusV1::Rejected;
            self.rejection = Some(AnonymousAttemptRejectionV1::Ballot(Box::new(closure)));
            return Ok(AnonymousBallotDecisionV1::Rejected(closure.outcome));
        }

        let binding = AnonymousParliamentBodyBindingV1 {
            sealed,
            result_height: closure.closure_anchor.height,
            result: AnonymousBodyResultV1::Ballot(Box::new(
                AnonymousBallotCertificateBindingV1::approve(closure, &policy)?,
            )),
        };
        self.validate_result(&binding)?;
        // Certification is all or nothing: a certificate that fails as a whole leaves the
        // attempt exactly as it was.
        let mut next = self.clone();
        next.results.insert(body, binding);
        next.attempt.stage = next.stage_after(body);
        let decision = if next.attempt.stage == GovernanceStageV1::Certification {
            let certificate = next.build_certificate();
            certificate.validate()?;
            next.certificate = Some(certificate);
            next.attempt.status = GovernanceAttemptStatusV1::Certified;
            next.attempt.stage = GovernanceStageV1::Enactment;
            AnonymousBallotDecisionV1::Certified
        } else {
            AnonymousBallotDecisionV1::ConfirmationRequired
        };
        *self = next;
        Ok(decision)
    }

    /// A closure record, whatever its outcome, is the record of `sealed` in this attempt,
    /// opened after the body was drawn, and valid under the locked ballot policy.
    fn check_closure(
        &self,
        sealed: &AnonymousSealedBodyV1,
        closure: &BallotClosureRecordV1,
    ) -> Result<(), AnonymousParliamentErrorV1> {
        if let Some(field) =
            sealed.closure_mismatch(self.attempt.proposal_content_id, self.attempt.id, closure)
        {
            return Err(AnonymousParliamentErrorV1::Certificate(
                AnonymousGovernanceCertificateErrorV1::BallotContext { field },
            ));
        }
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

    /// Check a restored State: it is exactly one this reducer produces.
    ///
    /// # Errors
    /// [`AnonymousParliamentErrorV1`] for the first rule a restored State violates.
    pub fn validate(&self) -> Result<(), AnonymousParliamentErrorV1> {
        use AnonymousParliamentErrorV1 as Error;
        use GovernanceAttemptStatusV1 as Status;

        self.validate_bindings()?;
        self.validate_restored_bodies()?;

        // The results are exactly the bodies the pipeline passed.
        let passed = self.passed_bodies()?;
        if !self.results.keys().copied().eq(passed.iter().copied()) {
            return Err(Error::Restore { field: "results" });
        }
        if self.rejection.is_some() != (self.attempt.status == Status::Rejected) {
            return Err(Error::Restore { field: "rejection" });
        }
        if let Some(rejection) = &self.rejection {
            self.validate_restored_rejection(rejection)?;
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
        self.validate_restored_execution()
    }

    /// Every sealed body, and every result against the sealed body it belongs to.
    fn validate_restored_bodies(&self) -> Result<(), AnonymousParliamentErrorV1> {
        use AnonymousParliamentErrorV1 as Error;

        for (body, sealed) in &self.sealed {
            if *body != sealed.body {
                return Err(Error::Restore { field: "sealed" });
            }
            self.check_seal(sealed)?;
        }
        for (body, binding) in &self.results {
            if self.sealed.get(body) != Some(&binding.sealed) {
                return Err(Error::Restore { field: "results" });
            }
            self.validate_result(binding)?;
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
                self.check_closure(sealed, closure)
            }
            AnonymousAttemptRejectionV1::PublicFinding(failure) => {
                self.check_public_finding_failure(failure)
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
mod tests;
