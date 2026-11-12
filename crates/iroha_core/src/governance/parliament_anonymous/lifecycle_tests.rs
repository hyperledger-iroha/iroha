//! Tests of the anonymous attempt's governance-clock lifecycle (design §8.1): qualification,
//! sortition generations, invitations, seals, retries and budgets, phases, early-close
//! intents, public findings, the schedule and its restore checks.

use iroha_data_model::governance::{
    anonymous_ballot::{AnonymousClosePhaseEarlyV1, ParliamentLifecycleParametersV1},
    types::{
        AssignmentId, BeaconSessionId, BodyElectionAttemptId, BodyInstanceId,
        parliament_quorum_seats_v1,
    },
};

use super::{
    test_driver::{
        ALTERNATES, Answer, CADENCE_BLOCKS, CREATED_HEIGHT, CREATED_MS, Driver, FINDING_ROOT,
        JURY_SEATS, PUBLIC_SEATS, WINDOW_MS, anchor, assert_restores, attempt, candidate,
        lifecycle_parameters, names, network, pipeline, policy_with, public_bodies, pulse,
    },
    *,
};

use AnonymousLifecycleItemV1 as Item;
use AnonymousParliamentErrorV1 as Error;

/// The bodies of the initial generation.
fn initial_bodies() -> Vec<ParliamentBody> {
    pipeline()
        .into_iter()
        .filter(|body| *body != ParliamentBody::ConfirmationJury)
        .collect()
}

/// A driver whose policy's sortition ceilings are `retries` and `redraws`.
fn bounded(retries: u32, redraws: u32, citizens: u32) -> Driver {
    let mut parameters = lifecycle_parameters();
    parameters.sortition.max_sortition_retries = retries;
    parameters.sortition.max_randomness_redraws = redraws;
    Driver::with(
        candidate(),
        attempt(0, RiskTierV1::Standard),
        policy_with(&parameters),
        0,
        citizens,
    )
}

/// A driver that qualified, registered and drew its initial generation.
fn drawn(citizens: u32) -> Driver {
    let mut driver = Driver::new(RiskTierV1::Standard, citizens);
    driver.qualify();
    driver.register_initial();
    driver.advance_to_slot();
    driver
}

/// A drawn driver whose every primary accepted and whose initial generation sealed.
fn sealed() -> Driver {
    let mut driver = drawn(40);
    for body in initial_bodies() {
        driver.answer(body, Answer::Primaries);
    }
    let closed = driver.step();
    assert_eq!(closed.len(), initial_bodies().len());
    driver
}

/// The first `count` seats of the sealed `body`.
fn seats(driver: &Driver, body: ParliamentBody, count: usize) -> Vec<(AssignmentId, AccountId)> {
    driver
        .state
        .body(body)
        .expect("sealed")
        .seats()
        .iter()
        .take(count)
        .map(|seat| (seat.assignment_id, seat.member.clone()))
        .collect()
}

#[test]
fn qualification_starts_after_creation_and_ends_at_its_deadline_or_an_earlier_intent() {
    let mut driver = Driver::new(RiskTierV1::Standard, 40);
    let id = driver.id();
    assert_eq!(
        driver.state.lifecycle_items(),
        Ok(vec![AnonymousDueItemV1 {
            due_ms: CREATED_MS,
            governance_attempt_id: id,
            item: Item::StartQualification,
        }])
    );
    // The creating block's pass ran before the creation.
    let mut same_block = driver.state.clone();
    assert_eq!(
        same_block.start_qualification(anchor(CREATED_HEIGHT, 0, CREATED_MS)),
        Err(Error::NotDue)
    );
    // No intent before qualification starts.
    assert_eq!(
        driver.state.close_qualification_early(id, CREATED_MS),
        Err(Error::NotDue)
    );
    assert_eq!(driver.block(1_000), vec![Item::StartQualification]);
    let started = driver.state.qualification().started().expect("started");
    assert_eq!(
        driver.state.qualification_due_ms(),
        Some(started.timestamp_ms + WINDOW_MS)
    );
    driver.assert_restores();

    // Nothing is due before the deadline; an authorized intent shortens it.
    assert_eq!(driver.block(1_000), Vec::new());
    driver
        .state
        .close_qualification_early(id, driver.now_ms)
        .expect("intent");
    assert_eq!(
        driver.state.close_qualification_early(id, driver.now_ms),
        Err(Error::EarlyCloseReplay)
    );
    driver.assert_restores();
    assert_eq!(driver.block(1), vec![Item::EndQualification]);
    assert_eq!(driver.state.attempt().stage, GovernanceStageV1::Rules);
    assert!(driver.state.qualification().ended().is_some());
    assert_eq!(
        driver.state.close_qualification_early(id, driver.now_ms),
        Err(Error::Stage {
            expected: GovernanceStageV1::Qualification,
            actual: GovernanceStageV1::Rules,
        })
    );
    driver.assert_restores();

    // Without an intent, qualification ends at its deadline exactly.
    let mut patient = Driver::new(RiskTierV1::Standard, 40);
    patient.block(1_000);
    let due = patient.state.qualification_due_ms().expect("due");
    patient.block(due - patient.now_ms - 1);
    assert_eq!(
        patient.state.attempt().stage,
        GovernanceStageV1::Qualification
    );
    assert_eq!(patient.block(1), vec![Item::EndQualification]);
}

#[test]
fn the_initial_generation_draws_every_body_but_the_confirmation_jury_from_one_pulse() {
    let mut driver = Driver::new(RiskTierV1::Standard, 40);
    let id = driver.id();
    // Registration needs the first body's stage.
    assert_eq!(
        driver.state.register_initial_sortition(
            id,
            &network(),
            driver.height,
            driver.now_ms,
            driver.citizens.clone()
        ),
        Err(Error::Stage {
            expected: GovernanceStageV1::Rules,
            actual: GovernanceStageV1::Qualification,
        })
    );
    driver.qualify();
    // A snapshot out of canonical order is refused.
    let mut unordered = driver.citizens.clone();
    unordered.swap(0, 1);
    assert_eq!(
        driver.state.clone().register_initial_sortition(
            id,
            &network(),
            driver.height,
            driver.now_ms,
            unordered
        ),
        Err(Error::CandidateSnapshot)
    );
    let AnonymousRegistrationV1::Requested { slot } = driver.register_initial() else {
        panic!("requests, not capacity evidence");
    };
    // The first native pulse height at or above the containing height plus the delay.
    let delay = driver.state.policy().sortition_policy.pulse_delay_blocks;
    assert!(slot >= driver.height + delay && slot < driver.height + delay + CADENCE_BLOCKS);
    assert_eq!((slot + 1) % CADENCE_BLOCKS, 0);
    assert!(driver.state.risk_locked());
    assert_eq!(
        driver.state.register_initial_sortition(
            id,
            &network(),
            driver.height,
            driver.now_ms,
            driver.citizens.clone()
        ),
        Err(Error::SortitionRegistered)
    );
    let generation = &driver.state.generations()[0];
    let bodies: Vec<_> = generation
        .elections()
        .iter()
        .map(AnonymousElectionV1::body)
        .collect();
    assert_eq!(bodies, initial_bodies());
    assert!(
        generation
            .elections()
            .iter()
            .all(|election| election.sequence() == 0)
    );
    assert_eq!(driver.state.awaiting_slots(), vec![slot]);
    // Nothing is due while the generation waits for its pulse.
    assert_eq!(driver.state.due_lifecycle_items(u64::MAX), Ok(Vec::new()));
    driver.assert_restores();

    // Only the exact slot's pulse of this network draws it, never before the slot.
    let mut early = driver.state.clone();
    assert_eq!(
        early.draw_generation(
            &network(),
            pulse(slot),
            anchor(slot - 1, 0, driver.now_ms + 1)
        ),
        Err(Error::Pulse)
    );
    let mut foreign = pulse(slot);
    foreign.beacon_session_id = BeaconSessionId::new([0x42; 32]);
    assert_eq!(
        early.draw_generation(&network(), foreign, anchor(slot, 0, driver.now_ms + 1)),
        Err(Error::Pulse)
    );
    assert_eq!(
        early.draw_generation(
            &network(),
            pulse(slot + 1),
            anchor(slot + 1, 0, driver.now_ms + 1)
        ),
        Err(Error::UnknownGeneration)
    );

    driver.advance_to_slot();
    assert_eq!(driver.state.awaiting_slots(), Vec::<u64>::new());
    let generation = &driver.state.generations()[0];
    let AnonymousGenerationStatusV1::Drawn(draw) = generation.status() else {
        panic!("drawn");
    };
    assert!(draw.assignment_cap.is_some());
    for election in generation.elections() {
        let target = if BodyDecisionModeV1::of(election.body()) == BodyDecisionModeV1::PublicFinding
        {
            PUBLIC_SEATS
        } else {
            JURY_SEATS
        };
        assert_eq!(election.primaries().len(), usize::from(target));
        assert_eq!(election.alternates().len(), usize::from(ALTERNATES));
    }
    // Every body's invitations close one window after the draw.
    let items = driver.state.lifecycle_items().expect("items");
    assert_eq!(items.len(), initial_bodies().len());
    for item in &items {
        assert_eq!(item.due_ms, draw.at.timestamp_ms + WINDOW_MS);
        assert!(matches!(item.item, Item::CloseInvitations { .. }));
    }
    driver.assert_restores();
}

#[test]
fn responses_are_facts_of_the_open_window() {
    let mut driver = drawn(40);
    let id = driver.id();
    let (generation, election) = driver
        .state
        .active_election(ParliamentBody::RulesCommittee)
        .expect("election");
    let election = &driver.state.generations()[generation].elections()[election];
    let election_id = election.id();
    let invited = election.primaries()[0].member.clone();
    let outsider = test_driver::citizens(41)
        .into_iter()
        .find(|account| !driver.citizens.contains(account))
        .expect("an account outside the snapshot");
    let (policy_generation, policy_election) = driver
        .state
        .active_election(ParliamentBody::PolicyJury)
        .expect("election");
    let policy_election =
        &driver.state.generations()[policy_generation].elections()[policy_election];
    let policy_id = policy_election.id();
    let juror = policy_election.primaries()[0].member.clone();
    let late = policy_election.primaries()[1].member.clone();

    let now = driver.now_ms;
    let state = &mut driver.state;
    assert_eq!(
        state.record_invitation_response(
            id,
            election_id,
            ParliamentBody::RulesCommittee,
            &outsider,
            true,
            now
        ),
        Err(Error::UnknownInvitation)
    );
    assert_eq!(
        state.record_invitation_response(
            id,
            policy_id,
            ParliamentBody::PolicyJury,
            &juror,
            true,
            now
        ),
        Err(Error::CredentialRequired)
    );
    assert_eq!(
        state.accept_binding_seat(
            id,
            election_id,
            ParliamentBody::RulesCommittee,
            &invited,
            now
        ),
        Err(Error::DecisionMode {
            body: ParliamentBody::RulesCommittee
        })
    );
    assert_eq!(
        state.record_invitation_response(
            id,
            policy_id,
            ParliamentBody::RulesCommittee,
            &invited,
            true,
            now
        ),
        Err(Error::UnknownElection)
    );
    state
        .record_invitation_response(
            id,
            election_id,
            ParliamentBody::RulesCommittee,
            &invited,
            true,
            now,
        )
        .expect("accept");
    assert_eq!(
        state.record_invitation_response(
            id,
            election_id,
            ParliamentBody::RulesCommittee,
            &invited,
            false,
            now
        ),
        Err(Error::ResponseReplay)
    );
    state
        .accept_binding_seat(id, policy_id, ParliamentBody::PolicyJury, &juror, now)
        .expect("binding acceptance with its credential");
    assert_restores(state);

    // The window is half-open: a response at its close is late.
    let close = now + WINDOW_MS;
    assert_eq!(
        state.accept_binding_seat(id, policy_id, ParliamentBody::PolicyJury, &late, close),
        Err(Error::InvitationWindowClosed)
    );
}

#[test]
fn an_invitation_close_seals_the_accepted_roster_or_ends_the_election() {
    let mut driver = drawn(40);
    driver.answer(ParliamentBody::RulesCommittee, Answer::First(3));
    driver.answer(ParliamentBody::AgendaCouncil, Answer::Nobody);
    driver.answer(ParliamentBody::InterestPanel, Answer::Primaries);
    driver.answer(ParliamentBody::ReviewPanel, Answer::Primaries);
    driver.answer(ParliamentBody::OversightCommittee, Answer::Primaries);
    driver.answer(ParliamentBody::PolicyJury, Answer::First(2));
    // The seating is read before the close; a binding jury's adapter seals exactly it.
    let close_ms = driver.state.lifecycle_items().expect("items")[0].due_ms;
    let seating = driver
        .state
        .seating(ParliamentBody::RulesCommittee, close_ms)
        .expect("seating");
    assert_eq!(seating.seats.len(), 3);
    assert_eq!(seating.no_roster, None);
    assert_eq!(
        driver
            .state
            .seating(ParliamentBody::RulesCommittee, close_ms - 1),
        Err(Error::NotDue)
    );
    // A public body takes no roster binding; a binding jury needs one of its seats.
    let at = anchor(driver.height + 1, 0, close_ms);
    let mut probe = driver.state.clone();
    assert_eq!(
        probe.close_invitations(
            ParliamentBody::RulesCommittee,
            at,
            Some(test_driver::roster_binding(
                ParliamentBody::RulesCommittee,
                3
            ))
        ),
        Err(Error::RosterBinding)
    );
    // A viable jury seals only with the binding of exactly its seats.
    let mut viable = drawn(40);
    viable.answer(ParliamentBody::PolicyJury, Answer::Primaries);
    let jury_at = anchor(viable.height + 1, 0, close_ms);
    for binding in [
        None,
        Some(test_driver::roster_binding(
            ParliamentBody::PolicyJury,
            usize::from(JURY_SEATS) - 1,
        )),
    ] {
        assert_eq!(
            viable
                .state
                .close_invitations(ParliamentBody::PolicyJury, jury_at, binding),
            Err(Error::RosterBinding)
        );
    }

    let closed = driver.step();
    assert_eq!(closed.len(), initial_bodies().len());
    let state = &driver.state;
    assert_eq!(
        state.no_roster_bodies(),
        BTreeMap::from([
            (ParliamentBody::AgendaCouncil, 0),
            (ParliamentBody::PolicyJury, 0)
        ])
    );
    let (generation, _) = state
        .active_election(ParliamentBody::AgendaCouncil)
        .expect("election");
    let generation = &state.generations()[generation];
    let kind = |body| {
        let (_, election) = state.active_election(body).expect("election");
        generation.no_roster(&generation.elections()[election])
    };
    assert_eq!(
        kind(ParliamentBody::AgendaCouncil),
        Some(AnonymousNoRosterKindV1::EmptyRoster)
    );
    assert_eq!(
        kind(ParliamentBody::PolicyJury),
        Some(AnonymousNoRosterKindV1::InsufficientRoster)
    );
    assert_eq!(
        state
            .sealed_body(ParliamentBody::RulesCommittee)
            .map(|body| body.seats),
        Some(3)
    );
    // The current body starts Orientation at its own seal.
    let rules = state.body(ParliamentBody::RulesCommittee).expect("sealed");
    assert_eq!(rules.phases()[0].entered_at, rules.sealed_at());
    // A body that is not current waits.
    assert!(
        state
            .body(ParliamentBody::InterestPanel)
            .expect("sealed")
            .phases()
            .is_empty()
    );
    assert_eq!(state.attempt().status, GovernanceAttemptStatusV1::Active);
    driver.assert_restores();

    // The retry redraws both failed bodies together, from a fresh slot.
    let first_slot = driver.state.generations()[0].slot();
    let AnonymousRegistrationV1::Requested { slot } = driver.register_retry().expect("retry")
    else {
        panic!("requests");
    };
    assert!(slot > first_slot);
    let retry = driver.state.generations().last().expect("retry");
    let retried: Vec<_> = retry
        .elections()
        .iter()
        .map(|election| (election.body(), election.sequence()))
        .collect();
    assert_eq!(
        retried,
        vec![
            (ParliamentBody::AgendaCouncil, 1),
            (ParliamentBody::PolicyJury, 1)
        ]
    );
    assert_eq!(driver.state.randomness_redraws_used(), Ok(1));
    assert_eq!(driver.register_retry(), Err(Error::NoRetryDue));
    driver.assert_restores();
}

#[test]
fn a_spent_retry_or_redraw_budget_rejects_the_attempt_at_the_failing_close() {
    // One retry: the second empty close of a body leaves no retry.
    let mut driver = bounded(1, 16, 40);
    driver.qualify();
    driver.register_initial();
    driver.advance_to_slot();
    for body in initial_bodies() {
        let answer = if body == ParliamentBody::AgendaCouncil {
            Answer::Nobody
        } else {
            Answer::Primaries
        };
        driver.answer(body, answer);
    }
    driver.step();
    assert_eq!(
        driver.state.attempt().status,
        GovernanceAttemptStatusV1::Active
    );
    driver.block(1);
    driver.register_retry().expect("retry");
    driver.advance_to_slot();
    driver.run_until(|item| {
        matches!(
            item,
            Item::CloseInvitations {
                body: ParliamentBody::AgendaCouncil,
                ..
            }
        )
    });
    let state = &driver.state;
    assert_eq!(state.attempt().status, GovernanceAttemptStatusV1::Rejected);
    let Some(AnonymousAttemptRejectionV1::SortitionExhausted(exhaustion)) = state.rejection()
    else {
        panic!("sortition exhausted");
    };
    assert_eq!(exhaustion.body, ParliamentBody::AgendaCouncil);
    assert_eq!(exhaustion.kind, AnonymousNoRosterKindV1::EmptyRoster);
    assert_eq!(state.lifecycle_items(), Ok(Vec::new()));
    driver.assert_restores();

    // No redraw unit: the first empty close is final.
    let mut driver = bounded(16, 0, 40);
    driver.qualify();
    driver.register_initial();
    driver.advance_to_slot();
    for body in initial_bodies() {
        driver.answer(body, Answer::Nobody);
    }
    driver.step();
    assert_eq!(
        driver.state.attempt().status,
        GovernanceAttemptStatusV1::Rejected
    );
    driver.assert_restores();
}

#[test]
fn capacity_evidence_counts_as_a_generation_and_its_exhaustion_waits_for_the_pass() {
    // Two citizens are below the binding jury's anonymity floor.
    let mut driver = bounded(16, 2, 2);
    driver.qualify();
    let first = driver.register_initial();
    assert!(matches!(
        first,
        AnonymousRegistrationV1::CapacityEvidence {
            exhausted: false,
            ..
        }
    ));
    assert_eq!(driver.state.awaiting_slots(), Vec::<u64>::new());
    assert_eq!(
        driver.state.no_roster_bodies().len(),
        initial_bodies().len()
    );
    assert_eq!(driver.state.lifecycle_items(), Ok(Vec::new()));
    driver.assert_restores();
    // A retry in the same block is refused; a later one records evidence again.
    assert_eq!(driver.register_retry(), Err(Error::RetryTooEarly));
    driver.block(1_000);
    assert!(matches!(
        driver.register_retry(),
        Ok(AnonymousRegistrationV1::CapacityEvidence {
            exhausted: false,
            ..
        })
    ));
    driver.block(1_000);
    let exhausting = driver.register_retry().expect("evidence");
    assert_eq!(
        exhausting,
        AnonymousRegistrationV1::CapacityEvidence {
            slot: driver
                .state
                .generations()
                .last()
                .expect("generation")
                .slot(),
            exhausted: true,
        }
    );
    // The exhausted attempt stays active until its own item rejects it.
    assert_eq!(
        driver.state.attempt().status,
        GovernanceAttemptStatusV1::Active
    );
    assert_eq!(driver.register_retry(), Err(Error::ExhaustionPending));
    let items = driver.state.lifecycle_items().expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item.name(), "RejectExhausted");
    assert_eq!(items[0].due_ms, driver.now_ms);
    driver.assert_restores();
    assert_eq!(names(&driver.block(1)), vec![("RejectExhausted", None)]);
    let state = &driver.state;
    assert!(matches!(
        state.rejection(),
        Some(AnonymousAttemptRejectionV1::SortitionExhausted(
            AnonymousSortitionExhaustionV1 {
                body: ParliamentBody::RulesCommittee,
                kind: AnonymousNoRosterKindV1::InsufficientElectorate,
                ..
            }
        ))
    ));
    driver.assert_restores();
}

#[test]
fn a_generation_without_an_allocation_under_its_cap_ends_at_its_draw() {
    // A cap of one invitation per citizen cannot seat six bodies from ten citizens.
    let mut parameters = lifecycle_parameters();
    parameters.sortition.max_cross_body_assignment_cap = 1;
    let infeasible = |parameters: &ParliamentLifecycleParametersV1| {
        let mut driver = Driver::with(
            candidate(),
            attempt(0, RiskTierV1::Standard),
            policy_with(parameters),
            0,
            10,
        );
        driver.qualify();
        driver.register_initial();
        driver.advance_to_slot();
        driver
    };
    let driver = infeasible(&parameters);
    let generation = &driver.state.generations()[0];
    assert!(matches!(
        generation.status(),
        AnonymousGenerationStatusV1::Drawn(draw) if draw.assignment_cap.is_none()
    ));
    assert!(
        generation
            .elections()
            .iter()
            .all(|election| election.primaries().is_empty())
    );
    assert_eq!(
        driver.state.no_roster_bodies().len(),
        initial_bodies().len()
    );
    assert_eq!(
        driver.state.attempt().status,
        GovernanceAttemptStatusV1::Active
    );
    driver.assert_restores();

    // Without a retry left, the same draw rejects the attempt.
    parameters.sortition.max_sortition_retries = 0;
    let driver = infeasible(&parameters);
    assert!(matches!(
        driver.state.rejection(),
        Some(AnonymousAttemptRejectionV1::SortitionExhausted(
            AnonymousSortitionExhaustionV1 {
                kind: AnonymousNoRosterKindV1::AllocationInfeasible,
                ..
            }
        ))
    ));
    driver.assert_restores();
}

#[test]
fn every_phase_is_anchored_at_its_entry_and_an_intent_shortens_only_that_phase() {
    let mut driver = sealed();
    let id = driver.id();
    let body = ParliamentBody::RulesCommittee;
    let instance = driver
        .state
        .sealed_body(body)
        .expect("sealed")
        .body_instance_id;
    let orientation = driver.state.body(body).expect("sealed").phases()[0];
    // Close Orientation early, one block after it started.
    driver.block(1_000);
    let intent = |phase| AnonymousClosePhaseEarlyV1 {
        body_instance_id: instance,
        phase,
    };
    assert_eq!(
        driver
            .state
            .close_phase_early(id, intent(DeliberationPhaseV1::Evidence), driver.now_ms),
        Err(Error::PhaseMismatch)
    );
    driver
        .state
        .close_phase_early(id, intent(DeliberationPhaseV1::Orientation), driver.now_ms)
        .expect("intent");
    assert_eq!(
        driver
            .state
            .close_phase_early(id, intent(DeliberationPhaseV1::Orientation), driver.now_ms),
        Err(Error::EarlyCloseReplay)
    );
    driver.assert_restores();
    assert_eq!(names(&driver.block(1)), vec![("AdvancePhase", Some(body))]);
    let evidence = *driver
        .state
        .body(body)
        .expect("sealed")
        .current_phase()
        .expect("phase");
    assert_eq!(evidence.phase, DeliberationPhaseV1::Evidence);
    assert!(evidence.entered_at.timestamp_ms < orientation.entered_at.timestamp_ms + WINDOW_MS);
    // Evidence runs its full window from its own entry.
    assert_eq!(
        driver.state.phase_due_ms(&evidence),
        Ok(Some(evidence.entered_at.timestamp_ms + WINDOW_MS))
    );
    // A stale intent for the phase the body left is refused.
    assert_eq!(
        driver
            .state
            .close_phase_early(id, intent(DeliberationPhaseV1::Orientation), driver.now_ms),
        Err(Error::PhaseMismatch)
    );
    driver.assert_restores();
    // The remaining phases advance one per pass at their deadlines; Reflection is never
    // shortened.
    for _ in 0..4 {
        assert_eq!(names(&driver.step()), vec![("AdvancePhase", Some(body))]);
    }
    assert_eq!(
        driver
            .state
            .close_phase_early(id, intent(DeliberationPhaseV1::Reflection), driver.now_ms),
        Err(Error::PhaseMismatch)
    );
    // Only bodies at their stage take intents.
    let interest = driver
        .state
        .sealed_body(ParliamentBody::InterestPanel)
        .expect("sealed")
        .body_instance_id;
    assert!(matches!(
        driver.state.close_phase_early(
            id,
            AnonymousClosePhaseEarlyV1 {
                body_instance_id: interest,
                phase: DeliberationPhaseV1::Orientation,
            },
            driver.now_ms
        ),
        Err(Error::Stage { .. })
    ));
    driver.assert_restores();
}

#[test]
fn a_public_body_is_decided_by_its_quorum_witness_and_concluded_by_the_pass() {
    let mut driver = sealed();
    let id = driver.id();
    let body = ParliamentBody::RulesCommittee;
    let instance = driver
        .state
        .sealed_body(body)
        .expect("sealed")
        .body_instance_id;
    let members = seats(&driver, body, usize::from(PUBLIC_SEATS));
    let quorum = parliament_quorum_seats_v1(u32::from(PUBLIC_SEATS));
    assert_eq!(quorum, 4);
    // Endorsements wait for Reflection.
    assert_eq!(
        driver.state.endorse_public_finding(
            id,
            instance,
            FINDING_ROOT,
            &members[0].1,
            driver.now_ms
        ),
        Err(Error::PhaseMismatch)
    );
    driver.deliberate(body);
    driver.block(1_000);
    let other_root = [0xc1; 32];
    let now = driver.now_ms;
    let state = &mut driver.state;
    assert_eq!(
        state.endorse_public_finding(id, instance, [0; 32], &members[0].1, now),
        Err(Error::InertFindingRoot)
    );
    for (_, member) in &members[..2] {
        state
            .endorse_public_finding(id, instance, FINDING_ROOT, member, now)
            .expect("endorse");
    }
    state
        .endorse_public_finding(id, instance, other_root, &members[2].1, now)
        .expect("endorse");
    assert_eq!(
        state.endorse_public_finding(id, instance, FINDING_ROOT, &members[0].1, now),
        Err(Error::EndorsementReplay)
    );
    assert_eq!(
        state.record_absence(id, instance, members[4].0, &members[4].1, now),
        Err(Error::AbsenceAfterEndorsement)
    );
    // Two endorse the root, one another root, two remain: still undecided.
    assert_eq!(
        state
            .body(body)
            .and_then(|body| body.finding())
            .and_then(AnonymousFindingV1::decision),
        None
    );
    // The last two seats join the root: the second of them brings it to quorum.
    state
        .endorse_public_finding(id, instance, FINDING_ROOT, &members[3].1, now)
        .expect("endorse");
    state
        .endorse_public_finding(id, instance, FINDING_ROOT, &members[4].1, now)
        .expect("decisive endorsement");
    let decision = state
        .body(body)
        .and_then(|body| body.finding())
        .and_then(AnonymousFindingV1::decision)
        .cloned()
        .expect("decided");
    let AnonymousFindingVerdictV1::Quorum {
        result_root,
        witness,
    } = &decision.verdict
    else {
        panic!("quorum");
    };
    assert_eq!(*result_root, FINDING_ROOT);
    assert_eq!(witness.len(), 4);
    assert_eq!(decision.decided_ms, now);
    assert_restores(state);
    // Nothing changes a decided body.
    assert_eq!(
        state.endorse_public_finding(id, instance, other_root, &members[2].1, now),
        Err(Error::FindingDecided)
    );

    // The conclusion runs in a later pass, after the window closed: a timely quorum stays
    // decided.
    let close = driver.state.finding_close_ms(body).expect("window");
    let late = close - driver.now_ms + 5_000;
    assert_eq!(
        names(&driver.block(late)),
        vec![("ConcludeFinding", Some(body))]
    );
    let result = driver.state.result(body).expect("result");
    let AnonymousBodyResultV1::PublicFinding(finding) = &result.result else {
        panic!("public finding");
    };
    assert_eq!(finding.result_root, FINDING_ROOT);
    assert_eq!(finding.finding.endorsing_assignments, *witness);
    assert_eq!(finding.finding.endorsements, quorum);
    assert_eq!(driver.state.attempt().stage, GovernanceStageV1::Agenda);
    // The next body was sealed already, so it starts Orientation at the conclusion.
    let agenda = driver
        .state
        .body(ParliamentBody::AgendaCouncil)
        .expect("sealed");
    assert_eq!(agenda.phases()[0].entered_at.height, result.result_height);
    driver.assert_restores();
}

#[test]
fn absences_can_make_the_quorum_unreachable_before_reflection() {
    let mut driver = sealed();
    let id = driver.id();
    let body = ParliamentBody::RulesCommittee;
    let instance = driver
        .state
        .sealed_body(body)
        .expect("sealed")
        .body_instance_id;
    let members = seats(&driver, body, usize::from(PUBLIC_SEATS));
    driver.block(1_000);
    let now = driver.now_ms;
    let state = &mut driver.state;
    // A member declares only its own absence, once.
    assert_eq!(
        state.record_absence(id, instance, members[0].0, &members[1].1, now),
        Err(Error::UnauthorizedMember)
    );
    state
        .record_absence(id, instance, members[0].0, &members[0].1, now)
        .expect("absence");
    assert_eq!(
        state.record_absence(id, instance, members[0].0, &members[0].1, now),
        Err(Error::AbsenceReplay)
    );
    // Four seats remain for a quorum of four.
    assert!(
        state
            .body(body)
            .and_then(|body| body.finding())
            .and_then(AnonymousFindingV1::decision)
            .is_none()
    );
    state
        .record_absence(id, instance, members[1].0, &members[1].1, now)
        .expect("decisive absence");
    assert!(matches!(
        state
            .body(body)
            .and_then(|body| body.finding())
            .and_then(AnonymousFindingV1::decision),
        Some(AnonymousFindingDecisionV1 {
            verdict: AnonymousFindingVerdictV1::Unreachable,
            ..
        })
    ));
    assert_restores(state);
    // In Orientation, the conclusion is due at once.
    assert_eq!(
        names(&driver.block(1)),
        vec![("ConcludeFinding", Some(body))]
    );
    assert!(matches!(
        driver.state.rejection(),
        Some(AnonymousAttemptRejectionV1::PublicFinding(
            AnonymousPublicFindingFailureV1 {
                kind: AnonymousPublicFindingFailureKindV1::QuorumUnreachable,
                ..
            }
        ))
    ));
    assert_eq!(driver.state.attempt().stage, GovernanceStageV1::Rules);
    driver.assert_restores();
}

#[test]
fn a_public_body_without_a_finding_at_its_window_rejects_the_attempt() {
    let mut driver = sealed();
    let body = ParliamentBody::RulesCommittee;
    driver.deliberate(body);
    let close = driver.state.finding_close_ms(body).expect("window");
    let items = driver.state.lifecycle_items().expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item.name(), "ConcludeFinding");
    assert_eq!(items[0].due_ms, close);
    assert_eq!(names(&driver.step()), vec![("ConcludeFinding", Some(body))]);
    let state = &driver.state;
    assert!(matches!(
        state.rejection(),
        Some(AnonymousAttemptRejectionV1::PublicFinding(
            AnonymousPublicFindingFailureV1 {
                kind: AnonymousPublicFindingFailureKindV1::WindowExpired,
                ..
            }
        ))
    ));
    assert!(state.is_terminal());
    // A late endorsement is no fact of a terminal attempt.
    let instance = state.sealed_body(body).expect("sealed").body_instance_id;
    let member = seats(&driver, body, 1).remove(0).1;
    let id = driver.id();
    assert!(matches!(
        driver.state.clone().endorse_public_finding(
            id,
            instance,
            FINDING_ROOT,
            &member,
            driver.now_ms
        ),
        Err(Error::NotActive { .. })
    ));
    driver.assert_restores();
}

#[test]
fn a_binding_jury_opens_no_earlier_than_its_notice() {
    let driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    let state = &driver.state;
    let jury = state.body(ParliamentBody::PolicyJury).expect("sealed");
    let reflection = jury.current_phase().expect("Reflection");
    assert_eq!(reflection.phase, DeliberationPhaseV1::Reflection);
    let notice = state.policy().binding.expect("binding").vote_notice_ms;
    assert_eq!(
        state.opening_at_ms(ParliamentBody::PolicyJury),
        Some(
            reflection
                .entered_at
                .timestamp_ms
                .max(jury.sealed_at().timestamp_ms + notice)
        )
    );
    // A jury has no finding window, and its Reflection no deadline.
    assert_eq!(state.finding_close_ms(ParliamentBody::PolicyJury), None);
    assert_eq!(state.lifecycle_items(), Ok(Vec::new()));
    assert_eq!(state.opening_at_ms(ParliamentBody::RulesCommittee), None);
}

#[test]
fn the_schedule_orders_items_by_due_time_rank_attempt_kind_and_body() {
    let a = GovernanceAttemptId::new([0x0a; 32]);
    let b = GovernanceAttemptId::new([0x0b; 32]);
    let election = BodyElectionAttemptId::new([0x0e; 32]);
    let instance = BodyInstanceId::new([0x0f; 32]);
    let close = |body| Item::CloseInvitations { body, election };
    let conclude = |body| Item::ConcludeFinding { body, instance };
    let advance = |body| Item::AdvancePhase {
        body,
        phase: DeliberationPhaseV1::Orientation,
    };
    let item = |due_ms, governance_attempt_id, item| AnonymousDueItemV1 {
        due_ms,
        governance_attempt_id,
        item,
    };
    let mut items = vec![
        item(5, a, conclude(ParliamentBody::RulesCommittee)),
        item(5, b, Item::StartQualification),
        item(5, a, close(ParliamentBody::AgendaCouncil)),
        item(5, a, advance(ParliamentBody::RulesCommittee)),
        item(5, a, Item::EndQualification),
        item(5, a, Item::RejectExhausted { slot: 19 }),
        item(5, a, close(ParliamentBody::RulesCommittee)),
        item(4, b, conclude(ParliamentBody::PolicyJury)),
    ];
    items.sort();
    assert_eq!(
        items
            .iter()
            .map(|entry| (entry.governance_attempt_id, entry.item))
            .collect::<Vec<_>>(),
        vec![
            // An earlier deadline first, whatever its rank.
            (b, conclude(ParliamentBody::PolicyJury)),
            // Then rank 0, attempt by attempt, attempt-level kinds first.
            (a, Item::EndQualification),
            (a, advance(ParliamentBody::RulesCommittee)),
            (b, Item::StartQualification),
            // Rank 1.
            (a, Item::RejectExhausted { slot: 19 }),
            (a, close(ParliamentBody::RulesCommittee)),
            (a, close(ParliamentBody::AgendaCouncil)),
            // Rank 2.
            (a, conclude(ParliamentBody::RulesCommittee)),
        ]
    );
    // Costs: a draw is the largest Parliament item and fits the 43-unit minimum budget
    // beside one progress ticket, six SCCP conclusions and one enactment.
    assert_eq!(close(ParliamentBody::PolicyJury).cost(), 8);
    assert_eq!(advance(ParliamentBody::PolicyJury).cost(), 1);
    assert_eq!(Item::RejectExhausted { slot: 19 }.cost(), 1);
    assert_eq!(1 + schedule::DRAW_GENERATION_COST_V1 + 6 + 4, 43);
    const { assert!(schedule::CLOSE_BALLOT_COST_V1 < schedule::DRAW_GENERATION_COST_V1) };
}

#[test]
fn restore_refuses_a_lifecycle_the_reducer_would_not_produce() {
    let driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    let valid = driver.state.clone();
    let refused = |edit: &dyn Fn(&mut AnonymousParliamentAttemptStateV1)| {
        let mut state = valid.clone();
        edit(&mut state);
        state.validate(&network()).expect_err("refused restore")
    };

    // A slot that is not the one its request height derives.
    assert!(matches!(
        refused(&|state| state.generations[0].slot += CADENCE_BLOCKS),
        Error::Restore { .. }
    ));
    // A plan that is not the draw of its snapshot and pulse.
    assert!(matches!(
        refused(&|state| state.generations[0].elections[0].primaries.reverse()),
        Error::Restore { .. }
    ));
    // A snapshot that grew after registration.
    assert!(matches!(
        refused(&|state| {
            let extra = test_driver::citizens(41).pop().expect("citizen");
            state.generations[0].candidates.push(extra);
            state.generations[0].candidates.sort();
        }),
        Error::Restore { .. } | Error::CandidateSnapshot
    ));
    // An acceptance by an assignment that was never invited.
    assert!(matches!(
        refused(&|state| {
            state.generations[0].elections[0]
                .accepted
                .insert(AssignmentId::new([0x5e; 32]));
        }),
        Error::Restore { .. }
    ));
    // A seal of another roster than the accepted one.
    assert!(matches!(
        refused(&|state| {
            state
                .bodies
                .get_mut(&ParliamentBody::RulesCommittee)
                .expect("sealed")
                .seats
                .pop();
        }),
        Error::Restore { .. }
    ));
    // A phase entered before its deadline.
    assert!(matches!(
        refused(&|state| {
            let phases = &mut state
                .bodies
                .get_mut(&ParliamentBody::PolicyJury)
                .expect("sealed")
                .phases;
            phases[1].entered_at.timestamp_ms = phases[0].entered_at.timestamp_ms + 1;
        }),
        Error::Restore { .. }
    ));
    // An early-close intent recorded after the phase ended.
    assert!(matches!(
        refused(&|state| {
            let phases = &mut state
                .bodies
                .get_mut(&ParliamentBody::PolicyJury)
                .expect("sealed")
                .phases;
            phases[0].early_close_ms = Some(phases[1].entered_at.timestamp_ms);
        }),
        Error::Restore { .. }
    ));
    // A quorum witness that is not exactly the root's endorsers.
    assert!(matches!(
        refused(&|state| {
            let finding = state
                .bodies
                .get_mut(&ParliamentBody::RulesCommittee)
                .expect("sealed")
                .finding
                .as_mut()
                .expect("finding");
            let Some(decision) = finding.decision.as_mut() else {
                return;
            };
            if let AnonymousFindingVerdictV1::Quorum { witness, .. } = &mut decision.verdict {
                witness.pop();
            }
        }),
        Error::Restore { .. }
    ));
    // A qualification that ended before its deadline.
    assert!(matches!(
        refused(&|state| {
            let started = state.qualification.started.expect("started");
            state.qualification.ended =
                Some(anchor(started.height + 1, 0, started.timestamp_ms + 1));
        }),
        Error::Restore { .. }
    ));
    // A risk tier unlocked after the initial registration.
    assert!(matches!(
        refused(&|state| state.risk_locked = false),
        Error::RiskTierNotLocked | Error::Restore { .. }
    ));
    // A spent redraw budget the reducer never reaches.
    assert!(matches!(
        refused(&|state| state.randomness_redraws_before_attempt = 7),
        Error::RedrawLineage | Error::Restore { .. }
    ));
}

#[test]
fn public_bodies_conclude_in_pipeline_order_and_every_body_was_drawn_from_one_pulse() {
    let driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    let state = &driver.state;
    let pulse_id = state
        .sealed_body(ParliamentBody::RulesCommittee)
        .expect("sealed")
        .beacon_pulse_id;
    let mut previous = 0;
    for body in public_bodies() {
        let result = state.result(body).expect("result");
        assert!(result.result_height > previous);
        previous = result.result_height;
        assert_eq!(result.sealed.beacon_pulse_id, pulse_id);
    }
    assert_eq!(
        state
            .sealed_body(ParliamentBody::PolicyJury)
            .expect("sealed")
            .beacon_pulse_id,
        pulse_id
    );
    assert_eq!(state.attempt().stage, GovernanceStageV1::PolicyJury);
    assert_eq!(state.randomness_redraws_used(), Ok(0));
}

#[test]
fn an_item_names_its_occurrence_and_a_passed_one_is_refused() {
    let refuse_seal = |_: &AnonymousSeatingV1| Err(Error::RosterBinding);
    // A phase item applies only to the phase it ends.
    let mut driver = sealed();
    let body = ParliamentBody::RulesCommittee;
    let orientation = Item::AdvancePhase {
        body,
        phase: DeliberationPhaseV1::Orientation,
    };
    assert!(
        driver
            .state
            .lifecycle_items()
            .expect("items")
            .iter()
            .any(|item| item.item == orientation)
    );
    assert_eq!(names(&driver.step()), vec![("AdvancePhase", Some(body))]);
    let evidence_due = driver.state.lifecycle_items().expect("items")[0].due_ms;
    let later = anchor(driver.height + 1, 0, evidence_due);
    let mut replay = driver.state.clone();
    assert_eq!(
        replay.apply_lifecycle_item(orientation, later, refuse_seal),
        Err(Error::StaleItem)
    );
    // A conclusion of another instance of the body is refused.
    assert_eq!(
        replay.apply_lifecycle_item(
            Item::ConcludeFinding {
                body,
                instance: BodyInstanceId::new([0x0f; 32]),
            },
            later,
            refuse_seal
        ),
        Err(Error::StaleItem)
    );
    assert_eq!(replay, driver.state, "a refused item changes nothing");

    // A close of an election that a retry superseded is refused.
    let mut driver = drawn(40);
    let stale_close = driver
        .state
        .lifecycle_items()
        .expect("items")
        .into_iter()
        .map(|item| item.item)
        .find(|item| {
            matches!(
                item,
                Item::CloseInvitations {
                    body: ParliamentBody::AgendaCouncil,
                    ..
                }
            )
        })
        .expect("Agenda close");
    for body in initial_bodies() {
        let answer = if body == ParliamentBody::AgendaCouncil {
            Answer::Nobody
        } else {
            Answer::Primaries
        };
        driver.answer(body, answer);
    }
    driver.step();
    driver.block(1);
    driver.register_retry().expect("retry");
    driver.advance_to_slot();
    let close_ms = driver
        .state
        .lifecycle_items()
        .expect("items")
        .into_iter()
        .find(|item| matches!(item.item, Item::CloseInvitations { .. }))
        .expect("retry close")
        .due_ms;
    let mut replay = driver.state.clone();
    assert_eq!(
        replay.apply_lifecycle_item(
            stale_close,
            anchor(driver.height + 1, 0, close_ms),
            refuse_seal
        ),
        Err(Error::StaleItem)
    );
}

#[test]
fn every_new_deadline_is_checked_before_anything_changes() {
    let mut driver = Driver::new(RiskTierV1::Standard, 40);
    let original = driver.state.clone();
    // A time whose deadlines overflow refuses the item and changes nothing.
    assert_eq!(
        driver
            .state
            .start_qualification(anchor(CREATED_HEIGHT + 1, 0, u64::MAX - 1)),
        Err(Error::Schedule)
    );
    assert_eq!(driver.state, original);
    let mut late_draw = Driver::new(RiskTierV1::Standard, 40);
    late_draw.qualify();
    late_draw.register_initial();
    let slot = late_draw.state.awaiting_slots()[0];
    let before = late_draw.state.clone();
    assert_eq!(
        late_draw
            .state
            .draw_generation(&network(), pulse(slot), anchor(slot, 0, u64::MAX - 1)),
        Err(Error::Schedule)
    );
    assert_eq!(late_draw.state, before);
}

#[test]
fn restore_requires_the_confirmation_generation_a_narrow_approval_registered() {
    let mut driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    assert_eq!(
        driver.close_ballot(ParliamentBody::PolicyJury, (13, 12, 0)),
        Ok(AnonymousBallotDecisionV1::ConfirmationRequired)
    );
    driver.assert_restores();
    let mut missing = driver.state.clone();
    missing.generations.pop();
    assert!(matches!(
        missing.validate(&network()),
        Err(Error::Restore { .. })
    ));
}

#[test]
fn restore_requires_every_body_without_a_roster_in_its_retry() {
    let mut driver = bounded(16, 16, 2);
    driver.qualify();
    driver.register_initial();
    driver.block(1_000);
    driver.register_retry().expect("evidence");
    driver.assert_restores();
    let mut partial = driver.state.clone();
    partial
        .generations
        .last_mut()
        .expect("retry")
        .elections
        .retain(|election| election.body() != ParliamentBody::AgendaCouncil);
    assert!(matches!(
        partial.validate(&network()),
        Err(Error::Restore { .. })
    ));
}

#[test]
fn restore_refuses_a_finding_record_outside_its_lifecycle() {
    // An absence in a body that has not started.
    let driver = sealed();
    let mut early = driver.state.clone();
    let agenda = early
        .bodies
        .get_mut(&ParliamentBody::AgendaCouncil)
        .expect("sealed");
    let seat = agenda.seats[0].assignment_id;
    agenda
        .finding
        .as_mut()
        .expect("finding")
        .excluded
        .insert(seat);
    assert_eq!(
        early.validate(&network()),
        Err(Error::Restore { field: "finding" })
    );

    // An absence after the absence that made the quorum unreachable.
    let mut driver = sealed();
    let id = driver.id();
    let body = ParliamentBody::RulesCommittee;
    let instance = driver
        .state
        .sealed_body(body)
        .expect("sealed")
        .body_instance_id;
    let members = seats(&driver, body, 3);
    driver.block(1_000);
    for (assignment, member) in &members[..2] {
        driver
            .state
            .record_absence(id, instance, *assignment, member, driver.now_ms)
            .expect("absence");
    }
    driver.assert_restores();
    let mut extra = driver.state.clone();
    extra
        .bodies
        .get_mut(&body)
        .and_then(|state| state.finding.as_mut())
        .expect("finding")
        .excluded
        .insert(members[2].0);
    assert_eq!(
        extra.validate(&network()),
        Err(Error::Restore { field: "finding" })
    );
}

#[test]
fn restore_derives_each_orientation_from_its_conclusion_and_each_ballot_from_its_reflection() {
    let mut driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    // Agenda started Orientation at the Rules conclusion's anchor, which the record keeps.
    let concluded = driver
        .state
        .body(ParliamentBody::RulesCommittee)
        .and_then(|state| state.finding())
        .and_then(AnonymousFindingV1::concluded_at)
        .expect("concluded");
    let agenda = driver
        .state
        .body(ParliamentBody::AgendaCouncil)
        .expect("sealed");
    assert_eq!(agenda.phases()[0].entered_at, concluded);
    let mut shifted = driver.state.clone();
    shifted
        .bodies
        .get_mut(&ParliamentBody::AgendaCouncil)
        .expect("sealed")
        .phases[0]
        .entered_at
        .timestamp_ms -= 1;
    assert!(matches!(
        shifted.validate(&network()),
        Err(Error::Restore { .. })
    ));

    // A restored approval opened after its jury's Reflection, never at it.
    assert_eq!(
        driver.close_ballot(ParliamentBody::PolicyJury, (20, 5, 0)),
        Ok(AnonymousBallotDecisionV1::Certified)
    );
    driver.assert_restores();
    let opening = driver
        .state
        .result(ParliamentBody::PolicyJury)
        .and_then(AnonymousParliamentBodyBindingV1::ballot)
        .expect("ballot")
        .closure
        .opening_anchor;
    let mut reopened = driver.state.clone();
    reopened
        .bodies
        .get_mut(&ParliamentBody::PolicyJury)
        .and_then(|state| state.phases.last_mut())
        .expect("Reflection")
        .entered_at = opening;
    assert!(reopened.validate(&network()).is_err());
}

#[test]
fn restore_places_every_recorded_time_on_one_chain() {
    let driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    let rules = ParliamentBody::RulesCommittee;
    let agenda = ParliamentBody::AgendaCouncil;
    // Two positions of one block at different times.
    let mut split = driver.state.clone();
    split.bodies.get_mut(&agenda).expect("sealed").phases[0]
        .entered_at
        .timestamp_ms += 1;
    assert_eq!(
        split.validate(&network()),
        Err(Error::Restore { field: "anchors" })
    );

    // A conclusion moved into its Reflection's block with its later time kept: the
    // deciding endorsement could not have been processed before it.
    let mut early = driver.state.clone();
    let reflection = early.bodies[&rules]
        .phases
        .last()
        .expect("Reflection")
        .entered_at;
    let concluded = early.bodies[&rules]
        .finding
        .as_ref()
        .and_then(|finding| finding.concluded_at)
        .expect("concluded");
    let moved = anchor(
        reflection.height,
        reflection.position + 1,
        concluded.timestamp_ms,
    );
    early
        .bodies
        .get_mut(&rules)
        .and_then(|state| state.finding.as_mut())
        .expect("finding")
        .concluded_at = Some(moved);
    early.results.get_mut(&rules).expect("result").result_height = moved.height;
    early.bodies.get_mut(&agenda).expect("sealed").phases[0].entered_at = moved;
    assert_eq!(
        early.validate(&network()),
        Err(Error::Restore { field: "anchors" })
    );
}

#[test]
fn restore_places_every_decisive_fact_in_its_window() {
    let rules = ParliamentBody::RulesCommittee;
    // A quorum decision at the close of the window, when no fact is accepted.
    let mut driver = sealed();
    driver.deliberate(rules);
    driver.block(1_000);
    driver.endorse_quorum(rules);
    driver.assert_restores();
    let close = driver.state.finding_close_ms(rules).expect("window");
    let mut late = driver.state.clone();
    late.bodies
        .get_mut(&rules)
        .and_then(|state| state.finding.as_mut())
        .and_then(|finding| finding.decision.as_mut())
        .expect("decided")
        .decided_ms = close;
    assert_eq!(
        late.validate(&network()),
        Err(Error::Restore { field: "finding" })
    );

    // An unreachable quorum decided before the body's Orientation.
    let mut driver = sealed();
    let id = driver.id();
    let instance = driver
        .state
        .sealed_body(rules)
        .expect("sealed")
        .body_instance_id;
    driver.block(1_000);
    for (assignment, member) in seats(&driver, rules, 2) {
        driver
            .state
            .record_absence(id, instance, assignment, &member, driver.now_ms)
            .expect("absence");
    }
    driver.assert_restores();
    let orientation = driver.state.body(rules).expect("sealed").phases()[0].entered_at;
    let mut early = driver.state.clone();
    early
        .bodies
        .get_mut(&rules)
        .and_then(|state| state.finding.as_mut())
        .and_then(|finding| finding.decision.as_mut())
        .expect("decided")
        .decided_ms = orientation.timestamp_ms - 1;
    assert_eq!(
        early.validate(&network()),
        Err(Error::Restore { field: "finding" })
    );
}

#[test]
fn an_overflowing_anchor_is_refused_before_the_seal_is_asked() {
    let mut driver = drawn(40);
    driver.answer(ParliamentBody::PolicyJury, Answer::Primaries);
    let close = driver
        .state
        .lifecycle_items()
        .expect("items")
        .into_iter()
        .map(|item| item.item)
        .find(|item| {
            matches!(
                item,
                Item::CloseInvitations {
                    body: ParliamentBody::PolicyJury,
                    ..
                }
            )
        })
        .expect("Policy close");
    let asked = std::cell::Cell::new(false);
    let mut state = driver.state.clone();
    assert_eq!(
        state.apply_lifecycle_item(close, anchor(driver.height + 1, 0, u64::MAX - 1), |_| {
            asked.set(true);
            Err(Error::RosterBinding)
        }),
        Err(Error::Schedule)
    );
    assert!(!asked.get(), "the seal is never asked for a refused item");
    assert_eq!(state, driver.state);
}

#[test]
fn restore_checks_the_anchor_of_every_closure_and_rejection() {
    let mut driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    assert_eq!(
        driver.close_ballot(ParliamentBody::PolicyJury, (5, 20, 0)),
        Ok(AnonymousBallotDecisionV1::Rejected(
            BallotOutcomeV1::Rejected
        ))
    );
    driver.assert_restores();
    let mut late = driver.state.clone();
    let Some(AnonymousAttemptRejectionV1::Ballot(closure)) = late.rejection.as_mut() else {
        panic!("a rejected ballot");
    };
    // `close_ballot` refuses a closure whose deadlines cannot be expressed; so does restore.
    closure.closure_anchor.timestamp_ms = u64::MAX;
    assert_eq!(
        late.validate(&network()),
        Err(Error::Restore { field: "anchors" })
    );
}
