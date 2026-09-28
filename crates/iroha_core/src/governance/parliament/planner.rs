//! Driver plan of one Parliament attempt (`specs/sccp.md` §4.14.5 item 4).
//!
//! A Parliament driver has no discretion. [`ParliamentAttemptStateV1::plan_driver_v1`] lists
//! every permissionless progress transition the reducer accepts at one execution height, with
//! payloads derived from committed state, the exact-height checkpoints, and the ballots that
//! need off-chain work (a masked-ballot corpus relay or the TLE final release). Each candidate
//! is trial-applied to a copy of the attempt in order, so the plan agrees with the reducer and
//! later steps build on earlier ones (consecutive deliberation phases, for example). World
//! checks outside the reducer (timed-OVN evidence, pulse verification) still run when the
//! transition executes; a plan is advice, never authority.

use iroha_data_model::isi::governance::{
    ParliamentAdvanceBodyPhaseV1, ParliamentBeginBallotOpeningBatchV1,
    ParliamentBeginInvitationAcceptanceV1, ParliamentCloseBallotRegistrationV1,
    ParliamentConsumeSortitionPulseBatchV1, ParliamentFailBallotNoResultV1,
    ParliamentFailBodyElectionNoRosterV1, ParliamentFailPublicFindingNoResultV1,
    ParliamentFreezeBallotSurvivorsV1, ParliamentLifecycleTransitionV1,
    ParliamentRegisterBallotAttemptV1, ParliamentSealBodyRosterV1,
};
use mv::storage::StorageReadOnly as _;

use super::*;

/// Blocks from the committed tip a public transaction is submitted at to the block that
/// executes it: `QueuePlan` admits it at `tip + 1`, carries its autonomous payload at `tip + 2`
/// and merges it at `tip + 3`.
pub const PARLIAMENT_DRIVER_EXECUTION_LAG_BLOCKS: u64 = 3;

/// World inputs of a driver plan.
pub trait ParliamentPlanWorldV1 {
    /// The verified finalized pulse of `session` at `height`, as its id and governance seed.
    fn verified_pulse(
        &self,
        session: BeaconSessionId,
        height: u64,
    ) -> Option<(BeaconPulseId, [u8; 32])>;

    /// The TLE key session a fresh ballot registered at `height` must use.
    fn fresh_ballot_tle_key_session(&self, height: u64) -> Option<TleKeySessionId>;
}

/// A transition valid at exactly one height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParliamentExactTransitionV1 {
    /// The only height at which the transition executes.
    pub height: u64,
    /// The transition.
    pub transition: ParliamentLifecycleTransitionV1,
}

/// Driver plan of one attempt at one execution height.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParliamentDriverPlanV1 {
    /// Transitions the reducer accepts at the execution height, in submission order.
    pub due: Vec<ParliamentLifecycleTransitionV1>,
    /// Transitions valid at exactly one future height, ascending by height.
    pub exact: Vec<ParliamentExactTransitionV1>,
    /// Ballots whose masked-ballot corpus a relayer freezes.
    pub relay_ballots: Vec<BallotAttemptId>,
    /// Ballots waiting for the combined TLE final release.
    pub finalize_ballots: Vec<BallotAttemptId>,
}

/// Apply `apply` to a copy of `state` and, when the reducer accepts it, keep the copy and
/// record `transition`.
fn try_due<T>(
    state: &mut ParliamentAttemptStateV1,
    due: &mut Vec<ParliamentLifecycleTransitionV1>,
    transition: ParliamentLifecycleTransitionV1,
    apply: impl FnOnce(&mut ParliamentAttemptStateV1) -> Result<T, ParliamentReducerErrorV1>,
) -> bool {
    let mut next = state.clone();
    if apply(&mut next).is_err() {
        return false;
    }
    *state = next;
    due.push(transition);
    true
}

impl ParliamentAttemptStateV1 {
    /// Plan this attempt for a transaction executing at `execution_height` over world state
    /// committed at `committed_height`.
    ///
    /// Pulses are consulted only at heights already committed, so a pulse that may still
    /// arrive is never reported unavailable.
    #[must_use]
    pub fn plan_driver_v1(
        &self,
        world: &impl ParliamentPlanWorldV1,
        network_id: &NetworkId,
        governance: &Governance,
        committed_height: u64,
        execution_height: u64,
    ) -> ParliamentDriverPlanV1 {
        let mut plan = ParliamentDriverPlanV1::default();
        if self.attempt.status != GovernanceAttemptStatusV1::Active {
            return plan;
        }
        let id = self.attempt.id;
        let height = execution_height;
        let mut state = self.clone();
        let due = &mut plan.due;

        try_due(
            &mut state,
            due,
            ParliamentLifecycleTransitionV1::CompleteQualification,
            |state| state.complete_qualification(id),
        );
        if state.ensure_initial_sortition_ready_v1(id).is_ok() {
            // Core derives the whole initial generation from world state, so planning resumes
            // once it is registered.
            due.push(ParliamentLifecycleTransitionV1::RegisterInitialSortition);
            return plan;
        }

        // Pulses of committed heights: consume complete batches or fail elections whose pulse
        // never arrived.
        let mut awaiting: BTreeMap<
            (BeaconSessionId, u64),
            Vec<(BodyElectionAttemptId, SortitionRequestId)>,
        > = BTreeMap::new();
        for (election_id, election) in &state.elections {
            let request = &election.attempt.request;
            if election.attempt.status == BodyElectionAttemptStatusV1::AwaitingPulse
                && request.pulse_height <= committed_height
            {
                awaiting
                    .entry((request.beacon_session_id, request.pulse_height))
                    .or_default()
                    .push((*election_id, request.id));
            }
        }
        for ((session, pulse_height), elections) in awaiting {
            if let Some((pulse_id, seed)) = world.verified_pulse(session, pulse_height) {
                let mut request_ids: Vec<_> =
                    elections.iter().map(|(_, request)| *request).collect();
                request_ids.sort_unstable();
                let transition = ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(
                    ParliamentConsumeSortitionPulseBatchV1 {
                        request_ids: request_ids.clone(),
                        beacon_session_id: session,
                        pulse_height,
                        pulse_id,
                    },
                );
                try_due(&mut state, due, transition, |state| {
                    state.consume_sortition_pulse_batch(
                        id,
                        request_ids,
                        session,
                        pulse_height,
                        pulse_id,
                        seed,
                        network_id,
                        governance,
                    )
                });
            } else {
                for (election_id, _) in elections {
                    try_due(
                        &mut state,
                        due,
                        ParliamentLifecycleTransitionV1::FailBodyElectionNoRoster(
                            ParliamentFailBodyElectionNoRosterV1 {
                                election_attempt_id: election_id,
                            },
                        ),
                        |state| state.fail_body_election_no_roster(id, election_id, false, height),
                    );
                }
            }
        }

        // Drawn elections open invitations; closed invitation windows seal or fail.
        let drawing: Vec<_> = state
            .elections
            .iter()
            .filter(|(_, election)| election.attempt.status == BodyElectionAttemptStatusV1::Drawing)
            .map(|(election_id, _)| *election_id)
            .collect();
        for election_id in drawing {
            try_due(
                &mut state,
                due,
                ParliamentLifecycleTransitionV1::BeginInvitationAcceptance(
                    ParliamentBeginInvitationAcceptanceV1 {
                        election_attempt_id: election_id,
                    },
                ),
                |state| {
                    state.begin_invitation_acceptance(
                        id,
                        election_id,
                        height,
                        governance.parliament_invitation_phase_blocks,
                    )
                },
            );
        }
        let accepting: Vec<_> = state
            .elections
            .iter()
            .filter(|(_, election)| {
                election.attempt.status == BodyElectionAttemptStatusV1::AcceptingInvitations
            })
            .map(|(election_id, _)| *election_id)
            .collect();
        for election_id in accepting {
            let sealed = try_due(
                &mut state,
                due,
                ParliamentLifecycleTransitionV1::SealBodyRoster(ParliamentSealBodyRosterV1 {
                    election_attempt_id: election_id,
                }),
                |state| state.seal_body_roster(id, election_id, height),
            );
            if !sealed {
                try_due(
                    &mut state,
                    due,
                    ParliamentLifecycleTransitionV1::FailBodyElectionNoRoster(
                        ParliamentFailBodyElectionNoRosterV1 {
                            election_attempt_id: election_id,
                        },
                    ),
                    |state| state.fail_body_election_no_roster(id, election_id, true, height),
                );
            }
        }

        // Sealed bodies deliberate phase by phase; expired public findings fail.
        let bodies: Vec<_> = state.active_bodies.values().copied().collect();
        for body_id in &bodies {
            let body_id = *body_id;
            loop {
                let Some(status) = state.bodies.get(&body_id).map(|body| body.instance.status)
                else {
                    break;
                };
                let target = match status {
                    BodyInstanceStatusV1::RosterSealed => DeliberationPhaseV1::Orientation,
                    BodyInstanceStatusV1::Deliberating(phase) => {
                        match next_deliberation_phase(phase) {
                            Some(next) => next,
                            None => break,
                        }
                    }
                    _ => break,
                };
                let advanced = try_due(
                    &mut state,
                    due,
                    ParliamentLifecycleTransitionV1::AdvanceBodyPhase(
                        ParliamentAdvanceBodyPhaseV1 {
                            body_instance_id: body_id,
                            target,
                        },
                    ),
                    |state| {
                        state.advance_body_phase(
                            id,
                            body_id,
                            target,
                            height,
                            governance.parliament_public_finding_phase_blocks,
                        )
                    },
                );
                if !advanced {
                    break;
                }
            }
            try_due(
                &mut state,
                due,
                ParliamentLifecycleTransitionV1::FailPublicFindingNoResult(
                    ParliamentFailPublicFindingNoResultV1 {
                        body_instance_id: body_id,
                    },
                ),
                |state| state.fail_public_finding_no_result(id, body_id, height),
            );
        }

        let logical_beacon = BeaconSessionId::for_network_v1(network_id);
        // Live ballots: terminal failures first, then checkpoints and off-chain work.
        let ballots: Vec<_> = state.active_ballots.values().copied().collect();
        let mut openings: BTreeMap<(BeaconSessionId, u64), Vec<BallotAttemptId>> = BTreeMap::new();
        for ballot_id in ballots {
            let Some(ballot) = state.ballots.get(&ballot_id).copied() else {
                continue;
            };
            let release = ballot.release_beacon_session_id.zip(ballot.release_height);
            let release_pulse = release
                .filter(|(_, release_height)| *release_height <= committed_height)
                .map(|(session, release_height)| world.verified_pulse(session, release_height));
            // A release height that is not committed yet cannot prove the pulse absent.
            let release_pulse_available = release_pulse.is_none_or(|pulse| pulse.is_some());
            let failed = try_due(
                &mut state,
                due,
                ParliamentLifecycleTransitionV1::FailBallotNoResult(
                    ParliamentFailBallotNoResultV1 {
                        ballot_attempt_id: ballot_id,
                    },
                ),
                |state| state.fail_ballot_no_result(id, ballot_id, release_pulse_available, height),
            );
            if failed {
                continue;
            }
            match ballot.attempt.status {
                BallotAttemptStatusV1::Registration
                    if ballot.registration_close_height >= height =>
                {
                    plan.exact.push(ParliamentExactTransitionV1 {
                        height: ballot.registration_close_height,
                        transition: ParliamentLifecycleTransitionV1::CloseBallotRegistration(
                            ParliamentCloseBallotRegistrationV1 {
                                ballot_attempt_id: ballot_id,
                            },
                        ),
                    });
                }
                BallotAttemptStatusV1::SurvivorFreeze
                    if ballot.survivor_freeze_height >= height =>
                {
                    plan.exact.push(ParliamentExactTransitionV1 {
                        height: ballot.survivor_freeze_height,
                        transition: ParliamentLifecycleTransitionV1::FreezeBallotSurvivors(
                            ParliamentFreezeBallotSurvivorsV1 {
                                ballot_attempt_id: ballot_id,
                            },
                        ),
                    });
                }
                BallotAttemptStatusV1::TimedCommitment => plan.relay_ballots.push(ballot_id),
                BallotAttemptStatusV1::AwaitingRelease => {
                    if let (Some(release), Some(Some(_))) = (release, release_pulse) {
                        openings.entry(release).or_default().push(ballot_id);
                    }
                }
                BallotAttemptStatusV1::Opening => plan.finalize_ballots.push(ballot_id),
                _ => {}
            }
        }
        for ((session, release_height), mut ballot_ids) in openings {
            let Some((pulse_id, _)) = world.verified_pulse(session, release_height) else {
                continue;
            };
            ballot_ids.sort_unstable();
            let transition = ParliamentLifecycleTransitionV1::BeginBallotOpeningBatch(
                ParliamentBeginBallotOpeningBatchV1 {
                    ballot_attempt_ids: ballot_ids.clone(),
                    release_beacon_session_id: session,
                    release_height,
                    pulse_id,
                },
            );
            try_due(&mut state, due, transition, |state| {
                state.begin_ballot_opening_batch(
                    id,
                    ballot_ids,
                    session,
                    release_height,
                    height,
                    pulse_id,
                )
            });
        }

        // A hidden-ballot body at `Vote`, or one whose last ballot ended without a result,
        // registers the next ballot. Its schedule starts at the execution height, so the
        // registration is exact.
        if let Some(key_session) = world.fresh_ballot_tle_key_session(height) {
            let policy = governance.parliament_timed_ovn;
            for body_id in &bodies {
                let body_id = *body_id;
                let sequence = state
                    .ballots
                    .values()
                    .filter(|ballot| ballot.attempt.body_instance_id == body_id)
                    .map(|ballot| ballot.attempt.sequence.saturating_add(1))
                    .max()
                    .unwrap_or(0);
                let Ok((_, _, _, release_height, _)) = timed_ballot_schedule(height, policy) else {
                    break;
                };
                let ballot_id = BallotAttemptId::derive_v1(body_id, sequence);
                let tle_session_id =
                    TleSessionId::derive_v1(ballot_id, key_session, logical_beacon, release_height);
                let mut trial = state.clone();
                if trial
                    .register_ballot_attempt(
                        id,
                        body_id,
                        ballot_id,
                        sequence,
                        tle_session_id,
                        key_session,
                        logical_beacon,
                        height,
                        policy,
                        release_height,
                    )
                    .is_ok()
                {
                    plan.exact.push(ParliamentExactTransitionV1 {
                        height,
                        transition: ParliamentLifecycleTransitionV1::RegisterBallotAttempt(
                            ParliamentRegisterBallotAttemptV1 {
                                body_instance_id: body_id,
                                ballot_attempt_id: ballot_id,
                                sequence,
                                tle_session_id,
                                tle_key_session_id: key_session,
                                release_beacon_session_id: logical_beacon,
                                release_height,
                            },
                        ),
                    });
                }
            }
        }
        plan.exact.sort_by_key(|exact| exact.height);
        plan
    }
}

/// [`ParliamentPlanWorldV1`] over committed world state.
pub struct WorldPlanInputsV1<'world, W> {
    /// Committed world state.
    pub world: &'world W,
    /// Network whose logical beacon the Parliament uses.
    pub network_id: &'world NetworkId,
}

impl<W: crate::state::WorldReadOnly> ParliamentPlanWorldV1 for WorldPlanInputsV1<'_, W> {
    fn verified_pulse(
        &self,
        session: BeaconSessionId,
        height: u64,
    ) -> Option<(BeaconPulseId, [u8; 32])> {
        if session != BeaconSessionId::for_network_v1(self.network_id) {
            return None;
        }
        let pulse_id = *self
            .world
            .global_beacon_pulse_slots()
            .get(&(session, height))?;
        let pulse = *self.world.global_beacon_pulses().get(&pulse_id)?;
        if pulse.pulse_id != pulse_id
            || pulse.network_id != *self.network_id
            || pulse.height != height
        {
            return None;
        }
        let seed = crate::beacon::verified_persisted_global_threshold_beacon_governance_seed_v1(
            self.world,
            self.network_id,
            pulse,
            height,
        )
        .ok()?;
        Some((BeaconPulseId::new(pulse_id), seed))
    }

    fn fresh_ballot_tle_key_session(&self, height: u64) -> Option<TleKeySessionId> {
        self.world
            .selectable_tle_key_session_for_fresh_ballot_at(height)
    }
}

/// Plan `attempt_id` over committed `state` for a transaction submitted now, returning the
/// committed and execution heights with the plan, or `None` for an unknown attempt.
#[must_use]
pub fn plan_parliament_attempt_v1(
    state: &impl crate::state::StateReadOnly,
    network_id: &NetworkId,
    governance: &Governance,
    attempt_id: GovernanceAttemptId,
) -> Option<(u64, u64, ParliamentDriverPlanV1)> {
    use crate::state::WorldReadOnly as _;
    let committed_height = u64::try_from(state.height()).ok()?;
    let execution_height = committed_height.checked_add(PARLIAMENT_DRIVER_EXECUTION_LAG_BLOCKS)?;
    let world = state.world();
    let attempt = world.parliament_attempts().get(&attempt_id)?;
    let inputs = WorldPlanInputsV1 { world, network_id };
    Some((
        committed_height,
        execution_height,
        attempt.plan_driver_v1(
            &inputs,
            network_id,
            governance,
            committed_height,
            execution_height,
        ),
    ))
}
