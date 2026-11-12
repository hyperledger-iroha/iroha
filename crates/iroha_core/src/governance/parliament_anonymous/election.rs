//! The election machine of the anonymous family (design §8.1, §9 and §10).
//!
//! A *generation* is the set of body elections one sortition registration creates
//! together: one fresh native pulse slot, one candidate snapshot and one election per body.
//! Registration is a transaction; the draw, the invitation close and every outcome run in
//! the positioned block-start pass:
//!
//! ```text
//! register (tx) -> AwaitingPulse -> DrawGeneration (G1) -> responses (tx)
//!     -> CloseInvitations (G2a rank 1) -> Sealed | NoRoster -> retry (tx) -> ...
//! register (tx) with a binding jury and fewer than three citizens -> capacity evidence
//! ```
//!
//! The first generation of a sequence-zero attempt is the baseline; every later generation,
//! capacity evidence included, consumes one unit of the proposal-wide redraw budget. A body
//! whose election ends without a roster at its last retry, or once that budget is spent,
//! rejects the attempt: at the draw or invitation close that found it, or through
//! `RejectExhausted` after capacity evidence.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            AnonymousGovernanceCertificateErrorV1, AnonymousParliamentRosterRootPreimageV1,
            AnonymousRosterBindingV1, AnonymousSealedBodyV1, MIN_SEALED_SEATS_V1, ProofProfileIdV1,
            anonymous_body_instance_id_v1, digests::anonymous_roster_root_v1,
        },
        types::{
            AssignmentId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId,
            GovernanceAttemptId, GovernanceAttemptStatusV1, ParliamentBody,
            ParliamentSeatAssignmentV1, SortitionRequestId, SortitionRequestV1,
            parliament_assignment_plan_root_v1, parliament_candidate_root_v1,
        },
    },
};
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

use super::{
    AnonymousAttemptRejectionV1, AnonymousBodyStateV1, AnonymousParliamentAttemptStateV1,
    AnonymousParliamentErrorV1 as Error, is_binding, stage_for_body,
};
use crate::governance::{
    draw::{ParliamentDrawSizingV1, derive_bounded_body_plan_v1},
    parliament::{aligned_sortition_pulse_height_v1, candidate_snapshot_fits_resource_bounds_v1},
};

/// Why a body's election ended without a roster.
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
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousNoRosterKindV1")]
pub enum AnonymousNoRosterKindV1 {
    /// The generation includes a binding jury and its snapshot held fewer than three
    /// citizens: capacity evidence, never drawn.
    #[codec(index = 0)]
    InsufficientElectorate,
    /// No allocation existed under the frozen cross-body cap.
    #[codec(index = 1)]
    AllocationInfeasible,
    /// No invited assignment accepted.
    #[codec(index = 2)]
    EmptyRoster,
    /// A binding jury accepted fewer than three seats.
    #[codec(index = 3)]
    InsufficientRoster,
}

/// How one drawn election's invitations closed.
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
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousElectionClosedV1")]
pub struct AnonymousElectionClosedV1 {
    /// Anchor of the `CloseInvitations` item.
    pub at: GovernanceAnchorV1,
    /// `None` when the roster was sealed; why there was none otherwise.
    pub no_roster: Option<AnonymousNoRosterKindV1>,
}

/// One body's election within a generation.
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
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousElectionV1")]
pub struct AnonymousElectionV1 {
    /// Zero-based sortition sequence of the body.
    pub(super) sequence: u32,
    /// The immutable request, committed before its pulse.
    pub(super) request: SortitionRequestV1,
    /// Primary invitations in rank order, once drawn.
    pub(super) primaries: Vec<ParliamentSeatAssignmentV1>,
    /// Alternate invitations in rank order, once drawn.
    pub(super) alternates: Vec<ParliamentSeatAssignmentV1>,
    /// Assignments that accepted their invitation.
    pub(super) accepted: BTreeSet<AssignmentId>,
    /// Assignments that declined their invitation.
    pub(super) declined: BTreeSet<AssignmentId>,
    /// How its invitations closed, once they did.
    pub(super) closed: Option<AnonymousElectionClosedV1>,
}

impl AnonymousElectionV1 {
    /// The body role.
    #[must_use]
    pub const fn body(&self) -> ParliamentBody {
        self.request.body
    }

    /// The election identifier.
    #[must_use]
    pub const fn id(&self) -> BodyElectionAttemptId {
        self.request.body_election_attempt_id
    }

    /// The zero-based sortition sequence.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// The immutable request.
    #[must_use]
    pub const fn request(&self) -> &SortitionRequestV1 {
        &self.request
    }

    /// Primary invitations in rank order.
    #[must_use]
    pub fn primaries(&self) -> &[ParliamentSeatAssignmentV1] {
        &self.primaries
    }

    /// Alternate invitations in rank order.
    #[must_use]
    pub fn alternates(&self) -> &[ParliamentSeatAssignmentV1] {
        &self.alternates
    }

    /// How its invitations closed.
    #[must_use]
    pub const fn closed(&self) -> Option<&AnonymousElectionClosedV1> {
        self.closed.as_ref()
    }

    /// The seats a close at this moment applies: accepted primaries in rank order, then
    /// accepted alternates in rank order, up to the request's target.
    pub(super) fn seated(&self) -> Vec<ParliamentSeatAssignmentV1> {
        let target = usize::try_from(self.request.target_seats).unwrap_or(usize::MAX);
        self.primaries
            .iter()
            .chain(&self.alternates)
            .filter(|seat| self.accepted.contains(&seat.assignment_id))
            .take(target)
            .cloned()
            .collect()
    }
}

/// The draw of a generation: the consumed pulse and the allocation it gave.
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
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousDrawV1")]
pub struct AnonymousDrawV1 {
    /// The consumed pulse.
    pub pulse_id: BeaconPulseId,
    /// The verified governance seed of that pulse.
    pub pulse_output: [u8; 32],
    /// The smallest cross-body cap under which the allocation existed; `None` when none
    /// existed under the frozen bound.
    pub assignment_cap: Option<u32>,
    /// Anchor of the `DrawGeneration` item.
    pub at: GovernanceAnchorV1,
}

/// Where a generation stands.
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
#[norito(tag = "status", content = "details", deny_unknown_fields)]
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousGenerationStatusV1")]
pub enum AnonymousGenerationStatusV1 {
    /// Waiting for the pulse of its exact slot, indefinitely.
    #[codec(index = 0)]
    AwaitingPulse,
    /// Capacity evidence, recorded by the registration in a block of this time; it awaits
    /// no pulse.
    #[codec(index = 1)]
    CapacityEvidence {
        /// Time of the registering block.
        recorded_at_ms: u64,
    },
    /// Its pulse was consumed.
    #[codec(index = 2)]
    Drawn(Box<AnonymousDrawV1>),
}

/// The elections one sortition registration created together.
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
#[norito_schema(name = "iroha_core::governance::parliament_anonymous::AnonymousGenerationV1")]
pub struct AnonymousGenerationV1 {
    /// Height of the block whose transaction registered it.
    pub(super) request_height: u64,
    /// The exact native pulse slot.
    pub(super) slot: u64,
    /// The canonical candidate snapshot every election of the generation shares.
    pub(super) candidates: Vec<AccountId>,
    /// One election per body, in pipeline order.
    pub(super) elections: Vec<AnonymousElectionV1>,
    /// Where it stands.
    pub(super) status: AnonymousGenerationStatusV1,
}

impl AnonymousGenerationV1 {
    /// Height of the registering block.
    #[must_use]
    pub const fn request_height(&self) -> u64 {
        self.request_height
    }

    /// The exact native pulse slot.
    #[must_use]
    pub const fn slot(&self) -> u64 {
        self.slot
    }

    /// The candidate snapshot.
    #[must_use]
    pub fn candidates(&self) -> &[AccountId] {
        &self.candidates
    }

    /// The elections, in pipeline order.
    #[must_use]
    pub fn elections(&self) -> &[AnonymousElectionV1] {
        &self.elections
    }

    /// Where it stands.
    #[must_use]
    pub const fn status(&self) -> &AnonymousGenerationStatusV1 {
        &self.status
    }

    /// The draw that allocated invitations, if one did.
    pub(super) fn feasible_draw(&self) -> Option<&AnonymousDrawV1> {
        match &self.status {
            AnonymousGenerationStatusV1::Drawn(draw) if draw.assignment_cap.is_some() => {
                Some(draw.as_ref())
            }
            _ => None,
        }
    }

    /// Why `election`, of this generation, ended without a roster, if it did.
    pub(super) fn no_roster(
        &self,
        election: &AnonymousElectionV1,
    ) -> Option<AnonymousNoRosterKindV1> {
        match &self.status {
            AnonymousGenerationStatusV1::CapacityEvidence { .. } => {
                Some(AnonymousNoRosterKindV1::InsufficientElectorate)
            }
            AnonymousGenerationStatusV1::Drawn(draw) if draw.assignment_cap.is_none() => {
                Some(AnonymousNoRosterKindV1::AllocationInfeasible)
            }
            _ => election.closed.and_then(|closed| closed.no_roster),
        }
    }
}

/// What a sortition registration recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnonymousRegistrationV1 {
    /// Requests awaiting the pulse of `slot`.
    Requested {
        /// The exact native pulse slot.
        slot: u64,
    },
    /// Capacity evidence: every body of the generation ended without a roster.
    /// `exhausted` when it left no retry, so `RejectExhausted` is due.
    CapacityEvidence {
        /// The slot the generation took.
        slot: u64,
        /// Whether it exhausted a budget.
        exhausted: bool,
    },
}

/// The authenticated pulse of a generation's exact slot, verified by Core against the key
/// session retained for that height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnonymousSortitionPulseV1 {
    /// The network's logical beacon session.
    pub beacon_session_id: BeaconSessionId,
    /// The pulse height: the slot.
    pub height: u64,
    /// The pulse identifier.
    pub pulse_id: BeaconPulseId,
    /// The verified persisted governance seed of the pulse.
    pub output: [u8; 32],
}

/// What a `DrawGeneration` item decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnonymousDrawOutcomeV1 {
    /// Invitations are open until the returned time.
    InvitationsOpen {
        /// `invitation_close_ms` of every body of the generation.
        close_ms: u64,
    },
    /// No allocation existed; every body ended without a roster and the attempt continues.
    AllocationInfeasible,
    /// No allocation existed and no retry is left: the attempt is rejected.
    Exhausted,
}

/// The seating a `CloseInvitations` item applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousSeatingV1 {
    /// The election.
    pub election_attempt_id: BodyElectionAttemptId,
    /// The body role.
    pub body: ParliamentBody,
    /// The seated assignments in rank order: accepted primaries, then accepted alternates.
    pub seats: Vec<ParliamentSeatAssignmentV1>,
    /// Why no roster can be sealed, if none can.
    pub no_roster: Option<AnonymousNoRosterKindV1>,
}

/// What a `CloseInvitations` item decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnonymousInvitationCloseV1 {
    /// The roster was sealed; [`AnonymousParliamentAttemptStateV1::sealed_body`] holds it.
    Sealed,
    /// No roster; a retry may redraw the body.
    NoRoster(AnonymousNoRosterKindV1),
    /// No roster and no retry left: the attempt is rejected.
    Exhausted(AnonymousNoRosterKindV1),
}

/// One election's ranked primary and alternate invitations.
pub(super) type ElectionInvitations = (
    Vec<ParliamentSeatAssignmentV1>,
    Vec<ParliamentSeatAssignmentV1>,
);

/// The frozen seat target and alternate cap of every body of an attempt's pipeline.
struct FrozenDrawSizing<'policy>(
    &'policy iroha_data_model::governance::anonymous_ballot::FrozenParliamentAttemptPolicyV1,
);

impl ParliamentDrawSizingV1 for FrozenDrawSizing<'_> {
    fn target_seats(&self, body: ParliamentBody) -> usize {
        self.0
            .required_body(body)
            .map_or(0, |required| usize::from(required.target_seats))
    }

    fn alternates(&self, body: ParliamentBody) -> usize {
        self.0
            .required_body(body)
            .map_or(0, |required| usize::from(required.alternate_cap))
    }
}

/// Check a candidate snapshot: strictly ordered and within the V1 bounds of 65,536
/// citizens and 8 MiB.
pub(super) fn check_snapshot(candidates: &[AccountId]) -> Result<(), Error> {
    if candidates.windows(2).all(|pair| pair[0] < pair[1])
        && candidate_snapshot_fits_resource_bounds_v1(candidates)
    {
        Ok(())
    } else {
        Err(Error::CandidateSnapshot)
    }
}

impl AnonymousParliamentAttemptStateV1 {
    /// Every generation, in registration order.
    #[must_use]
    pub fn generations(&self) -> &[AnonymousGenerationV1] {
        &self.generations
    }

    /// The latest election of `body`: its generation and position in it.
    pub(super) fn active_election(&self, body: ParliamentBody) -> Option<(usize, usize)> {
        self.generations
            .iter()
            .enumerate()
            .rev()
            .find_map(|(generation, entry)| {
                entry
                    .elections
                    .iter()
                    .position(|election| election.body() == body)
                    .map(|election| (generation, election))
            })
    }

    /// Every body whose latest election ended without a roster, with that election's
    /// sequence, in pipeline order.
    #[must_use]
    pub fn no_roster_bodies(&self) -> BTreeMap<ParliamentBody, u32> {
        self.policy
            .required_bodies
            .iter()
            .filter_map(|required| {
                let (generation, election) = self.active_election(required.body)?;
                let generation = &self.generations[generation];
                let election = &generation.elections[election];
                generation
                    .no_roster(election)
                    .map(|_| (required.body, election.sequence))
            })
            .collect()
    }

    /// The proposal-wide redraw units consumed through this attempt's transcript.
    ///
    /// The first generation of a sequence-zero attempt is the baseline; every other
    /// generation consumes one unit (design §8.1).
    ///
    /// TODO(E2, design §12.3 slice 2): the attempts table checks the lineage across one
    /// proposal's attempts (each successor starts from its predecessor's final count), as
    /// `validate_parliament_randomness_redraw_lineage_v1` does for the baseline.
    ///
    /// # Errors
    /// [`Error::RedrawBudgetExhausted`] if the count overflows.
    pub fn randomness_redraws_used(&self) -> Result<u32, Error> {
        let generations =
            u32::try_from(self.generations.len()).map_err(|_| Error::RedrawBudgetExhausted)?;
        let baseline = u32::from(self.attempt.sequence == 0 && generations > 0);
        self.randomness_redraws_before_attempt
            .checked_add(generations - baseline)
            .ok_or(Error::RedrawBudgetExhausted)
    }

    /// Whether the budget admits one more generation.
    fn redraw_available(&self) -> Result<bool, Error> {
        if self.attempt.sequence == 0 && self.generations.is_empty() {
            return Ok(true);
        }
        Ok(self.randomness_redraws_used()? < self.policy.sortition_policy.max_randomness_redraws)
    }

    /// Whether a body whose election of `sequence` ended without a roster leaves the
    /// attempt no retry: that was its last retry, or the redraw budget is spent.
    pub(super) fn exhausted_after(&self, sequence: u32) -> Result<bool, Error> {
        let policy = &self.policy.sortition_policy;
        Ok(sequence >= policy.max_sortition_retries
            || self.randomness_redraws_used()? >= policy.max_randomness_redraws)
    }

    /// The capacity evidence whose exhaustion `RejectExhausted` applies, with the time it is
    /// due: the latest generation, while the attempt is active.
    pub(super) fn pending_exhaustion(&self) -> Result<Option<u64>, Error> {
        if self.attempt.status != GovernanceAttemptStatusV1::Active {
            return Ok(None);
        }
        let Some(generation) = self.generations.last() else {
            return Ok(None);
        };
        let AnonymousGenerationStatusV1::CapacityEvidence { recorded_at_ms } = generation.status
        else {
            return Ok(None);
        };
        for election in &generation.elections {
            if self.exhausted_after(election.sequence)? {
                return Ok(Some(recorded_at_ms));
            }
        }
        Ok(None)
    }

    /// The exact slot of a generation registered at `request_height`: the first native
    /// pulse height at or above `max(request_height + d, g + 1)`, where `g` is the slot of
    /// the latest generation (design §9).
    fn next_slot(&self, request_height: u64) -> Option<u64> {
        let policy = &self.policy.sortition_policy;
        let delayed = request_height.checked_add(policy.pulse_delay_blocks)?;
        let earliest = match self.generations.last() {
            Some(previous) => delayed.max(previous.slot.checked_add(1)?),
            None => delayed,
        };
        aligned_sortition_pulse_height_v1(earliest, policy.pulse_cadence_blocks)
    }

    /// Register the initial generation (design §8.1): every required body but the
    /// Confirmation Jury at sequence zero, from the canonical eligible `candidates` at the
    /// containing `request_height`. It locks the risk tier.
    ///
    /// `now_ms` is the time of the containing block.
    ///
    /// # Errors
    /// [`Error`] for another or an inactive attempt, an attempt not at its first body's
    /// stage or already registered, an invalid snapshot, or an inexpressible slot.
    pub fn register_initial_sortition(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        network_id: &NetworkId,
        request_height: u64,
        now_ms: u64,
        candidates: Vec<AccountId>,
    ) -> Result<AnonymousRegistrationV1, Error> {
        self.ensure_active(governance_attempt_id)?;
        self.ensure_stage(self.first_stage()?)?;
        if !self.generations.is_empty() {
            return Err(Error::SortitionRegistered);
        }
        let bodies = self
            .policy
            .required_bodies
            .iter()
            .filter(|required| required.body != ParliamentBody::ConfirmationJury)
            .map(|required| (required.body, 0))
            .collect();
        self.register_generation(bodies, network_id, request_height, now_ms, candidates)
    }

    /// Register the next generation of every body whose latest election ended without a
    /// roster (design §8.1 and §10). A Confirmation Jury retry excludes every sealed Policy
    /// Jury member from `eligible`.
    ///
    /// # Errors
    /// [`Error`] for another or an inactive attempt, a pending exhaustion, no body to
    /// redraw, a height not above a retried generation's registration, a spent retry or
    /// redraw budget, an invalid snapshot, or an inexpressible slot.
    pub fn register_sortition_retry(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        network_id: &NetworkId,
        request_height: u64,
        now_ms: u64,
        eligible: Vec<AccountId>,
    ) -> Result<AnonymousRegistrationV1, Error> {
        self.ensure_active(governance_attempt_id)?;
        if self.pending_exhaustion()?.is_some() {
            return Err(Error::ExhaustionPending);
        }
        let failed = self.no_roster_bodies();
        if failed.is_empty() {
            return Err(Error::NoRetryDue);
        }
        for body in failed.keys() {
            let (generation, _) = self.active_election(*body).ok_or(Error::UnknownElection)?;
            if request_height <= self.generations[generation].request_height {
                return Err(Error::RetryTooEarly);
            }
        }
        let candidates = if failed.contains_key(&ParliamentBody::ConfirmationJury) {
            if failed.len() != 1 {
                return Err(Error::NoRetryDue);
            }
            self.exclude_policy_jury(eligible)?
        } else {
            eligible
        };
        let bodies = failed
            .into_iter()
            .map(|(body, sequence)| {
                sequence
                    .checked_add(1)
                    .map(|next| (body, next))
                    .ok_or(Error::SortitionRetryLimitExceeded)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.register_generation(bodies, network_id, request_height, now_ms, candidates)
    }

    /// `eligible` less every member of the sealed Policy Jury, in canonical order.
    fn exclude_policy_jury(&self, eligible: Vec<AccountId>) -> Result<Vec<AccountId>, Error> {
        let policy = self
            .bodies
            .get(&ParliamentBody::PolicyJury)
            .ok_or(Error::ConfirmationNotRequired)?;
        let members: BTreeSet<_> = policy.seats.iter().map(|seat| &seat.member).collect();
        Ok(eligible
            .into_iter()
            .filter(|candidate| !members.contains(candidate))
            .collect())
    }

    /// Register one generation of `bodies` with their sequences, atomically.
    fn register_generation(
        &mut self,
        bodies: Vec<(ParliamentBody, u32)>,
        network_id: &NetworkId,
        request_height: u64,
        now_ms: u64,
        candidates: Vec<AccountId>,
    ) -> Result<AnonymousRegistrationV1, Error> {
        if request_height == 0 {
            return Err(Error::Schedule);
        }
        check_snapshot(&candidates)?;
        let binding = bodies.iter().any(|(body, _)| is_binding(*body));
        let evidence = binding && candidates.len() < usize::from(MIN_SEALED_SEATS_V1);
        if candidates.is_empty() && !evidence {
            return Err(Error::CandidateSnapshot);
        }
        if !self.redraw_available()? {
            return Err(Error::RedrawBudgetExhausted);
        }
        let slot = self.next_slot(request_height).ok_or(Error::Schedule)?;
        let session = BeaconSessionId::for_network_v1(network_id);
        let candidate_count =
            u32::try_from(candidates.len()).map_err(|_| Error::CandidateSnapshot)?;
        let mut elections = Vec::with_capacity(bodies.len());
        for (body, sequence) in &bodies {
            if *sequence > self.policy.sortition_policy.max_sortition_retries {
                return Err(Error::SortitionRetryLimitExceeded);
            }
            let required = self
                .policy
                .required_body(*body)
                .ok_or(Error::UnknownBody { body: *body })?;
            let election_attempt_id =
                BodyElectionAttemptId::derive_v1(self.attempt.id, *body, *sequence);
            let mut request = SortitionRequestV1 {
                id: SortitionRequestId::new([0; 32]),
                governance_attempt_id: self.attempt.id,
                body_election_attempt_id: election_attempt_id,
                body: *body,
                candidate_root: parliament_candidate_root_v1(self.attempt.id, *body, &candidates),
                candidate_count,
                target_seats: u32::from(required.target_seats),
                request_height,
                pulse_height: slot,
                beacon_session_id: session,
            };
            request.id = request.canonical_id();
            if !evidence {
                request.validate(None).map_err(|_| Error::Schedule)?;
            }
            elections.push(AnonymousElectionV1 {
                sequence: *sequence,
                request,
                primaries: Vec::new(),
                alternates: Vec::new(),
                accepted: BTreeSet::new(),
                declined: BTreeSet::new(),
                closed: None,
            });
        }
        // The Policy Jury's first request locks the tier, and both ballot policies must exist
        // under it; check before anything changes.
        let locks = !self.risk_locked
            && bodies
                .iter()
                .any(|(body, _)| *body == ParliamentBody::PolicyJury);
        if locks {
            self.policy
                .binding_ballot_policies(self.attempt.risk_tier)?;
            self.risk_locked = true;
        }
        let status = if evidence {
            AnonymousGenerationStatusV1::CapacityEvidence {
                recorded_at_ms: now_ms,
            }
        } else {
            AnonymousGenerationStatusV1::AwaitingPulse
        };
        self.generations.push(AnonymousGenerationV1 {
            request_height,
            slot,
            candidates,
            elections,
            status,
        });
        Ok(if evidence {
            AnonymousRegistrationV1::CapacityEvidence {
                slot,
                exhausted: self.pending_exhaustion()?.is_some(),
            }
        } else {
            AnonymousRegistrationV1::Requested { slot }
        })
    }

    /// Register the Confirmation Jury's first generation atomically with the narrow Policy
    /// Jury approval closed at `closure_anchor` (design §10): its snapshot is `eligible`
    /// less every sealed Policy Jury member. Returns `false`, changing nothing, when fewer
    /// than three citizens remain or the redraw budget is spent: the terminal
    /// `ConfirmationJuryCapacityUnavailable`.
    pub(super) fn register_confirmation(
        &mut self,
        network_id: &NetworkId,
        closure_anchor: GovernanceAnchorV1,
        eligible: Vec<AccountId>,
    ) -> Result<bool, Error> {
        let candidates = self.exclude_policy_jury(eligible)?;
        if candidates.len() < usize::from(MIN_SEALED_SEATS_V1) || !self.redraw_available()? {
            return Ok(false);
        }
        let registration = self.register_generation(
            vec![(ParliamentBody::ConfirmationJury, 0)],
            network_id,
            closure_anchor.height,
            closure_anchor.timestamp_ms,
            candidates,
        )?;
        debug_assert!(matches!(
            registration,
            AnonymousRegistrationV1::Requested { .. }
        ));
        Ok(true)
    }

    /// Slots of the generations awaiting their pulse while the attempt is active, in
    /// increasing order: the `DrawGeneration` items G1 may process once each slot's pulse
    /// exists.
    #[must_use]
    pub fn awaiting_slots(&self) -> Vec<u64> {
        if self.attempt.status != GovernanceAttemptStatusV1::Active {
            return Vec::new();
        }
        self.generations
            .iter()
            .filter(|generation| generation.status == AnonymousGenerationStatusV1::AwaitingPulse)
            .map(|generation| generation.slot)
            .collect()
    }

    /// G1 `DrawGeneration` at `at`: consume `pulse` for the generation of its slot and draw
    /// every body of it under the frozen sizes and cross-body cap.
    ///
    /// # Errors
    /// [`Error`] for an inactive attempt, no generation awaiting that slot, a pulse of
    /// another session or an inert identifier, an item before the slot, or a plan that
    /// does not fit its requests.
    pub fn draw_generation(
        &mut self,
        network_id: &NetworkId,
        pulse: AnonymousSortitionPulseV1,
        at: GovernanceAnchorV1,
    ) -> Result<AnonymousDrawOutcomeV1, Error> {
        self.ensure_active(self.attempt.id)?;
        self.check_anchor(at)?;
        let close_ms = self.invitation_close_ms(at)?;
        let index = self
            .generations
            .iter()
            .position(|generation| {
                generation.slot == pulse.height
                    && generation.status == AnonymousGenerationStatusV1::AwaitingPulse
            })
            .ok_or(Error::UnknownGeneration)?;
        if pulse.beacon_session_id != BeaconSessionId::for_network_v1(network_id)
            || pulse.pulse_id.as_bytes() == &[0; 32]
            || at.height < pulse.height
        {
            return Err(Error::Pulse);
        }
        let generation = &self.generations[index];
        if generation
            .elections
            .iter()
            .any(|election| election.request.beacon_session_id != pulse.beacon_session_id)
        {
            return Err(Error::Pulse);
        }
        let assignments = self.derive_assignments(network_id, generation, &pulse.output)?;
        let draw = AnonymousDrawV1 {
            pulse_id: pulse.pulse_id,
            pulse_output: pulse.output,
            assignment_cap: assignments.as_ref().map(|(cap, _)| *cap),
            at,
        };
        let generation = &mut self.generations[index];
        generation.status = AnonymousGenerationStatusV1::Drawn(Box::new(draw));
        let Some((_, plans)) = assignments else {
            // Every body ended without a roster; the same item rejects an exhausted attempt.
            let sequences: Vec<_> = generation
                .elections
                .iter()
                .map(|election| (election.body(), election.sequence))
                .collect();
            for (body, sequence) in sequences {
                if self.exhausted_after(sequence)? {
                    self.reject_exhausted_at(
                        body,
                        AnonymousNoRosterKindV1::AllocationInfeasible,
                        at,
                    );
                    return Ok(AnonymousDrawOutcomeV1::Exhausted);
                }
            }
            return Ok(AnonymousDrawOutcomeV1::AllocationInfeasible);
        };
        for (election, (primaries, alternates)) in generation.elections.iter_mut().zip(plans) {
            election.primaries = primaries;
            election.alternates = alternates;
        }
        Ok(AnonymousDrawOutcomeV1::InvitationsOpen { close_ms })
    }

    /// The deterministic allocation of `generation` for a pulse of `output`: its cross-body
    /// cap and, per election in order, the ranked primary and alternate invitations; `None`
    /// when no allocation exists under the frozen bound.
    pub(super) fn derive_assignments(
        &self,
        network_id: &NetworkId,
        generation: &AnonymousGenerationV1,
        output: &[u8; 32],
    ) -> Result<Option<(u32, Vec<ElectionInvitations>)>, Error> {
        let bodies: Vec<_> = generation
            .elections
            .iter()
            .map(AnonymousElectionV1::body)
            .collect();
        let bound = u32::try_from(bodies.len())
            .unwrap_or(u32::MAX)
            .min(self.policy.sortition_policy.max_cross_body_assignment_cap);
        let Some(plan) = derive_bounded_body_plan_v1(
            &FrozenDrawSizing(&self.policy),
            network_id,
            generation.slot,
            output,
            &generation.candidates,
            &bodies,
            bound,
        ) else {
            return Ok(None);
        };
        let mut plans = Vec::with_capacity(generation.elections.len());
        for election in &generation.elections {
            let roster = plan
                .rosters
                .get(&election.body())
                .ok_or(Error::Allocation)?;
            let expected = election
                .request
                .target_seats
                .min(election.request.candidate_count);
            if roster.body != election.body()
                || roster.pulse_height != generation.slot
                || roster.candidate_count != election.request.candidate_count
                || roster.members.is_empty()
                || u32::try_from(roster.members.len()).ok() != Some(expected)
            {
                return Err(Error::Allocation);
            }
            let assign = |members: &[AccountId]| -> Vec<ParliamentSeatAssignmentV1> {
                members
                    .iter()
                    .map(|member| ParliamentSeatAssignmentV1 {
                        assignment_id: AssignmentId::derive_v1(election.id(), member),
                        member: member.clone(),
                    })
                    .collect()
            };
            let primaries = assign(&roster.members);
            let alternates = assign(&roster.alternates);
            let invited: BTreeSet<_> = primaries
                .iter()
                .chain(&alternates)
                .map(|seat| &seat.member)
                .collect();
            if invited.len() != primaries.len() + alternates.len() {
                return Err(Error::Allocation);
            }
            plans.push((primaries, alternates));
        }
        Ok(Some((plan.assignment_cap, plans)))
    }

    /// Reject the attempt because the election of `body` ended without a roster and no
    /// retry is left; the item anchored `at` applies it.
    pub(super) fn reject_exhausted_at(
        &mut self,
        body: ParliamentBody,
        kind: AnonymousNoRosterKindV1,
        at: GovernanceAnchorV1,
    ) {
        self.attempt.status = GovernanceAttemptStatusV1::Rejected;
        self.rejection = Some(AnonymousAttemptRejectionV1::SortitionExhausted(
            super::AnonymousSortitionExhaustionV1 { body, kind, at },
        ));
    }

    /// G2a rank 1 `RejectExhausted` at `at`: the capacity evidence of the latest generation
    /// left no retry.
    ///
    /// # Errors
    /// [`Error`] for an inactive attempt, no pending exhaustion, or an early item.
    pub fn reject_exhausted(&mut self, at: GovernanceAnchorV1) -> Result<(), Error> {
        self.ensure_active(self.attempt.id)?;
        self.check_anchor(at)?;
        let due_ms = self.pending_exhaustion()?.ok_or(Error::NotDue)?;
        let generation = self.generations.last().ok_or(Error::NotDue)?;
        if at.timestamp_ms <= due_ms || at.height <= generation.request_height {
            return Err(Error::NotDue);
        }
        let body = generation
            .elections
            .first()
            .map(AnonymousElectionV1::body)
            .ok_or(Error::NotDue)?;
        self.reject_exhausted_at(body, AnonymousNoRosterKindV1::InsufficientElectorate, at);
        Ok(())
    }

    /// When the invitations of a generation drawn at `drawn_at` close.
    ///
    /// # Errors
    /// [`Error::Schedule`] if the time cannot be expressed.
    pub(super) fn invitation_close_ms(&self, drawn_at: GovernanceAnchorV1) -> Result<u64, Error> {
        drawn_at
            .timestamp_ms
            .checked_add(self.policy.lifecycle_policy.invitation_window_ms)
            .ok_or(Error::Schedule)
    }

    /// The election `election_attempt_id` of `body`, accepting responses at `now_ms`.
    fn open_election(
        &self,
        election_attempt_id: BodyElectionAttemptId,
        body: ParliamentBody,
        now_ms: u64,
    ) -> Result<(usize, usize), Error> {
        let (generation_index, election_index) =
            self.active_election(body).ok_or(Error::UnknownElection)?;
        let generation = &self.generations[generation_index];
        let election = &generation.elections[election_index];
        if election.id() != election_attempt_id {
            return Err(Error::UnknownElection);
        }
        let draw = generation
            .feasible_draw()
            .ok_or(Error::InvitationsNotOpen)?;
        if election.closed.is_some() {
            return Err(Error::InvitationsNotOpen);
        }
        if now_ms >= self.invitation_close_ms(draw.at)? {
            return Err(Error::InvitationWindowClosed);
        }
        Ok((generation_index, election_index))
    }

    /// The assignment `member` holds in the open election `election_attempt_id` of `body`
    /// at `now_ms`, if it has not responded yet. The credential adapter checks a
    /// registration against it before it verifies the proof.
    ///
    /// # Errors
    /// [`Error`] for another or an inactive attempt, an election that is not the body's
    /// open one, a closed window, no invitation, or a response already recorded.
    pub fn invitation(
        &self,
        governance_attempt_id: GovernanceAttemptId,
        election_attempt_id: BodyElectionAttemptId,
        body: ParliamentBody,
        member: &AccountId,
        now_ms: u64,
    ) -> Result<AssignmentId, Error> {
        self.ensure_active(governance_attempt_id)?;
        let (generation, election) = self.open_election(election_attempt_id, body, now_ms)?;
        let election = &self.generations[generation].elections[election];
        let assignment_id = election
            .primaries
            .iter()
            .chain(&election.alternates)
            .find_map(|seat| (&seat.member == member).then_some(seat.assignment_id))
            .ok_or(Error::UnknownInvitation)?;
        if election.accepted.contains(&assignment_id) || election.declined.contains(&assignment_id)
        {
            return Err(Error::ResponseReplay);
        }
        Ok(assignment_id)
    }

    fn respond(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        election_attempt_id: BodyElectionAttemptId,
        body: ParliamentBody,
        member: &AccountId,
        accept: bool,
        now_ms: u64,
    ) -> Result<AssignmentId, Error> {
        let assignment_id = self.invitation(
            governance_attempt_id,
            election_attempt_id,
            body,
            member,
            now_ms,
        )?;
        let (generation, election) = self.active_election(body).ok_or(Error::UnknownElection)?;
        let election = &mut self.generations[generation].elections[election];
        if accept {
            election.accepted.insert(assignment_id);
        } else {
            election.declined.insert(assignment_id);
        }
        Ok(assignment_id)
    }

    /// Record the authority `member`'s response to its invitation in the open election
    /// `election_attempt_id` of `body`: a decline of any seat, or the acceptance of a
    /// public-body seat.
    ///
    /// # Errors
    /// As [`Self::invitation`], and [`Error::CredentialRequired`] for the acceptance of a
    /// binding seat, which only [`Self::accept_binding_seat`] records.
    pub fn record_invitation_response(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        election_attempt_id: BodyElectionAttemptId,
        body: ParliamentBody,
        member: &AccountId,
        accept: bool,
        now_ms: u64,
    ) -> Result<AssignmentId, Error> {
        if accept && is_binding(body) {
            return Err(Error::CredentialRequired);
        }
        self.respond(
            governance_attempt_id,
            election_attempt_id,
            body,
            member,
            accept,
            now_ms,
        )
    }

    /// Record the acceptance of a binding seat. Only the credential adapter calls it, in the
    /// same atomic step that records the verified credential of that assignment (design
    /// §3.2).
    ///
    /// # Errors
    /// As [`Self::invitation`], and [`Error::DecisionMode`] for a public body.
    pub(super) fn accept_binding_seat(
        &mut self,
        governance_attempt_id: GovernanceAttemptId,
        election_attempt_id: BodyElectionAttemptId,
        body: ParliamentBody,
        member: &AccountId,
        now_ms: u64,
    ) -> Result<AssignmentId, Error> {
        if !is_binding(body) {
            return Err(Error::DecisionMode { body });
        }
        self.respond(
            governance_attempt_id,
            election_attempt_id,
            body,
            member,
            true,
            now_ms,
        )
    }

    /// The seating `CloseInvitations` of `body` applies at `now_ms`. A binding jury's
    /// credential adapter seals exactly these seats before the close (design §3.3).
    ///
    /// # Errors
    /// [`Error`] for an inactive attempt, a body without an open drawn election, or an
    /// item before the invitation window closed.
    pub fn seating(&self, body: ParliamentBody, now_ms: u64) -> Result<AnonymousSeatingV1, Error> {
        self.ensure_active(self.attempt.id)?;
        let (generation, election) = self.active_election(body).ok_or(Error::UnknownElection)?;
        let generation = &self.generations[generation];
        let election = &generation.elections[election];
        let draw = generation
            .feasible_draw()
            .ok_or(Error::InvitationsNotOpen)?;
        if election.closed.is_some() {
            return Err(Error::InvitationsNotOpen);
        }
        if now_ms < self.invitation_close_ms(draw.at)? {
            return Err(Error::NotDue);
        }
        let seats = election.seated();
        let no_roster = if seats.is_empty() {
            Some(AnonymousNoRosterKindV1::EmptyRoster)
        } else if is_binding(body) && seats.len() < usize::from(MIN_SEALED_SEATS_V1) {
            Some(AnonymousNoRosterKindV1::InsufficientRoster)
        } else {
            None
        };
        Ok(AnonymousSeatingV1 {
            election_attempt_id: election.id(),
            body,
            seats,
            no_roster,
        })
    }

    /// G2a rank 1 `CloseInvitations` of `body` at `at`.
    ///
    /// A binding jury's roster is sealed with `binding`, the `(R, S, profile_id)` its
    /// credential adapter sealed for exactly the seats of [`Self::seating`]; a public body
    /// takes none. Without a viable roster the body ends `NoRoster`, and the same item
    /// rejects an attempt left without a retry.
    ///
    /// # Errors
    /// As [`Self::seating`]; [`Error::RosterBinding`] for a binding that is missing,
    /// unexpected or not of the seated roster; and the rules of a sealed body
    /// ([`AnonymousSealedBodyV1::validate`], the anonymity floor, the risk lock and the
    /// Confirmation Jury's freshness).
    pub(super) fn close_invitations(
        &mut self,
        body: ParliamentBody,
        at: GovernanceAnchorV1,
        binding: Option<AnonymousRosterBindingV1>,
    ) -> Result<AnonymousInvitationCloseV1, Error> {
        self.check_anchor(at)?;
        let seating = self.seating(body, at.timestamp_ms)?;
        let (generation_index, election_index) =
            self.active_election(body).ok_or(Error::UnknownElection)?;
        if let Some(kind) = seating.no_roster {
            if binding.is_some() {
                return Err(Error::RosterBinding);
            }
            let election = &mut self.generations[generation_index].elections[election_index];
            election.closed = Some(AnonymousElectionClosedV1 {
                at,
                no_roster: Some(kind),
            });
            let sequence = election.sequence;
            if self.exhausted_after(sequence)? {
                self.reject_exhausted_at(body, kind, at);
                return Ok(AnonymousInvitationCloseV1::Exhausted(kind));
            }
            return Ok(AnonymousInvitationCloseV1::NoRoster(kind));
        }

        let state =
            self.sealed_body_state(generation_index, election_index, &seating, binding, at)?;
        self.check_seal(&state.sealed)?;
        if body == ParliamentBody::ConfirmationJury {
            let policy = self
                .bodies
                .get(&ParliamentBody::PolicyJury)
                .ok_or(Error::ConfirmationNotRequired)?;
            let members: BTreeSet<_> = policy.seats.iter().map(|seat| &seat.member).collect();
            if state
                .seats
                .iter()
                .any(|seat| members.contains(&seat.member))
            {
                return Err(Error::Certificate(
                    AnonymousGovernanceCertificateErrorV1::ConfirmationJuryMismatch,
                ));
            }
        }
        self.generations[generation_index].elections[election_index].closed =
            Some(AnonymousElectionClosedV1 {
                at,
                no_roster: None,
            });
        self.bodies.insert(body, state);
        if self.attempt.stage == stage_for_body(body) {
            self.start_orientation(body, at);
        }
        Ok(AnonymousInvitationCloseV1::Sealed)
    }

    /// The sealed State of the election at `(generation_index, election_index)` for
    /// `seating`, sealed at `at` with `binding`.
    pub(super) fn sealed_body_state(
        &self,
        generation_index: usize,
        election_index: usize,
        seating: &AnonymousSeatingV1,
        binding: Option<AnonymousRosterBindingV1>,
        at: GovernanceAnchorV1,
    ) -> Result<AnonymousBodyStateV1, Error> {
        let generation = &self.generations[generation_index];
        let election = &generation.elections[election_index];
        let draw = generation
            .feasible_draw()
            .ok_or(Error::InvitationsNotOpen)?;
        let cap = draw.assignment_cap.ok_or(Error::InvitationsNotOpen)?;
        let body = election.body();
        let seats = u16::try_from(seating.seats.len()).map_err(|_| Error::RosterBinding)?;
        match (is_binding(body), binding) {
            (true, Some(binding))
                if binding.seats == seats && binding.profile_id == ProofProfileIdV1::V1 => {}
            (false, None) => {}
            _ => return Err(Error::RosterBinding),
        }
        let mut assignments = seating.seats.clone();
        assignments.sort_unstable_by(|left, right| {
            left.assignment_id
                .as_bytes()
                .cmp(right.assignment_id.as_bytes())
        });
        let roster_root = anonymous_roster_root_v1(&AnonymousParliamentRosterRootPreimageV1 {
            election_attempt_id: election.id(),
            assignments: assignments.clone(),
            anonymous_ballot: binding,
        })
        .map_err(|_| Error::RosterBinding)?;
        let sealed = AnonymousSealedBodyV1 {
            body_instance_id: anonymous_body_instance_id_v1(election.id(), &roster_root),
            election_attempt_id: election.id(),
            election_attempt_sequence: election.sequence,
            sortition_request: election.request,
            body,
            seats,
            beacon_pulse_id: draw.pulse_id,
            roster_root,
            assignment_root: parliament_assignment_plan_root_v1(
                election.id(),
                &election.primaries,
                &election.alternates,
                cap,
            ),
        };
        Ok(AnonymousBodyStateV1::sealed(
            sealed,
            at,
            assignments,
            binding,
        ))
    }
}
