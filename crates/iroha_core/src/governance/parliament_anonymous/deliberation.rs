//! Qualification, deliberation and public findings of the anonymous family (design §8.1).
//!
//! Every wait has one millisecond deadline, fixed from the anchor of the pass item that
//! started it and never moved:
//!
//! ```text
//! creation (tx) -> StartQualification -> EndQualification -> [per body, when current and
//! sealed] Orientation -> Evidence -> Questions -> Responses -> Deliberation -> Reflection
//!     public body:  endorsements and absences (tx) -> ConcludeFinding
//!     binding jury: opening_at_ms -> its ballot (positioned pass, slice 3)
//! ```
//!
//! An authorized early-close intent stores its own time beside the deadline it shortens
//! and is bound to the exact phase it closes. A public body is decided by the transaction
//! whose endorsement brings one root to quorum, which freezes that root and its endorsing
//! assignments as the witness, or by the absence or endorsement after which no root can
//! reach quorum; its conclusion then falls due in the next pass.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    account::AccountId,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            AnonymousClosePhaseEarlyV1, AnonymousPublicFindingResultV1, AnonymousRosterBindingV1,
            AnonymousSealedBodyV1, BodyDecisionModeV1,
        },
        types::{
            AssignmentId, BodyInstanceId, DeliberationPhaseV1, GovernanceAttemptId,
            GovernanceStageV1, ParliamentBody, ParliamentPublicFindingCertificateBindingV1,
            ParliamentSeatAssignmentV1, parliament_public_finding_endorsement_root_v1,
            parliament_quorum_seats_v1,
        },
    },
};
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

use super::{
    AnonymousParliamentAttemptStateV1, AnonymousParliamentErrorV1 as Error,
    AnonymousPublicFindingFailureKindV1, AnonymousPublicFindingFailureV1, stage_for_body,
};

/// The qualification of an attempt: created by a transaction, started and ended by the
/// pass.
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
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousQualificationV1")]
pub struct AnonymousQualificationV1 {
    /// Time of the block that created the attempt: `StartQualification` is due at it.
    pub(super) created_at_ms: u64,
    /// Anchor of `StartQualification`.
    pub(super) started: Option<GovernanceAnchorV1>,
    /// Time of the `CloseQualificationEarly` intent.
    pub(super) early_close_ms: Option<u64>,
    /// Anchor of `EndQualification`.
    pub(super) ended: Option<GovernanceAnchorV1>,
}

impl AnonymousQualificationV1 {
    /// Qualification of an attempt created in a block of time `created_at_ms`.
    pub(super) const fn created(created_at_ms: u64) -> Self {
        Self {
            created_at_ms,
            started: None,
            early_close_ms: None,
            ended: None,
        }
    }

    /// Time of the creating block.
    #[must_use]
    pub const fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }

    /// Anchor of `StartQualification`.
    #[must_use]
    pub const fn started(&self) -> Option<GovernanceAnchorV1> {
        self.started
    }

    /// Time of the early-close intent.
    #[must_use]
    pub const fn early_close_ms(&self) -> Option<u64> {
        self.early_close_ms
    }

    /// Anchor of `EndQualification`.
    #[must_use]
    pub const fn ended(&self) -> Option<GovernanceAnchorV1> {
        self.ended
    }
}

/// One phase a sealed body entered.
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
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousPhaseV1")]
pub struct AnonymousPhaseV1 {
    /// The phase.
    pub phase: DeliberationPhaseV1,
    /// Anchor of the pass item that entered it.
    pub entered_at: GovernanceAnchorV1,
    /// Time of the early-close intent for this phase.
    pub early_close_ms: Option<u64>,
}

/// What decided a public body.
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
#[norito(tag = "verdict", content = "details", deny_unknown_fields)]
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousFindingVerdictV1")]
pub enum AnonymousFindingVerdictV1 {
    /// One root reached quorum: the root and exactly its quorum of endorsing assignments,
    /// in canonical order.
    #[codec(index = 0)]
    Quorum {
        /// The endorsed result root.
        result_root: [u8; 32],
        /// The endorsing assignments.
        witness: Vec<AssignmentId>,
    },
    /// No root can reach quorum any more.
    #[codec(index = 1)]
    Unreachable,
}

/// The decisive fact of a public body and the time of the block that recorded it.
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
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousFindingDecisionV1")]
pub struct AnonymousFindingDecisionV1 {
    /// What decided the body.
    pub verdict: AnonymousFindingVerdictV1,
    /// Time of the deciding block: the conclusion is due at it.
    pub decided_ms: u64,
}

/// The account-signed record of a public body: absences, endorsements and the decision.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousFindingV1")]
pub struct AnonymousFindingV1 {
    /// Seats whose members declared their own absence.
    pub(super) excluded: BTreeSet<AssignmentId>,
    /// The root each endorsing seat endorsed.
    pub(super) endorsements: BTreeMap<AssignmentId, [u8; 32]>,
    /// The decisive fact, once one occurred.
    pub(super) decision: Option<AnonymousFindingDecisionV1>,
    /// Anchor of the `ConcludeFinding` item that concluded the body.
    pub(super) concluded_at: Option<GovernanceAnchorV1>,
}

impl AnonymousFindingV1 {
    /// Seats excluded by their members' absence.
    #[must_use]
    pub const fn excluded(&self) -> &BTreeSet<AssignmentId> {
        &self.excluded
    }

    /// Endorsements by seat.
    #[must_use]
    pub const fn endorsements(&self) -> &BTreeMap<AssignmentId, [u8; 32]> {
        &self.endorsements
    }

    /// The decisive fact.
    #[must_use]
    pub const fn decision(&self) -> Option<&AnonymousFindingDecisionV1> {
        self.decision.as_ref()
    }

    /// Anchor of the conclusion.
    #[must_use]
    pub const fn concluded_at(&self) -> Option<GovernanceAnchorV1> {
        self.concluded_at
    }

    /// The seats endorsing `root`, in canonical order.
    fn endorsers(&self, root: [u8; 32]) -> Vec<AssignmentId> {
        self.endorsements
            .iter()
            .filter_map(|(assignment, endorsed)| (*endorsed == root).then_some(*assignment))
            .collect()
    }

    /// Whether no root can reach the quorum of `seats` any more: the strongest root plus
    /// every seat neither excluded nor endorsing stays below it.
    pub(super) fn quorum_unreachable(&self, seats: usize) -> bool {
        let quorum = u32::try_from(seats)
            .map(parliament_quorum_seats_v1)
            .map_or(usize::MAX, |quorum| {
                usize::try_from(quorum).unwrap_or(usize::MAX)
            });
        let remaining = seats
            .saturating_sub(self.excluded.len())
            .saturating_sub(self.endorsements.len());
        let mut counts = BTreeMap::<[u8; 32], usize>::new();
        for root in self.endorsements.values() {
            *counts.entry(*root).or_default() += 1;
        }
        let strongest = counts.values().copied().max().unwrap_or(0);
        strongest.saturating_add(remaining) < quorum
    }

    /// The decisive fact the record holds right after recording one absence or endorsement
    /// in a block of time `now_ms`, if it holds one.
    pub(super) fn decide(
        &self,
        seats: usize,
        endorsed: Option<[u8; 32]>,
        now_ms: u64,
    ) -> Option<AnonymousFindingDecisionV1> {
        let quorum = u32::try_from(seats).ok().map(parliament_quorum_seats_v1)?;
        if let Some(root) = endorsed {
            let witness = self.endorsers(root);
            if u32::try_from(witness.len()).ok() == Some(quorum) {
                return Some(AnonymousFindingDecisionV1 {
                    verdict: AnonymousFindingVerdictV1::Quorum {
                        result_root: root,
                        witness,
                    },
                    decided_ms: now_ms,
                });
            }
        }
        self.quorum_unreachable(seats)
            .then_some(AnonymousFindingDecisionV1 {
                verdict: AnonymousFindingVerdictV1::Unreachable,
                decided_ms: now_ms,
            })
    }
}

/// One sealed body: its seal, its phases and, for a public body, its finding record.
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
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousBodyStateV1")]
pub struct AnonymousBodyStateV1 {
    /// The immutable seal.
    pub(super) sealed: AnonymousSealedBodyV1,
    /// Anchor of the `CloseInvitations` item that sealed it.
    pub(super) sealed_at: GovernanceAnchorV1,
    /// The seated assignments in canonical order.
    pub(super) seats: Vec<ParliamentSeatAssignmentV1>,
    /// A binding jury's `(R, S, profile_id)`, bound into its roster root.
    pub(super) roster_binding: Option<AnonymousRosterBindingV1>,
    /// The phases entered, in order; the last is current.
    pub(super) phases: Vec<AnonymousPhaseV1>,
    /// A public body's finding record.
    pub(super) finding: Option<AnonymousFindingV1>,
}

impl AnonymousBodyStateV1 {
    /// A body sealed at `sealed_at`, before its first phase.
    pub(super) fn sealed(
        sealed: AnonymousSealedBodyV1,
        sealed_at: GovernanceAnchorV1,
        seats: Vec<ParliamentSeatAssignmentV1>,
        roster_binding: Option<AnonymousRosterBindingV1>,
    ) -> Self {
        let finding = (BodyDecisionModeV1::of(sealed.body) == BodyDecisionModeV1::PublicFinding)
            .then(AnonymousFindingV1::default);
        Self {
            sealed,
            sealed_at,
            seats,
            roster_binding,
            phases: Vec::new(),
            finding,
        }
    }

    /// The immutable seal.
    #[must_use]
    pub const fn sealed_body(&self) -> &AnonymousSealedBodyV1 {
        &self.sealed
    }

    /// Anchor of the sealing item.
    #[must_use]
    pub const fn sealed_at(&self) -> GovernanceAnchorV1 {
        self.sealed_at
    }

    /// The seated assignments in canonical order.
    #[must_use]
    pub fn seats(&self) -> &[ParliamentSeatAssignmentV1] {
        &self.seats
    }

    /// A binding jury's roster binding.
    #[must_use]
    pub const fn roster_binding(&self) -> Option<&AnonymousRosterBindingV1> {
        self.roster_binding.as_ref()
    }

    /// The phases entered.
    #[must_use]
    pub fn phases(&self) -> &[AnonymousPhaseV1] {
        &self.phases
    }

    /// The current phase.
    #[must_use]
    pub fn current_phase(&self) -> Option<&AnonymousPhaseV1> {
        self.phases.last()
    }

    /// A public body's finding record.
    #[must_use]
    pub const fn finding(&self) -> Option<&AnonymousFindingV1> {
        self.finding.as_ref()
    }

    /// The anchor at which the body entered `phase`.
    pub(super) fn entered(&self, phase: DeliberationPhaseV1) -> Option<GovernanceAnchorV1> {
        self.phases
            .iter()
            .find(|entry| entry.phase == phase)
            .map(|entry| entry.entered_at)
    }
}

/// The phase after `phase`; Reflection is the last phase of the anonymous family.
pub(super) const fn next_phase(phase: DeliberationPhaseV1) -> Option<DeliberationPhaseV1> {
    match phase {
        DeliberationPhaseV1::Orientation => Some(DeliberationPhaseV1::Evidence),
        DeliberationPhaseV1::Evidence => Some(DeliberationPhaseV1::Questions),
        DeliberationPhaseV1::Questions => Some(DeliberationPhaseV1::Responses),
        DeliberationPhaseV1::Responses => Some(DeliberationPhaseV1::Deliberation),
        DeliberationPhaseV1::Deliberation => Some(DeliberationPhaseV1::Reflection),
        DeliberationPhaseV1::Reflection | DeliberationPhaseV1::Vote => None,
    }
}

impl AnonymousParliamentAttemptStateV1 {
    /// The qualification record.
    #[must_use]
    pub const fn qualification(&self) -> &AnonymousQualificationV1 {
        &self.qualification
    }

    /// The State of the sealed `body`.
    #[must_use]
    pub fn body(&self, body: ParliamentBody) -> Option<&AnonymousBodyStateV1> {
        self.bodies.get(&body)
    }

    /// The sealed body with instance `body_instance_id`.
    fn body_by_instance(&self, body_instance_id: BodyInstanceId) -> Result<ParliamentBody, Error> {
        self.bodies
            .iter()
            .find_map(|(body, state)| {
                (state.sealed.body_instance_id == body_instance_id).then_some(*body)
            })
            .ok_or(Error::UnknownBodyInstance)
    }

    /// `StartQualification`'s deadline and the qualification deadline it starts.
    #[must_use]
    pub fn qualification_due_ms(&self) -> Option<u64> {
        let started = self.qualification.started?;
        started
            .timestamp_ms
            .checked_add(self.policy.lifecycle_policy.qualification_window_ms)
    }

    /// When `EndQualification` is due: the qualification deadline, or an earlier intent.
    pub(super) fn end_qualification_due_ms(&self) -> Result<u64, Error> {
        let deadline = self.qualification_due_ms().ok_or(Error::Schedule)?;
        Ok(self
            .qualification
            .early_close_ms
            .map_or(deadline, |early| early.min(deadline)))
    }

    /// G2a rank 0 `StartQualification` at `at`.
    ///
    /// # Errors
    /// [`Error`] for an inactive attempt, a qualification already started, an item in the
    /// creating block, or an anchor whose deadlines cannot be expressed.
    pub fn start_qualification(&mut self, at: GovernanceAnchorV1) -> Result<(), Error> {
        self.ensure_active(self.attempt.id)?;
        self.check_anchor(at)?;
        if self.qualification.started.is_some() {
            return Err(Error::QualificationStarted);
        }
        if at.timestamp_ms <= self.qualification.created_at_ms {
            return Err(Error::NotDue);
        }
        self.qualification.started = Some(at);
        Ok(())
    }

    /// G2a rank 0 `EndQualification` at `at`: the stage moves to the first required body.
    ///
    /// # Errors
    /// [`Error`] for an inactive attempt, an attempt past qualification or not started, an
    /// item before the qualification deadline and any intent, or an inexpressible deadline.
    pub fn end_qualification(&mut self, at: GovernanceAnchorV1) -> Result<(), Error> {
        self.ensure_active(self.attempt.id)?;
        self.check_anchor(at)?;
        self.ensure_stage(GovernanceStageV1::Qualification)?;
        let started = self.qualification.started.ok_or(Error::NotDue)?;
        let due = self.end_qualification_due_ms()?;
        if at.timestamp_ms < due || at.timestamp_ms <= started.timestamp_ms {
            return Err(Error::NotDue);
        }
        let first = self.first_stage()?;
        self.qualification.ended = Some(at);
        self.enter_stage(first, at);
        Ok(())
    }

    /// Record the `CloseQualificationEarly` intent in a block of time `now_ms`.
    ///
    /// # Errors
    /// [`Error`] for another or an inactive attempt, an attempt past qualification or not
    /// started, or an intent already recorded.
    pub fn close_qualification_early(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        now_ms: u64,
    ) -> Result<(), Error> {
        self.ensure_active(governance_attempt_id)?;
        self.ensure_stage(GovernanceStageV1::Qualification)?;
        let started = self.qualification.started.ok_or(Error::NotDue)?;
        if self.qualification.early_close_ms.is_some() {
            return Err(Error::EarlyCloseReplay);
        }
        if now_ms < started.timestamp_ms {
            return Err(Error::NotDue);
        }
        self.qualification.early_close_ms = Some(now_ms);
        Ok(())
    }

    /// Move the attempt to `stage` in the item anchored `at`; a sealed body that becomes
    /// current starts Orientation there.
    pub(super) fn enter_stage(&mut self, stage: GovernanceStageV1, at: GovernanceAnchorV1) {
        self.attempt.stage = stage;
        let current = self
            .policy
            .required_bodies
            .iter()
            .map(|required| required.body)
            .find(|body| stage_for_body(*body) == stage);
        if let Some(body) = current {
            self.start_orientation(body, at);
        }
    }

    /// Start Orientation of the sealed `body` at `at`, if it has not started.
    pub(super) fn start_orientation(&mut self, body: ParliamentBody, at: GovernanceAnchorV1) {
        if let Some(state) = self.bodies.get_mut(&body)
            && state.phases.is_empty()
        {
            state.phases.push(AnonymousPhaseV1 {
                phase: DeliberationPhaseV1::Orientation,
                entered_at: at,
                early_close_ms: None,
            });
        }
    }

    /// The longest offset any deadline of this attempt adds to an anchor: a lifecycle
    /// window, the enactment delay, or a binding jury's notice or casting window.
    fn longest_deadline_ms(&self) -> u64 {
        let lifecycle = &self.policy.lifecycle_policy;
        let mut longest = lifecycle
            .windows()
            .into_iter()
            .map(|(_, window)| window)
            .max()
            .unwrap_or(0)
            .max(lifecycle.enactment_delay_ms);
        if let Some(binding) = self.policy.binding {
            longest = longest
                .max(binding.vote_notice_ms)
                .max(binding.casting_window_ms);
        }
        longest
    }

    /// Whether every deadline a pass item anchored `at` may set can be expressed. Each pass
    /// item checks it before it changes anything.
    ///
    /// # Errors
    /// [`Error::Schedule`] when the anchor's time plus the longest deadline overflows.
    pub(super) fn check_anchor(&self, at: GovernanceAnchorV1) -> Result<(), Error> {
        at.timestamp_ms
            .checked_add(self.longest_deadline_ms())
            .map(|_| ())
            .ok_or(Error::Schedule)
    }

    /// The window of `phase`, a deliberation phase before Reflection.
    const fn phase_window_ms(&self, phase: DeliberationPhaseV1) -> Option<u64> {
        let lifecycle = &self.policy.lifecycle_policy;
        match phase {
            DeliberationPhaseV1::Orientation => Some(lifecycle.orientation_window_ms),
            DeliberationPhaseV1::Evidence => Some(lifecycle.evidence_window_ms),
            DeliberationPhaseV1::Questions => Some(lifecycle.questions_window_ms),
            DeliberationPhaseV1::Responses => Some(lifecycle.responses_window_ms),
            DeliberationPhaseV1::Deliberation => Some(lifecycle.deliberation_window_ms),
            DeliberationPhaseV1::Reflection | DeliberationPhaseV1::Vote => None,
        }
    }

    /// When `entry` ends: its deadline, or an earlier intent; `None` for Reflection, which
    /// has no deadline.
    ///
    /// # Errors
    /// [`Error::Schedule`] if the deadline cannot be expressed.
    pub(super) fn phase_due_ms(&self, entry: &AnonymousPhaseV1) -> Result<Option<u64>, Error> {
        let Some(window) = self.phase_window_ms(entry.phase) else {
            return Ok(None);
        };
        let deadline = entry
            .entered_at
            .timestamp_ms
            .checked_add(window)
            .ok_or(Error::Schedule)?;
        Ok(Some(
            entry
                .early_close_ms
                .map_or(deadline, |early| early.min(deadline)),
        ))
    }

    /// The body of the attempt's current stage, sealed.
    fn current_body(&self, body: ParliamentBody) -> Result<&AnonymousBodyStateV1, Error> {
        self.ensure_stage(stage_for_body(body))?;
        self.bodies.get(&body).ok_or(Error::BodyNotSealed { body })
    }

    /// G2a rank 0 `AdvancePhase` of the current `body` at `at`: the next phase, entered
    /// there. Entering Reflection opens a public body's finding window and fixes a binding
    /// jury's opening time.
    ///
    /// # Errors
    /// [`Error`] for an inactive attempt, a body that is not current or not deliberating,
    /// a body in Reflection, an item before the phase's deadline and any intent, or an
    /// inexpressible deadline.
    pub fn advance_phase(
        &mut self,
        body: ParliamentBody,
        at: GovernanceAnchorV1,
    ) -> Result<DeliberationPhaseV1, Error> {
        self.ensure_active(self.attempt.id)?;
        self.check_anchor(at)?;
        let state = self.current_body(body)?;
        let current = *state.current_phase().ok_or(Error::NotDue)?;
        let next = next_phase(current.phase).ok_or(Error::NotDue)?;
        let due = self.phase_due_ms(&current)?.ok_or(Error::NotDue)?;
        if at.timestamp_ms < due || at.timestamp_ms <= current.entered_at.timestamp_ms {
            return Err(Error::NotDue);
        }
        self.bodies
            .get_mut(&body)
            .ok_or(Error::BodyNotSealed { body })?
            .phases
            .push(AnonymousPhaseV1 {
                phase: next,
                entered_at: at,
                early_close_ms: None,
            });
        Ok(next)
    }

    /// Record the `ClosePhaseEarly` intent in a block of time `now_ms`.
    ///
    /// # Errors
    /// [`Error`] for another or an inactive attempt, an unknown body instance, a body that
    /// is not current, a phase that never closes early or is not the body's current
    /// phase, or an intent already recorded for it.
    pub fn close_phase_early(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        intent: AnonymousClosePhaseEarlyV1,
        now_ms: u64,
    ) -> Result<(), Error> {
        self.ensure_active(governance_attempt_id)?;
        if !AnonymousClosePhaseEarlyV1::closes_early(intent.phase) {
            return Err(Error::PhaseMismatch);
        }
        let body = self.body_by_instance(intent.body_instance_id)?;
        let state = self.current_body(body)?;
        let current = state.current_phase().ok_or(Error::PhaseMismatch)?;
        if current.phase != intent.phase {
            return Err(Error::PhaseMismatch);
        }
        if current.early_close_ms.is_some() {
            return Err(Error::EarlyCloseReplay);
        }
        if now_ms < current.entered_at.timestamp_ms {
            return Err(Error::NotDue);
        }
        self.bodies
            .get_mut(&body)
            .and_then(|state| state.phases.last_mut())
            .expect("the current phase exists")
            .early_close_ms = Some(now_ms);
        Ok(())
    }

    /// When a binding jury's casting may open: `max(t_r, sealed_at + vote_notice_ms)` once
    /// it entered Reflection at `t_r` (design §3.4).
    #[must_use]
    pub fn opening_at_ms(&self, body: ParliamentBody) -> Option<u64> {
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::AnonymousBallot {
            return None;
        }
        let state = self.bodies.get(&body)?;
        let reflection = state.entered(DeliberationPhaseV1::Reflection)?;
        let notice = state
            .sealed_at
            .timestamp_ms
            .checked_add(self.policy.binding?.vote_notice_ms)?;
        Some(reflection.timestamp_ms.max(notice))
    }

    /// When a public body's finding window closes, once it entered Reflection.
    #[must_use]
    pub fn finding_close_ms(&self, body: ParliamentBody) -> Option<u64> {
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::PublicFinding {
            return None;
        }
        self.bodies
            .get(&body)?
            .entered(DeliberationPhaseV1::Reflection)?
            .timestamp_ms
            .checked_add(self.policy.lifecycle_policy.public_finding_window_ms)
    }

    /// The current public body with instance `body_instance_id`, undecided.
    fn undecided_public_body(
        &self,
        body_instance_id: BodyInstanceId,
    ) -> Result<(ParliamentBody, &AnonymousBodyStateV1, &AnonymousFindingV1), Error> {
        let body = self.body_by_instance(body_instance_id)?;
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::PublicFinding {
            return Err(Error::DecisionMode { body });
        }
        let state = self.current_body(body)?;
        let finding = state.finding.as_ref().ok_or(Error::DecisionMode { body })?;
        if finding.decision.is_some() {
            return Err(Error::FindingDecided);
        }
        Ok((body, state, finding))
    }

    /// Record the authority `member`'s absence from its seat `assignment_id` of the current
    /// public body `body_instance_id`, in a block of time `now_ms`. The absence decides the
    /// body when no root can reach quorum any more.
    ///
    /// # Errors
    /// [`Error`] for another or an inactive attempt, an unknown, binding, not current or
    /// decided body, a body with an endorsement, a closed finding window, a seat the
    /// authority does not hold, or a repeated absence.
    pub fn record_absence(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        assignment_id: AssignmentId,
        member: &AccountId,
        now_ms: u64,
    ) -> Result<(), Error> {
        self.ensure_active(governance_attempt_id)?;
        let (body, state, finding) = self.undecided_public_body(body_instance_id)?;
        if !finding.endorsements.is_empty() {
            return Err(Error::AbsenceAfterEndorsement);
        }
        if self
            .finding_close_ms(body)
            .is_some_and(|close| now_ms >= close)
        {
            return Err(Error::FindingWindowClosed);
        }
        let seat = state
            .seats
            .iter()
            .find(|seat| seat.assignment_id == assignment_id)
            .ok_or(Error::UnauthorizedMember)?;
        if &seat.member != member {
            return Err(Error::UnauthorizedMember);
        }
        if finding.excluded.contains(&assignment_id) {
            return Err(Error::AbsenceReplay);
        }
        let seats = state.seats.len();
        let finding = self
            .bodies
            .get_mut(&body)
            .and_then(|state| state.finding.as_mut())
            .expect("the public body has a finding record");
        finding.excluded.insert(assignment_id);
        finding.decision = finding.decide(seats, None, now_ms);
        Ok(())
    }

    /// Record the authority `member`'s endorsement of `result_root` in the current public
    /// body `body_instance_id` in Reflection, in a block of time `now_ms`. The endorsement
    /// that brings one root to quorum decides the body with that root and its endorsing
    /// seats; one after which no root can reach quorum decides it as well.
    ///
    /// # Errors
    /// [`Error`] for another or an inactive attempt, an inert root, an unknown, binding,
    /// not current or decided body, a body outside Reflection or past its window, an
    /// authority without a seat, an excluded seat, or a repeated endorsement.
    pub fn endorse_public_finding(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        body_instance_id: BodyInstanceId,
        result_root: [u8; 32],
        member: &AccountId,
        now_ms: u64,
    ) -> Result<(), Error> {
        self.ensure_active(governance_attempt_id)?;
        if result_root == [0; 32] {
            return Err(Error::InertFindingRoot);
        }
        let (body, state, finding) = self.undecided_public_body(body_instance_id)?;
        let close = self.finding_close_ms(body).ok_or(Error::PhaseMismatch)?;
        if now_ms >= close {
            return Err(Error::FindingWindowClosed);
        }
        let assignment_id = state
            .seats
            .iter()
            .find(|seat| &seat.member == member)
            .map(|seat| seat.assignment_id)
            .ok_or(Error::UnauthorizedMember)?;
        if finding.excluded.contains(&assignment_id) {
            return Err(Error::SeatExcluded);
        }
        if finding.endorsements.contains_key(&assignment_id) {
            return Err(Error::EndorsementReplay);
        }
        let seats = state.seats.len();
        let finding = self
            .bodies
            .get_mut(&body)
            .and_then(|state| state.finding.as_mut())
            .expect("the public body has a finding record");
        finding.endorsements.insert(assignment_id, result_root);
        finding.decision = finding.decide(seats, Some(result_root), now_ms);
        Ok(())
    }

    /// When the current public `body`'s conclusion is due: at its decision, or at the end
    /// of its finding window once it entered Reflection; `None` otherwise.
    ///
    /// # Errors
    /// [`Error::Schedule`] if the end of the window cannot be expressed.
    pub(super) fn conclusion_due_ms(&self, body: ParliamentBody) -> Result<Option<u64>, Error> {
        if self.attempt.stage != stage_for_body(body) {
            return Ok(None);
        }
        let Some(state) = self.bodies.get(&body) else {
            return Ok(None);
        };
        let Some(finding) = state.finding.as_ref() else {
            return Ok(None);
        };
        if let Some(decision) = &finding.decision {
            return Ok(Some(decision.decided_ms));
        }
        let Some(reflection) = state.entered(DeliberationPhaseV1::Reflection) else {
            return Ok(None);
        };
        reflection
            .timestamp_ms
            .checked_add(self.policy.lifecycle_policy.public_finding_window_ms)
            .map(Some)
            .ok_or(Error::Schedule)
    }

    /// G2a rank 2 `ConcludeFinding` of the current public `body` at `at`: a quorum witness
    /// becomes the body's result and the stage advances; an unreachable quorum or an
    /// expired window rejects the attempt. The conclusion keeps its anchor.
    ///
    /// # Errors
    /// [`Error`] for an inactive attempt, a body that is not the current public one, an
    /// item before its decision's block or the window's end, or an inexpressible deadline.
    pub fn conclude_finding(
        &mut self,
        body: ParliamentBody,
        at: GovernanceAnchorV1,
    ) -> Result<(), Error> {
        self.ensure_active(self.attempt.id)?;
        self.check_anchor(at)?;
        if BodyDecisionModeV1::of(body) != BodyDecisionModeV1::PublicFinding {
            return Err(Error::DecisionMode { body });
        }
        let state = self.current_body(body)?;
        let finding = state.finding.as_ref().ok_or(Error::DecisionMode { body })?;
        // Decide from the record first: a quorum witness becomes the result, anything else
        // is the failure of the body.
        let conclusion = match &finding.decision {
            Some(decision) => {
                if at.timestamp_ms <= decision.decided_ms {
                    return Err(Error::NotDue);
                }
                match &decision.verdict {
                    AnonymousFindingVerdictV1::Quorum {
                        result_root,
                        witness,
                    } => Ok(self.public_finding_result(&state.sealed, *result_root, witness)?),
                    AnonymousFindingVerdictV1::Unreachable => {
                        Err(AnonymousPublicFindingFailureKindV1::QuorumUnreachable)
                    }
                }
            }
            None => {
                let close = self.finding_close_ms(body).ok_or(Error::NotDue)?;
                if at.timestamp_ms < close {
                    return Err(Error::NotDue);
                }
                Err(AnonymousPublicFindingFailureKindV1::WindowExpired)
            }
        };
        match conclusion {
            Ok(result) => self.record_public_finding(body, result, at)?,
            Err(kind) => {
                self.fail_public_finding(AnonymousPublicFindingFailureV1 { body, kind, at })?;
            }
        }
        if let Some(finding) = self
            .bodies
            .get_mut(&body)
            .and_then(|state| state.finding.as_mut())
        {
            finding.concluded_at = Some(at);
        }
        Ok(())
    }

    /// The certificate result of a quorum `witness` on `result_root` in `sealed`.
    pub(super) fn public_finding_result(
        &self,
        sealed: &AnonymousSealedBodyV1,
        result_root: [u8; 32],
        witness: &[AssignmentId],
    ) -> Result<AnonymousPublicFindingResultV1, Error> {
        let quorum = parliament_quorum_seats_v1(u32::from(sealed.seats));
        let endorsements = u32::try_from(witness.len()).map_err(|_| Error::FindingDecided)?;
        Ok(AnonymousPublicFindingResultV1 {
            result_root,
            finding: ParliamentPublicFindingCertificateBindingV1 {
                endorsement_root: parliament_public_finding_endorsement_root_v1(
                    self.attempt.id,
                    sealed.body_instance_id,
                    result_root,
                    witness,
                ),
                endorsing_assignments: witness.to_vec(),
                endorsements,
                quorum,
            },
        })
    }
}
