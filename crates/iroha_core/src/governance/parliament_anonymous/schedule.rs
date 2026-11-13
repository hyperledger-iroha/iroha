//! The positioned block-start items of an anonymous attempt (design §8.1): their kinds,
//! occurrences, order keys, costs and due times.
//!
//! The attempt reports its own live occurrences; the positioned pass (`specs/sccp.md`
//! §4.7.2) merges every attempt's items with the other owners' items, orders them by
//! `(due_ms, kind_rank, item key)` within G2a, charges each its fixed cost, and applies the
//! ones it reaches with [`AnonymousParliamentAttemptStateV1::apply_lifecycle_item`]. An item
//! names the occurrence that made it due, and applying it after that occurrence passed is
//! refused. G1 draws are reported separately
//! ([`AnonymousParliamentAttemptStateV1::awaiting_slots`]) because they wait for a pulse,
//! not for a time.
//!
//! The positioned pass (`world_pass::run_governance_pass`) collects these items with the
//! ballot openings and closures of the boards and the casting-clock prelude, and drives them
//! under the shared budget. Production enactments join G3 with slice 5.

use core::cmp::Ordering;

use iroha_data_model::governance::{
    GovernanceAnchorV1,
    anonymous_ballot::{AnonymousRosterBindingV1, BodyDecisionModeV1},
    types::{
        BodyElectionAttemptId, BodyInstanceId, DeliberationPhaseV1, GovernanceAttemptId,
        GovernanceAttemptStatusV1, ParliamentBody,
    },
};

use super::{
    AnonymousInvitationCloseV1, AnonymousParliamentAttemptStateV1,
    AnonymousParliamentErrorV1 as Error, election::AnonymousSeatingV1, stage_for_body,
};

/// Cost of a `DrawGeneration` item: the largest Parliament item.
pub const DRAW_GENERATION_COST_V1: u32 = 32;
/// Cost of a `CloseInvitations` item, which may seal and publish a credential list.
pub const CLOSE_INVITATIONS_COST_V1: u32 = 8;
/// Cost of every other lifecycle item of ranks 0 to 2.
pub const LIFECYCLE_STEP_COST_V1: u32 = 1;
/// Cost of a `CloseBallot` item, which may register the Confirmation Jury's generation.
pub const CLOSE_BALLOT_COST_V1: u32 = 16;
/// Cost of an `OpenBallot` item.
pub const OPEN_BALLOT_COST_V1: u32 = 2;

/// One lifecycle item of G2a ranks 0 to 2 of an anonymous attempt, with the occurrence it
/// applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnonymousLifecycleItemV1 {
    /// Rank 0: anchor the qualification window.
    StartQualification,
    /// Rank 0: end qualification and enter the first required body.
    EndQualification,
    /// Rank 0: end `phase` of the current `body` and enter the next phase.
    AdvancePhase {
        /// The current body.
        body: ParliamentBody,
        /// The phase it ends.
        phase: DeliberationPhaseV1,
    },
    /// Rank 1: reject an attempt whose capacity evidence of `slot` left no retry.
    RejectExhausted {
        /// The slot of the exhausting evidence.
        slot: u64,
    },
    /// Rank 1: seal `body`'s drawn `election`, or end it without a roster.
    CloseInvitations {
        /// The body.
        body: ParliamentBody,
        /// Its drawn election.
        election: BodyElectionAttemptId,
    },
    /// Rank 2: conclude the current public `body`'s sealed `instance`.
    ConcludeFinding {
        /// The body.
        body: ParliamentBody,
        /// Its sealed instance.
        instance: BodyInstanceId,
    },
}

impl AnonymousLifecycleItemV1 {
    /// The G2a rank: 0 for qualification and phases, 1 for elections, 2 for findings.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::StartQualification | Self::EndQualification | Self::AdvancePhase { .. } => 0,
            Self::RejectExhausted { .. } | Self::CloseInvitations { .. } => 1,
            Self::ConcludeFinding { .. } => 2,
        }
    }

    /// The kind value within the rank: attempt-level kinds precede body kinds.
    #[must_use]
    pub const fn kind(self) -> u8 {
        match self {
            Self::StartQualification
            | Self::RejectExhausted { .. }
            | Self::ConcludeFinding { .. } => 0,
            Self::EndQualification | Self::CloseInvitations { .. } => 1,
            Self::AdvancePhase { .. } => 2,
        }
    }

    /// The body of a body item.
    #[must_use]
    pub const fn body(self) -> Option<ParliamentBody> {
        match self {
            Self::AdvancePhase { body, .. }
            | Self::CloseInvitations { body, .. }
            | Self::ConcludeFinding { body, .. } => Some(body),
            Self::StartQualification | Self::EndQualification | Self::RejectExhausted { .. } => {
                None
            }
        }
    }

    /// The stable name of the item's kind.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::StartQualification => "StartQualification",
            Self::EndQualification => "EndQualification",
            Self::AdvancePhase { .. } => "AdvancePhase",
            Self::RejectExhausted { .. } => "RejectExhausted",
            Self::CloseInvitations { .. } => "CloseInvitations",
            Self::ConcludeFinding { .. } => "ConcludeFinding",
        }
    }

    /// The fixed cost.
    #[must_use]
    pub const fn cost(self) -> u32 {
        match self {
            Self::CloseInvitations { .. } => CLOSE_INVITATIONS_COST_V1,
            _ => LIFECYCLE_STEP_COST_V1,
        }
    }
}

/// A live lifecycle occurrence of one attempt and when it is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnonymousDueItemV1 {
    /// When the item is due on the governance clock.
    pub due_ms: u64,
    /// The attempt.
    pub governance_attempt_id: GovernanceAttemptId,
    /// The item.
    pub item: AnonymousLifecycleItemV1,
}

impl AnonymousDueItemV1 {
    /// The G2a order key `(due_ms, kind_rank, governance_attempt_id, kind, body)`, with an
    /// attempt-level item before the body items of its attempt. One occurrence is live per
    /// key, so the key orders every live item totally.
    fn key(&self) -> (u64, u8, GovernanceAttemptId, u8, Option<ParliamentBody>) {
        (
            self.due_ms,
            self.item.rank(),
            self.governance_attempt_id,
            self.item.kind(),
            self.item.body(),
        )
    }
}

impl PartialOrd for AnonymousDueItemV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AnonymousDueItemV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl AnonymousParliamentAttemptStateV1 {
    /// Every live lifecycle occurrence of ranks 0 to 2 with its due time, due or not, in
    /// G2a order. A terminal attempt has none.
    ///
    /// # Errors
    /// [`Error::Schedule`] if a deadline cannot be expressed, or the error of an
    /// inconsistent redraw count.
    pub fn lifecycle_items(&self) -> Result<Vec<AnonymousDueItemV1>, Error> {
        if self.attempt.status != GovernanceAttemptStatusV1::Active {
            return Ok(Vec::new());
        }
        let id = self.attempt.id;
        let mut items = Vec::new();
        let mut push = |due_ms, item| {
            items.push(AnonymousDueItemV1 {
                due_ms,
                governance_attempt_id: id,
                item,
            });
        };

        let qualification = &self.qualification;
        if qualification.started.is_none() {
            push(
                qualification.created_at_ms,
                AnonymousLifecycleItemV1::StartQualification,
            );
        } else if qualification.ended.is_none() {
            push(
                self.end_qualification_due_ms()?,
                AnonymousLifecycleItemV1::EndQualification,
            );
        }

        if let Some(due_ms) = self.pending_exhaustion()?
            && let Some(generation) = self.generations.last()
        {
            push(
                due_ms,
                AnonymousLifecycleItemV1::RejectExhausted {
                    slot: generation.slot,
                },
            );
        }

        for required in &self.policy.required_bodies {
            let body = required.body;
            if let Some((generation, election)) = self.active_election(body) {
                let generation = &self.generations[generation];
                let election = &generation.elections[election];
                if let Some(draw) = generation.feasible_draw()
                    && election.closed.is_none()
                {
                    push(
                        self.invitation_close_ms(draw.at)?,
                        AnonymousLifecycleItemV1::CloseInvitations {
                            body,
                            election: election.id(),
                        },
                    );
                }
            }
            if self.attempt.stage != stage_for_body(body) {
                continue;
            }
            let Some(state) = self.bodies.get(&body) else {
                continue;
            };
            if let Some(phase) = state.current_phase()
                && let Some(due_ms) = self.phase_due_ms(phase)?
            {
                push(
                    due_ms,
                    AnonymousLifecycleItemV1::AdvancePhase {
                        body,
                        phase: phase.phase,
                    },
                );
            }
            if let Some(due_ms) = self.conclusion_due_ms(body)? {
                push(
                    due_ms,
                    AnonymousLifecycleItemV1::ConcludeFinding {
                        body,
                        instance: state.sealed.body_instance_id,
                    },
                );
            }
        }
        items.sort_unstable();
        Ok(items)
    }

    /// The lifecycle items due at `now_ms`, in G2a order.
    ///
    /// # Errors
    /// As [`Self::lifecycle_items`].
    pub fn due_lifecycle_items(&self, now_ms: u64) -> Result<Vec<AnonymousDueItemV1>, Error> {
        let mut items = self.lifecycle_items()?;
        items.retain(|item| item.due_ms <= now_ms);
        Ok(items)
    }

    /// Whether `item` still names a live occurrence of this attempt.
    pub(super) fn check_occurrence(&self, item: AnonymousLifecycleItemV1) -> Result<(), Error> {
        let live = match item {
            AnonymousLifecycleItemV1::StartQualification
            | AnonymousLifecycleItemV1::EndQualification => true,
            AnonymousLifecycleItemV1::AdvancePhase { body, phase } => self
                .bodies
                .get(&body)
                .and_then(|state| state.current_phase())
                .is_some_and(|current| current.phase == phase),
            AnonymousLifecycleItemV1::RejectExhausted { slot } => self
                .generations
                .last()
                .is_some_and(|generation| generation.slot == slot),
            AnonymousLifecycleItemV1::CloseInvitations { body, election } => self
                .active_election(body)
                .is_some_and(|(generation, index)| {
                    self.generations[generation].elections[index].id() == election
                }),
            AnonymousLifecycleItemV1::ConcludeFinding { body, instance } => self
                .bodies
                .get(&body)
                .is_some_and(|state| state.sealed.body_instance_id == instance),
        };
        if live { Ok(()) } else { Err(Error::StaleItem) }
    }

    /// Apply the lifecycle `item` at `at`; returns what a `CloseInvitations` item decided.
    /// A binding jury's `CloseInvitations` asks `seal` for the `(R, S, profile_id)` of
    /// exactly the seats it is given; every other item never calls it, and no item calls it
    /// before its occurrence and anchor are checked. Execution seals through the credential
    /// adapter ([`Self::apply_item_with_credentials`]).
    ///
    /// # Errors
    /// [`Error::StaleItem`] for an item whose occurrence passed, [`Error::Schedule`] for an
    /// anchor whose deadlines cannot be expressed, and otherwise the error of the item's
    /// own method or of `seal`.
    pub(super) fn apply_lifecycle_item<F>(
        &mut self,
        item: AnonymousLifecycleItemV1,
        at: GovernanceAnchorV1,
        seal: F,
    ) -> Result<Option<AnonymousInvitationCloseV1>, Error>
    where
        F: FnOnce(&AnonymousSeatingV1) -> Result<AnonymousRosterBindingV1, Error>,
    {
        self.check_occurrence(item)?;
        self.check_anchor(at)?;
        match item {
            AnonymousLifecycleItemV1::StartQualification => {
                self.start_qualification(at).map(|()| None)
            }
            AnonymousLifecycleItemV1::EndQualification => self.end_qualification(at).map(|()| None),
            AnonymousLifecycleItemV1::AdvancePhase { body, .. } => {
                self.advance_phase(body, at).map(|_| None)
            }
            AnonymousLifecycleItemV1::RejectExhausted { .. } => {
                self.reject_exhausted(at).map(|()| None)
            }
            AnonymousLifecycleItemV1::CloseInvitations { body, .. } => {
                let seating = self.seating(body, at.timestamp_ms)?;
                let binding = if seating.no_roster.is_none()
                    && BodyDecisionModeV1::of(body) == BodyDecisionModeV1::AnonymousBallot
                {
                    Some(seal(&seating)?)
                } else {
                    None
                };
                self.close_invitations(body, at, binding).map(Some)
            }
            AnonymousLifecycleItemV1::ConcludeFinding { body, .. } => {
                self.conclude_finding(body, at).map(|()| None)
            }
        }
    }
}
