//! Restore validation of an anonymous attempt's lifecycle (design §8.1, "Restore").
//!
//! A stored attempt is recomputed, never trusted: every slot, request, draw, seal, phase
//! and decision must be the one the reducer's own transitions produce from the recorded
//! anchors, intents and facts and the frozen policy.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    NetworkId,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            AnonymousBodyResultV1, AnonymousClosePhaseEarlyV1, BallotClosureRecordV1,
            MIN_SEALED_SEATS_V1,
        },
        types::{
            AssignmentId, BeaconSessionId, BodyElectionAttemptId, DeliberationPhaseV1,
            GovernanceAttemptStatusV1, GovernanceStageV1, ParliamentBody,
            parliament_candidate_root_v1, parliament_quorum_seats_v1,
        },
    },
};

use super::{
    AnonymousAttemptRejectionV1, AnonymousBodyStateV1, AnonymousParliamentAttemptStateV1,
    AnonymousParliamentErrorV1 as Error, AnonymousPublicFindingFailureKindV1,
    AnonymousSortitionExhaustionV1,
    deliberation::{AnonymousFindingV1, AnonymousFindingVerdictV1, next_phase},
    election::{
        AnonymousElectionClosedV1, AnonymousElectionV1, AnonymousGenerationStatusV1,
        AnonymousGenerationV1, AnonymousNoRosterKindV1, AnonymousSeatingV1, check_snapshot,
    },
    is_binding, stage_for_body,
};
use crate::governance::parliament::aligned_sortition_pulse_height_v1;

const fn restore(field: &'static str) -> Error {
    Error::Restore { field }
}

/// Whether the anchor `later` comes after `earlier` in pass order.
fn follows(later: GovernanceAnchorV1, earlier: GovernanceAnchorV1) -> bool {
    (later.height, later.position) > (earlier.height, earlier.position)
}

/// The later of two anchors in pass order.
fn later_of(left: GovernanceAnchorV1, right: GovernanceAnchorV1) -> GovernanceAnchorV1 {
    if follows(right, left) { right } else { left }
}

/// The height of the item or transaction at which `election` of `generation` ended
/// without a roster, if it did.
fn no_roster_height(
    generation: &AnonymousGenerationV1,
    election: &AnonymousElectionV1,
) -> Option<u64> {
    match &generation.status {
        AnonymousGenerationStatusV1::CapacityEvidence { .. } => Some(generation.request_height),
        AnonymousGenerationStatusV1::Drawn(draw) if draw.assignment_cap.is_none() => {
            Some(draw.at.height)
        }
        _ => election
            .closed
            .filter(|closed| closed.no_roster.is_some())
            .map(|closed| closed.at.height),
    }
}

impl AnonymousParliamentAttemptStateV1 {
    /// Check the lifecycle records against each other and the frozen policy.
    pub(super) fn validate_lifecycle(&self, network_id: &NetworkId) -> Result<(), Error> {
        self.validate_restored_anchors()?;
        self.validate_restored_qualification()?;
        self.validate_restored_generations(network_id)?;
        self.validate_restored_phases()?;
        self.validate_restored_findings()?;
        if self.randomness_redraws_used()? > self.policy.sortition_policy.max_randomness_redraws {
            return Err(restore("generations"));
        }
        // The initial generation always holds the Policy Jury, so it locked the tier.
        if self.risk_locked == self.generations.is_empty() {
            return Err(restore("risk_locked"));
        }
        // An active attempt is exhausted only while `RejectExhausted` awaits its pass.
        if self.attempt.status == GovernanceAttemptStatusV1::Active
            && self.pending_exhaustion()?.is_none()
        {
            for sequence in self.no_roster_bodies().into_values() {
                if self.exhausted_after(sequence)? {
                    return Err(restore("generations"));
                }
            }
        }
        Ok(())
    }

    /// Every anchor the reducer checked when it applied admits the deadlines it set, and
    /// every recorded block time lies on one chain: positions at one height share that
    /// block's time, and a later height never has an earlier one (a block's time is at
    /// least its parent's plus the cadence, `specs/sumeragi.md` "Block time").
    fn validate_restored_anchors(&self) -> Result<(), Error> {
        let qualification = &self.qualification;
        // The anchors of the pass items, closures and rejections, each checked when applied.
        let mut checked: Vec<GovernanceAnchorV1> = qualification
            .started
            .into_iter()
            .chain(qualification.ended)
            .collect();
        for generation in &self.generations {
            if let AnonymousGenerationStatusV1::Drawn(draw) = &generation.status {
                checked.push(draw.at);
            }
            checked.extend(
                generation
                    .elections
                    .iter()
                    .filter_map(|election| election.closed.map(|closed| closed.at)),
            );
        }
        for state in self.bodies.values() {
            checked.push(state.sealed_at);
            checked.extend(state.phases.iter().map(|phase| phase.entered_at));
            checked.extend(
                state
                    .finding
                    .as_ref()
                    .and_then(|finding| finding.concluded_at),
            );
        }
        let mut closures: Vec<&BallotClosureRecordV1> = self
            .results
            .values()
            .filter_map(|binding| binding.ballot().map(|ballot| &ballot.closure))
            .collect();
        match &self.rejection {
            Some(
                AnonymousAttemptRejectionV1::Ballot(closure)
                | AnonymousAttemptRejectionV1::ConfirmationCapacityUnavailable(closure),
            ) => closures.push(&**closure),
            Some(AnonymousAttemptRejectionV1::PublicFinding(failure)) => checked.push(failure.at),
            Some(AnonymousAttemptRejectionV1::SortitionExhausted(exhaustion)) => {
                checked.push(exhaustion.at);
            }
            None => {}
        }
        checked.extend(closures.iter().map(|closure| closure.closure_anchor));
        if checked
            .iter()
            .any(|anchor| self.check_anchor(*anchor).is_err())
        {
            return Err(restore("anchors"));
        }
        // The opening, the casting clock and the execution set no deadline the reducer
        // checks; their block times still lie on the chain.
        let mut times: Vec<(u64, u64)> = checked
            .iter()
            .map(|anchor| (anchor.height, anchor.timestamp_ms))
            .collect();
        for closure in &closures {
            times.push((
                closure.opening_anchor.height,
                closure.opening_anchor.timestamp_ms,
            ));
            times.push((
                closure.casting_clock_height,
                closure.casting_clock_timestamp_ms,
            ));
        }
        times.extend(
            self.execution
                .as_ref()
                .map(|execution| (execution.at.height, execution.at.timestamp_ms)),
        );
        times.sort_unstable();
        let one_chain = times.windows(2).all(|pair| {
            let ((height, time), (next_height, next_time)) = (pair[0], pair[1]);
            if height == next_height {
                time == next_time
            } else {
                time <= next_time
            }
        });
        if one_chain {
            Ok(())
        } else {
            Err(restore("anchors"))
        }
    }

    fn validate_restored_qualification(&self) -> Result<(), Error> {
        let qualification = &self.qualification;
        let qualifying = self.attempt.stage == GovernanceStageV1::Qualification;
        let started = match (qualification.started, qualification.ended) {
            (None, None) if qualifying && qualification.early_close_ms.is_none() => return Ok(()),
            (Some(started), None) if qualifying => started,
            (Some(started), Some(ended)) if !qualifying => {
                let due = self.end_qualification_due_ms()?;
                if ended.timestamp_ms < due
                    || !follows(ended, started)
                    || qualification
                        .early_close_ms
                        .is_some_and(|early| early >= ended.timestamp_ms)
                {
                    return Err(restore("qualification"));
                }
                started
            }
            _ => return Err(restore("qualification")),
        };
        if started.timestamp_ms <= qualification.created_at_ms
            || qualification
                .early_close_ms
                .is_some_and(|early| early < started.timestamp_ms)
        {
            return Err(restore("qualification"));
        }
        Ok(())
    }

    fn validate_restored_generations(&self, network_id: &NetworkId) -> Result<(), Error> {
        let session = BeaconSessionId::for_network_v1(network_id);
        let sortition = &self.policy.sortition_policy;
        let mut latest = BTreeMap::<ParliamentBody, (usize, usize)>::new();
        let mut previous_slot = None::<u64>;
        for (index, generation) in self.generations.iter().enumerate() {
            let delayed = generation
                .request_height
                .checked_add(sortition.pulse_delay_blocks)
                .ok_or(Error::Schedule)?;
            let earliest = match previous_slot {
                Some(previous) => delayed.max(previous.checked_add(1).ok_or(Error::Schedule)?),
                None => delayed,
            };
            if generation.request_height == 0
                || aligned_sortition_pulse_height_v1(earliest, sortition.pulse_cadence_blocks)
                    != Some(generation.slot)
            {
                return Err(restore("generations"));
            }
            previous_slot = Some(generation.slot);

            check_snapshot(&generation.candidates)?;
            let bodies: Vec<_> = generation
                .elections
                .iter()
                .map(AnonymousElectionV1::body)
                .collect();
            if bodies.is_empty() || !bodies.windows(2).all(|pair| pair[0] < pair[1]) {
                return Err(restore("generations"));
            }
            let evidence = bodies.iter().any(|body| is_binding(*body))
                && generation.candidates.len() < usize::from(MIN_SEALED_SEATS_V1);
            let shape_matches = match generation.status {
                AnonymousGenerationStatusV1::CapacityEvidence { .. } => evidence,
                _ => !evidence && !generation.candidates.is_empty(),
            };
            if !shape_matches {
                return Err(restore("generations"));
            }
            self.validate_restored_generation_bodies(index, generation, &bodies, &latest)?;

            let candidate_count =
                u32::try_from(generation.candidates.len()).map_err(|_| restore("generations"))?;
            for (election_index, election) in generation.elections.iter().enumerate() {
                let body = election.body();
                let required = self
                    .policy
                    .required_body(body)
                    .ok_or(Error::UnknownBody { body })?;
                // Sequences continue the body's previous election, which ended without a
                // roster and was registered at an earlier height.
                let expected_sequence = match latest.get(&body) {
                    None => 0,
                    Some(&(previous_generation, previous_election)) => {
                        let previous_generation = &self.generations[previous_generation];
                        let previous = &previous_generation.elections[previous_election];
                        if previous_generation.no_roster(previous).is_none()
                            || generation.request_height <= previous_generation.request_height
                        {
                            return Err(restore("generations"));
                        }
                        previous
                            .sequence
                            .checked_add(1)
                            .ok_or_else(|| restore("generations"))?
                    }
                };
                if election.sequence != expected_sequence
                    || election.sequence > sortition.max_sortition_retries
                {
                    return Err(restore("generations"));
                }
                let request = &election.request;
                if request.governance_attempt_id != self.attempt.id
                    || request.body_election_attempt_id
                        != BodyElectionAttemptId::derive_v1(
                            self.attempt.id,
                            body,
                            election.sequence,
                        )
                    || request.candidate_root
                        != parliament_candidate_root_v1(
                            self.attempt.id,
                            body,
                            &generation.candidates,
                        )
                    || request.candidate_count != candidate_count
                    || request.target_seats != u32::from(required.target_seats)
                    || request.request_height != generation.request_height
                    || request.pulse_height != generation.slot
                    || request.beacon_session_id != session
                    || request.id != request.canonical_id()
                    || (!evidence && request.validate(None).is_err())
                {
                    return Err(restore("generations"));
                }
                if body == ParliamentBody::ConfirmationJury {
                    self.validate_restored_confirmation_request(election.sequence, generation)?;
                }
                let invited: BTreeSet<_> = election
                    .primaries
                    .iter()
                    .chain(&election.alternates)
                    .map(|seat| seat.assignment_id)
                    .collect();
                if !election.accepted.is_disjoint(&election.declined)
                    || !election.accepted.is_subset(&invited)
                    || !election.declined.is_subset(&invited)
                {
                    return Err(restore("generations"));
                }
                latest.insert(body, (index, election_index));
            }
            self.validate_restored_draw(network_id, index, generation)?;
        }

        // A narrow Policy approval registered its Confirmation generation atomically.
        if self.confirmation_required()
            && !self.generations.iter().any(|generation| {
                generation.elections.iter().any(|election| {
                    election.body() == ParliamentBody::ConfirmationJury && election.sequence == 0
                })
            })
        {
            return Err(restore("generations"));
        }

        // Every sealed body is the seal of its body's latest election.
        for (body, state) in &self.bodies {
            let &(generation, election) = latest.get(body).ok_or_else(|| restore("bodies"))?;
            let election = &self.generations[generation].elections[election];
            if election.closed
                != Some(AnonymousElectionClosedV1 {
                    at: state.sealed_at,
                    no_roster: None,
                })
            {
                return Err(restore("bodies"));
            }
        }
        Ok(())
    }

    /// The bodies of the generation at `index`: every required body but the Confirmation
    /// Jury for the initial generation; the Confirmation Jury alone for its first
    /// generation; and for a retry, exactly the bodies whose latest elections had ended
    /// without a roster when it was registered.
    fn validate_restored_generation_bodies(
        &self,
        index: usize,
        generation: &AnonymousGenerationV1,
        bodies: &[ParliamentBody],
        latest: &BTreeMap<ParliamentBody, (usize, usize)>,
    ) -> Result<(), Error> {
        if index == 0 {
            let initial: Vec<_> = self
                .policy
                .required_bodies
                .iter()
                .map(|required| required.body)
                .filter(|body| *body != ParliamentBody::ConfirmationJury)
                .collect();
            return if bodies == initial {
                Ok(())
            } else {
                Err(restore("generations"))
            };
        }
        let first_confirmation = bodies == [ParliamentBody::ConfirmationJury]
            && !latest.contains_key(&ParliamentBody::ConfirmationJury);
        if first_confirmation {
            return Ok(());
        }
        let failed: Vec<_> = latest
            .iter()
            .filter_map(|(body, &(generation_index, election_index))| {
                let previous = &self.generations[generation_index];
                no_roster_height(previous, &previous.elections[election_index])
                    .filter(|height| *height <= generation.request_height)
                    .map(|_| *body)
            })
            .collect();
        if failed == bodies {
            Ok(())
        } else {
            Err(restore("generations"))
        }
    }

    /// A Confirmation Jury generation follows a narrow Policy Jury approval: its first
    /// request in the closure's block, a retry later; its snapshot excludes the sealed
    /// Policy Jury.
    fn validate_restored_confirmation_request(
        &self,
        sequence: u32,
        generation: &AnonymousGenerationV1,
    ) -> Result<(), Error> {
        let policy = self
            .results
            .get(&ParliamentBody::PolicyJury)
            .filter(|_| self.confirmation_required())
            .ok_or(Error::ConfirmationNotRequired)?;
        let in_time = if sequence == 0 {
            generation.request_height == policy.result_height
        } else {
            generation.request_height > policy.result_height
        };
        let members: BTreeSet<_> = self
            .bodies
            .get(&ParliamentBody::PolicyJury)
            .ok_or_else(|| restore("generations"))?
            .seats
            .iter()
            .map(|seat| &seat.member)
            .collect();
        if !in_time
            || generation
                .candidates
                .iter()
                .any(|candidate| members.contains(candidate))
        {
            return Err(restore("generations"));
        }
        Ok(())
    }

    /// The generation at `index` is unanswered while it awaits its pulse or recorded
    /// capacity evidence; a drawn one holds exactly the draw of its snapshot and pulse, and
    /// each of its closes the outcome its responses give.
    fn validate_restored_draw(
        &self,
        network_id: &NetworkId,
        index: usize,
        generation: &AnonymousGenerationV1,
    ) -> Result<(), Error> {
        let untouched = |generation: &AnonymousGenerationV1| {
            generation.elections.iter().all(|election| {
                election.primaries.is_empty()
                    && election.alternates.is_empty()
                    && election.accepted.is_empty()
                    && election.declined.is_empty()
                    && election.closed.is_none()
            })
        };
        let AnonymousGenerationStatusV1::Drawn(draw) = &generation.status else {
            return if untouched(generation) {
                Ok(())
            } else {
                Err(restore("generations"))
            };
        };
        if draw.at.height < generation.slot || draw.pulse_id.as_bytes() == &[0; 32] {
            return Err(restore("generations"));
        }
        let derived = self.derive_assignments(network_id, generation, &draw.pulse_output)?;
        match (derived, draw.assignment_cap) {
            (None, None) if untouched(generation) => Ok(()),
            (Some((cap, plans)), Some(stored)) if cap == stored => {
                for (election, (primaries, alternates)) in generation.elections.iter().zip(&plans) {
                    if &election.primaries != primaries || &election.alternates != alternates {
                        return Err(restore("generations"));
                    }
                }
                let close_ms = self.invitation_close_ms(draw.at)?;
                for (election_index, election) in generation.elections.iter().enumerate() {
                    if let Some(closed) = election.closed {
                        if closed.at.timestamp_ms < close_ms || !follows(closed.at, draw.at) {
                            return Err(restore("generations"));
                        }
                        self.validate_restored_close(index, election_index, closed)?;
                    }
                }
                Ok(())
            }
            _ => Err(restore("generations")),
        }
    }

    /// The close of the election at `(generation_index, election_index)` is the one its
    /// responses give at `closed.at`; a sealed one is exactly the stored body.
    fn validate_restored_close(
        &self,
        generation_index: usize,
        election_index: usize,
        closed: AnonymousElectionClosedV1,
    ) -> Result<(), Error> {
        let election = &self.generations[generation_index].elections[election_index];
        let body = election.body();
        let seats = election.seated();
        let no_roster = if seats.is_empty() {
            Some(AnonymousNoRosterKindV1::EmptyRoster)
        } else if is_binding(body) && seats.len() < usize::from(MIN_SEALED_SEATS_V1) {
            Some(AnonymousNoRosterKindV1::InsufficientRoster)
        } else {
            None
        };
        if closed.no_roster != no_roster {
            return Err(restore("generations"));
        }
        if no_roster.is_some() {
            return Ok(());
        }
        let state = self.bodies.get(&body).ok_or_else(|| restore("bodies"))?;
        let seating = AnonymousSeatingV1 {
            election_attempt_id: election.id(),
            body,
            seats,
            no_roster,
        };
        let expected = self.sealed_body_state(
            generation_index,
            election_index,
            &seating,
            state.roster_binding,
            closed.at,
        )?;
        if expected.sealed != state.sealed
            || expected.sealed_at != state.sealed_at
            || expected.seats != state.seats
            || expected.roster_binding != state.roster_binding
        {
            return Err(restore("bodies"));
        }
        Ok(())
    }

    /// The anchor of the item that moved the attempt to `body`'s stage: the end of
    /// qualification for the first body, the admitting closure for the Confirmation Jury,
    /// and otherwise the previous public body's conclusion.
    fn stage_entry(&self, body: ParliamentBody) -> Option<GovernanceAnchorV1> {
        let required = &self.policy.required_bodies;
        let position = required.iter().position(|entry| entry.body == body)?;
        if position == 0 {
            return self.qualification.ended;
        }
        if body == ParliamentBody::ConfirmationJury {
            let policy = self.results.get(&ParliamentBody::PolicyJury)?;
            return Some(policy.ballot()?.closure.closure_anchor);
        }
        let previous = required[..position]
            .iter()
            .rev()
            .find(|entry| entry.body != ParliamentBody::ConfirmationJury)?;
        self.bodies
            .get(&previous.body)?
            .finding
            .as_ref()?
            .concluded_at
    }

    fn validate_restored_phases(&self) -> Result<(), Error> {
        for (body, state) in &self.bodies {
            let stage = stage_for_body(*body);
            let current = self.attempt.stage == stage;
            let passed = self.attempt.stage > stage;
            if !current && !passed {
                if !state.phases.is_empty() {
                    return Err(restore("phases"));
                }
                continue;
            }
            // A body whose stage was reached starts Orientation at once, at the later of
            // its seal and its stage entry.
            let entry = self.stage_entry(*body).ok_or_else(|| restore("phases"))?;
            let Some(first) = state.phases.first() else {
                return Err(restore("phases"));
            };
            if first.phase != DeliberationPhaseV1::Orientation
                || first.entered_at != later_of(state.sealed_at, entry)
            {
                return Err(restore("phases"));
            }
            for pair in state.phases.windows(2) {
                let (previous, next) = (&pair[0], &pair[1]);
                let due = self
                    .phase_due_ms(previous)?
                    .ok_or_else(|| restore("phases"))?;
                if next_phase(previous.phase) != Some(next.phase)
                    || next.entered_at.timestamp_ms < due
                    || !follows(next.entered_at, previous.entered_at)
                    || previous
                        .early_close_ms
                        .is_some_and(|early| early >= next.entered_at.timestamp_ms)
                {
                    return Err(restore("phases"));
                }
            }
            for entry in &state.phases {
                if entry.early_close_ms.is_some_and(|early| {
                    early < entry.entered_at.timestamp_ms
                        || !AnonymousClosePhaseEarlyV1::closes_early(entry.phase)
                }) {
                    return Err(restore("phases"));
                }
            }
            let last = state.phases.last().map(|entry| entry.phase);
            if (passed || self.results.contains_key(body))
                && last != Some(DeliberationPhaseV1::Reflection)
            {
                return Err(restore("phases"));
            }
        }
        Ok(())
    }

    fn validate_restored_findings(&self) -> Result<(), Error> {
        for (body, state) in &self.bodies {
            let Some(finding) = &state.finding else {
                if !is_binding(*body) {
                    return Err(restore("finding"));
                }
                continue;
            };
            if is_binding(*body) {
                return Err(restore("finding"));
            }
            // Nothing is recorded for a body before its Orientation.
            if state.phases.is_empty() {
                if *finding != AnonymousFindingV1::default() {
                    return Err(restore("finding"));
                }
                continue;
            }
            let seats: BTreeSet<_> = state.seats.iter().map(|seat| seat.assignment_id).collect();
            if !finding.excluded.is_subset(&seats)
                || finding.endorsements.iter().any(|(assignment, root)| {
                    !seats.contains(assignment)
                        || finding.excluded.contains(assignment)
                        || *root == [0; 32]
                })
            {
                return Err(restore("finding"));
            }
            let reflection = state.entered(DeliberationPhaseV1::Reflection);
            if !finding.endorsements.is_empty() && reflection.is_none() {
                return Err(restore("finding"));
            }
            // The conclusion ends the body: no phase of it is entered at or after it.
            if finding.concluded_at.is_some_and(|concluded| {
                state
                    .phases
                    .iter()
                    .any(|phase| !follows(concluded, phase.entered_at))
            }) {
                return Err(restore("finding"));
            }
            self.validate_restored_decision(*body, state, finding)?;
            self.validate_restored_finding_outcome(*body, state)?;
        }
        Ok(())
    }

    /// A public body's record ends at its first decisive fact: absences come before every
    /// endorsement and none of them decided the body unless no endorsement followed, and
    /// the record held no decision before its last fact. The decisive fact was recorded
    /// while facts were accepted: at or after Orientation, before the finding window
    /// closed, and in Reflection when an endorsement is recorded.
    fn validate_restored_decision(
        &self,
        body: ParliamentBody,
        state: &AnonymousBodyStateV1,
        finding: &AnonymousFindingV1,
    ) -> Result<(), Error> {
        let seats = state.seats.len();
        let quorum = usize::try_from(parliament_quorum_seats_v1(u32::from(state.sealed.seats)))
            .map_err(|_| restore("finding"))?;
        let mut counts = BTreeMap::<[u8; 32], usize>::new();
        for root in finding.endorsements.values() {
            *counts.entry(*root).or_default() += 1;
        }
        let excluded = finding.excluded.len();
        // The absences alone left the quorum reachable, unless they decided the body.
        let absences_decided = seats.saturating_sub(excluded) < quorum;
        let reachable_without = |removed: AssignmentId| {
            let mut fewer = finding.clone();
            fewer.endorsements.remove(&removed);
            fewer.decision = None;
            !fewer.quorum_unreachable(seats)
        };
        let consistent = match &finding.decision {
            None => {
                !absences_decided
                    && !finding.quorum_unreachable(seats)
                    && counts.values().all(|count| *count < quorum)
            }
            Some(decision) => match &decision.verdict {
                AnonymousFindingVerdictV1::Quorum {
                    result_root,
                    witness,
                } => {
                    let endorsers: Vec<_> = finding
                        .endorsements
                        .iter()
                        .filter_map(|(assignment, root)| {
                            (root == result_root).then_some(*assignment)
                        })
                        .collect();
                    !absences_decided
                        && &endorsers == witness
                        && witness.len() == quorum
                        && counts.values().filter(|count| **count >= quorum).count() == 1
                }
                AnonymousFindingVerdictV1::Unreachable => {
                    finding.quorum_unreachable(seats)
                        && counts.values().all(|count| *count < quorum)
                        && if finding.endorsements.is_empty() {
                            // The absence that made the quorum unreachable was the last.
                            excluded == seats.saturating_sub(quorum).saturating_add(1)
                        } else {
                            !absences_decided
                                && finding
                                    .endorsements
                                    .keys()
                                    .any(|endorsement| reachable_without(*endorsement))
                        }
                }
            },
        };
        let timely = finding.decision.as_ref().is_none_or(|decision| {
            let decided = decision.decided_ms;
            let oriented = state
                .phases
                .first()
                .is_some_and(|orientation| decided >= orientation.entered_at.timestamp_ms);
            let in_window = match state.entered(DeliberationPhaseV1::Reflection) {
                // The window is open from Reflection; an endorsement needs it.
                Some(reflection) => {
                    self.finding_close_ms(body)
                        .is_some_and(|close| decided < close)
                        && (finding.endorsements.is_empty() || decided >= reflection.timestamp_ms)
                }
                // Before Reflection only absences are recorded.
                None => finding.endorsements.is_empty(),
            };
            oriented && in_window
        });
        if consistent && timely {
            Ok(())
        } else {
            Err(restore("finding"))
        }
    }

    /// A public body's result is its quorum witness, concluded at its stored anchor after
    /// the deciding block; its failure is its unreachable quorum or its expired window,
    /// concluded at the rejection's anchor; an unconcluded body has no anchor.
    fn validate_restored_finding_outcome(
        &self,
        body: ParliamentBody,
        state: &AnonymousBodyStateV1,
    ) -> Result<(), Error> {
        let finding = state.finding.as_ref().ok_or_else(|| restore("finding"))?;
        let decision = finding.decision.as_ref();
        let failure = match &self.rejection {
            Some(AnonymousAttemptRejectionV1::PublicFinding(failure)) if failure.body == body => {
                Some(failure)
            }
            _ => None,
        };
        if let Some(result) = self.results.get(&body) {
            let Some(AnonymousFindingVerdictV1::Quorum {
                result_root,
                witness,
            }) = decision.map(|decision| &decision.verdict)
            else {
                return Err(restore("results"));
            };
            let expected = self.public_finding_result(&state.sealed, *result_root, witness)?;
            let concluded = finding.concluded_at.ok_or_else(|| restore("results"))?;
            let decided_before =
                decision.is_some_and(|decision| concluded.timestamp_ms > decision.decided_ms);
            if result.result != AnonymousBodyResultV1::PublicFinding(Box::new(expected))
                || concluded.height != result.result_height
                || !decided_before
            {
                return Err(restore("results"));
            }
        } else if let Some(failure) = failure {
            let consistent = finding.concluded_at == Some(failure.at)
                && match failure.kind {
                    AnonymousPublicFindingFailureKindV1::QuorumUnreachable => {
                        decision.is_some_and(|decision| {
                            decision.verdict == AnonymousFindingVerdictV1::Unreachable
                                && failure.at.timestamp_ms > decision.decided_ms
                        })
                    }
                    AnonymousPublicFindingFailureKindV1::WindowExpired => {
                        decision.is_none()
                            && self
                                .finding_close_ms(body)
                                .is_some_and(|close| failure.at.timestamp_ms >= close)
                    }
                };
            if !consistent {
                return Err(restore("rejection"));
            }
        } else if finding.concluded_at.is_some() {
            return Err(restore("finding"));
        }
        Ok(())
    }

    /// A sortition exhaustion names the election that ended without a roster, the item
    /// that rejected the attempt for it, and an exhaustion that held.
    pub(super) fn validate_restored_exhaustion(
        &self,
        exhaustion: &AnonymousSortitionExhaustionV1,
    ) -> Result<(), Error> {
        let (generation_index, election_index) = self
            .active_election(exhaustion.body)
            .ok_or_else(|| restore("rejection"))?;
        let generation = &self.generations[generation_index];
        let election = &generation.elections[election_index];
        if generation.no_roster(election) != Some(exhaustion.kind) {
            return Err(restore("rejection"));
        }
        let mut exhausted_bodies = Vec::new();
        for entry in &generation.elections {
            if self.exhausted_after(entry.sequence)? {
                exhausted_bodies.push(entry.body());
            }
        }
        let item_matches = match (&generation.status, exhaustion.kind) {
            (
                AnonymousGenerationStatusV1::CapacityEvidence { recorded_at_ms },
                AnonymousNoRosterKindV1::InsufficientElectorate,
            ) => {
                generation_index + 1 == self.generations.len()
                    && exhaustion.at.timestamp_ms > *recorded_at_ms
                    && exhaustion.at.height > generation.request_height
                    && generation.elections.first().map(AnonymousElectionV1::body)
                        == Some(exhaustion.body)
                    && !exhausted_bodies.is_empty()
            }
            (
                AnonymousGenerationStatusV1::Drawn(draw),
                AnonymousNoRosterKindV1::AllocationInfeasible,
            ) => draw.at == exhaustion.at && exhausted_bodies.first() == Some(&exhaustion.body),
            (
                AnonymousGenerationStatusV1::Drawn(_),
                AnonymousNoRosterKindV1::EmptyRoster | AnonymousNoRosterKindV1::InsufficientRoster,
            ) => {
                election.closed.map(|closed| closed.at) == Some(exhaustion.at)
                    && exhausted_bodies.contains(&exhaustion.body)
            }
            _ => false,
        };
        if !item_matches {
            return Err(restore("rejection"));
        }
        Ok(())
    }
}
