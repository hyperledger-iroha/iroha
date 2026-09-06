#[test]
fn narrow_policy_requires_the_anonymity_floor_of_fresh_confirmation_candidates() {
    for eligible_confirmation_candidates in 0..MIN_PARLIAMENT_HIDDEN_BALLOT_ANONYMITY_V1 {
        let mut fixture = opened_policy_ballot(100, 100);
        let governance_attempt_id = fixture.state.attempt.id;
        let result_height = fixture
            .state
            .ballot(&fixture.ballot_id)
            .and_then(|ballot| ballot.opening_height)
            .expect("fixture opening height");

        assert_eq!(
            finalize_policy_with_confirmation_capacity(
                &mut fixture,
                51,
                49,
                0,
                eligible_confirmation_candidates,
            ),
            ParliamentAggregateOutcomeV1::NoResult
        );
        assert_eq!(
            fixture.state.attempt.status,
            GovernanceAttemptStatusV1::Rejected
        );
        assert_eq!(fixture.state.attempt.stage, GovernanceStageV1::PolicyJury);
        assert_eq!(
            fixture.state.required_bodies.last().map(|entry| entry.body),
            Some(ParliamentBody::PolicyJury),
            "an unfillable Confirmation requirement must never be committed"
        );
        assert!(
            !fixture
                .state
                .body_bindings
                .contains_key(&ParliamentBody::PolicyJury)
        );
        let body = fixture
            .state
            .body(&fixture.body_id)
            .expect("failed Policy Jury body");
        assert_eq!(body.instance.status, BodyInstanceStatusV1::NoResult);
        assert!(body.ballot_binding.is_none());
        assert!(body.result_root.is_none());
        let ballot = fixture
            .state
            .ballot(&fixture.ballot_id)
            .expect("failed Policy Jury ballot");
        assert_eq!(ballot.attempt.status, BallotAttemptStatusV1::NoResult);
        assert_eq!(
            ballot.failure_kind,
            Some(ParliamentBallotFailureKindV1::ConfirmationJuryCapacityUnavailable)
        );
        assert_eq!(
            ballot.eligible_confirmation_candidates,
            Some(eligible_confirmation_candidates)
        );
        assert_eq!(ballot.failure_height, Some(result_height));
        assert_eq!(
            ballot.failure_root,
            Some(parliament_ballot_failure_root_v1(
                governance_attempt_id,
                fixture.ballot_id,
                ParliamentBallotFailureKindV1::ConfirmationJuryCapacityUnavailable,
                result_height,
            ))
        );
        assert_eq!(ballot.outcome, Some(ParliamentAggregateOutcomeV1::Approved));
        fixture
            .state
            .validate()
            .expect("capacity no-result transcript must restore canonically");
        let mut nonterminal = fixture.state.clone();
        nonterminal.attempt.status = GovernanceAttemptStatusV1::Active;
        assert_eq!(
            nonterminal.validate(),
            Err(ParliamentReducerErrorV1::BallotFailureKindMismatch),
            "Confirmation capacity failure cannot leave an active retry path"
        );
        let mut retryable = fixture.state.clone();
        retryable
            .ballots
            .get_mut(&fixture.ballot_id)
            .expect("failed ballot")
            .attempt
            .status = BallotAttemptStatusV1::Superseded;
        assert_eq!(
            retryable.validate(),
            Err(ParliamentReducerErrorV1::BallotFailureKindMismatch),
            "Confirmation capacity failure cannot be superseded by a ballot retry"
        );
    }
}

#[test]
fn narrow_policy_at_randomness_redraw_ceiling_persists_terminal_no_result() {
    let mut fixture = opened_policy_ballot(100, 100);
    fixture.state.randomness_redraws_before_attempt = MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1;
    let governance_attempt_id = fixture.state.attempt.id;
    let result_height = fixture
        .state
        .ballot(&fixture.ballot_id)
        .and_then(|ballot| ballot.opening_height)
        .expect("fixture opening height");

    assert_eq!(
        finalize_policy_with_confirmation_capacity(
            &mut fixture,
            51,
            49,
            0,
            MIN_PARLIAMENT_HIDDEN_BALLOT_ANONYMITY_V1,
        ),
        ParliamentAggregateOutcomeV1::NoResult
    );
    assert_eq!(
        fixture.state.attempt.status,
        GovernanceAttemptStatusV1::Rejected
    );
    assert_eq!(fixture.state.attempt.stage, GovernanceStageV1::PolicyJury);
    assert_eq!(
        fixture.state.required_bodies.last().map(|entry| entry.body),
        Some(ParliamentBody::PolicyJury),
        "the unaffordable Confirmation draw must never enter the pipeline"
    );
    assert!(
        !fixture
            .state
            .body_bindings
            .contains_key(&ParliamentBody::PolicyJury),
        "the narrow Policy result must remain uncommitted"
    );
    let body = fixture
        .state
        .body(&fixture.body_id)
        .expect("failed Policy Jury body");
    assert_eq!(body.instance.status, BodyInstanceStatusV1::NoResult);
    assert!(body.ballot_binding.is_none());
    assert!(body.result_root.is_none());
    let ballot = fixture
        .state
        .ballot(&fixture.ballot_id)
        .expect("failed Policy Jury ballot");
    assert_eq!(ballot.attempt.status, BallotAttemptStatusV1::NoResult);
    assert_eq!(
        ballot.failure_kind,
        Some(ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted)
    );
    assert_eq!(
        ballot.eligible_confirmation_candidates,
        Some(MIN_PARLIAMENT_HIDDEN_BALLOT_ANONYMITY_V1)
    );
    assert_eq!(ballot.failure_height, Some(result_height));
    assert_eq!(
        ballot.failure_root,
        Some(parliament_ballot_failure_root_v1(
            governance_attempt_id,
            fixture.ballot_id,
            ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted,
            result_height,
        ))
    );
    assert_eq!(ballot.outcome, Some(ParliamentAggregateOutcomeV1::Approved));
    assert_eq!(
        ParliamentNoResultKindV1::from(
            ParliamentBallotFailureKindV1::RandomnessRedrawBudgetExhausted
        ),
        ParliamentNoResultKindV1::RandomnessRedrawBudgetExhausted
    );
    fixture
        .state
        .validate()
        .expect("redraw-exhausted opening must restore canonically");

    let bytes = norito::to_bytes(&fixture.state).expect("encode terminal opening");
    let decoded = norito::decode_from_bytes::<ParliamentAttemptStateV1>(&bytes)
        .expect("decode terminal opening");
    assert_eq!(decoded, fixture.state);
    decoded
        .validate()
        .expect("Norito-decoded redraw exhaustion must restore canonically");
    assert_eq!(
        norito::to_bytes(&decoded).expect("re-encode terminal opening"),
        bytes
    );

    let mut below_ceiling = decoded.clone();
    below_ceiling.randomness_redraws_before_attempt -= 1;
    assert_eq!(
        below_ceiling.validate(),
        Err(ParliamentReducerErrorV1::BallotFailureKindMismatch),
        "the redraw-exhaustion classification requires the exact shared ceiling"
    );
    let mut disguised_as_capacity = decoded;
    let disguised_ballot = disguised_as_capacity
        .ballots
        .get_mut(&fixture.ballot_id)
        .expect("terminal ballot");
    disguised_ballot.failure_kind =
        Some(ParliamentBallotFailureKindV1::ConfirmationJuryCapacityUnavailable);
    disguised_ballot.failure_root = Some(parliament_ballot_failure_root_v1(
        governance_attempt_id,
        fixture.ballot_id,
        ParliamentBallotFailureKindV1::ConfirmationJuryCapacityUnavailable,
        result_height,
    ));
    assert_eq!(
        disguised_as_capacity.validate(),
        Err(ParliamentReducerErrorV1::BallotFailureKindMismatch),
        "a floor-sized eligible set cannot be reclassified as capacity unavailable"
    );
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
    } = sealed_policy_body(4);
    advance_to_vote(&mut state, body_id);
    let governance_attempt_id = state.attempt.id;
    let ballot_id = BallotAttemptId::derive_v1(body_id, 0);
    let release_beacon_session_id = beacon_session(24);
    let tle_key_session_id = tle_key_session(23);
    let release_height = 42;
    let tle_session_id = TleSessionId::derive_v1(
        ballot_id,
        tle_key_session_id,
        release_beacon_session_id,
        release_height,
    );
    let four_seat_policy = ParliamentTimedOvn {
        registration_phase_blocks: 5,
        survivor_freeze_phase_blocks: 4,
        max_corpus_entries: 4,
        ..timed_ovn_policy()
    };
    let subfloor_policy = ParliamentTimedOvn {
        max_corpus_entries: 2,
        ..four_seat_policy
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
            subfloor_policy,
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
            four_seat_policy,
            release_height,
        )
        .expect("register ballot with capacity for every original seat");
    let mut registration_window_too_short = state.clone();
    registration_window_too_short
        .ballots
        .get_mut(&ballot_id)
        .expect("registered ballot")
        .registration_phase_blocks = 4;
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
        .survivor_freeze_phase_blocks = 3;
    assert_eq!(
        survivor_window_too_short.validate(),
        Err(ParliamentReducerErrorV1::InvalidBallotSchedule),
        "snapshot validation reserves one authenticated dropout slot per corpus entry"
    );
    state
        .ballots
        .get_mut(&ballot_id)
        .expect("registered ballot")
        .max_corpus_entries = 3;
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
            PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
            10,
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
            PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
            10,
            root(3),
            GovernanceExpectedHeadV1::Present(
                iroha_data_model::governance::types::GovernanceExpectedHeadPresentV1 {
                    subject_id: root(4),
                    version: 1,
                    head_root: [0; 32],
                },
            ),
            required.clone(),
        ),
        Err(ParliamentReducerErrorV1::ImmutableBindingMismatch)
    );
    assert_eq!(
        ParliamentAttemptStateV1::try_new(
            attempt(),
            PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
            10,
            root(3),
            GovernanceExpectedHeadV1::Present(
                iroha_data_model::governance::types::GovernanceExpectedHeadPresentV1 {
                    subject_id: root(4),
                    version: 0,
                    head_root: root(5),
                },
            ),
            required,
        ),
        Err(ParliamentReducerErrorV1::ImmutableBindingMismatch)
    );
}

#[test]
fn attempt_rejects_unsupported_policy_and_noncanonical_decision_modes() {
    let policy_only = vec![RequiredParliamentBodyV1 {
        body: ParliamentBody::PolicyJury,
        decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
    }];
    assert_eq!(
        ParliamentAttemptStateV1::try_new(
            attempt(),
            PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1 + 1,
            10,
            root(3),
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: root(4),
            }),
            policy_only,
        ),
        Err(ParliamentReducerErrorV1::UnsupportedPolicyVersion)
    );
    let mut restored = policy_only_state();
    restored.policy_version = PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1 + 1;
    assert_eq!(
        restored.validate(),
        Err(ParliamentReducerErrorV1::UnsupportedPolicyVersion)
    );

    let hidden_public_body = vec![
        RequiredParliamentBodyV1 {
            body: ParliamentBody::RulesCommittee,
            decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
        },
        RequiredParliamentBodyV1 {
            body: ParliamentBody::PolicyJury,
            decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
        },
    ];
    assert_eq!(
        ParliamentAttemptStateV1::try_new(
            attempt(),
            PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
            10,
            root(3),
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: root(4),
            }),
            hidden_public_body,
        ),
        Err(ParliamentReducerErrorV1::InvalidRequiredBodyPipeline)
    );
}

#[test]
fn sortition_request_requires_the_exact_frozen_pulse_delay_without_overflow() {
    assert_eq!(
        ParliamentAttemptStateV1::try_new(
            attempt(),
            PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
            0,
            root(3),
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: root(4),
            }),
            vec![RequiredParliamentBodyV1 {
                body: ParliamentBody::PolicyJury,
                decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
            }],
        ),
        Err(ParliamentReducerErrorV1::InvalidSortitionPulseSchedule)
    );

    let mut state = policy_only_state();
    let id = state.attempt.id;
    state
        .complete_qualification(id)
        .expect("enter policy stage");
    for (request_height, pulse_height) in [(10, 19), (10, 21), (u64::MAX - 1, u64::MAX)] {
        let (request, candidates) = sortition_request(
            id,
            0,
            ParliamentBody::PolicyJury,
            115,
            3,
            3,
            request_height,
            pulse_height,
            beacon_session(116),
            None,
        );
        assert_eq!(
            state.register_sortition_request(id, 0, request, candidates),
            Err(ParliamentReducerErrorV1::InvalidSortitionPulseSchedule)
        );
    }

    let (request, candidates) = sortition_request(
        id,
        0,
        ParliamentBody::PolicyJury,
        115,
        3,
        3,
        10,
        20,
        beacon_session(116),
        None,
    );
    state
        .register_sortition_request(id, 0, request, candidates)
        .expect("the exact checked request-height plus frozen delay is accepted");
    state.validate().expect("exact frozen schedule persists");
}

#[test]
fn hidden_ballot_sortition_requires_the_anonymity_floor() {
    let mut state = policy_only_state();
    let id = state.attempt.id;
    state
        .complete_qualification(id)
        .expect("enter Policy Jury stage");
    let (request, one_candidate) = sortition_request(
        id,
        0,
        ParliamentBody::PolicyJury,
        115,
        1,
        3,
        10,
        20,
        beacon_session(116),
        None,
    );
    assert_eq!(
        state.register_sortition_request(id, 0, request, one_candidate),
        Err(ParliamentReducerErrorV1::InvalidCandidateSnapshot)
    );
    assert!(state.elections.is_empty());
    assert!(state.candidate_snapshots.is_empty());

    let (request, two_candidates) = sortition_request(
        id,
        0,
        ParliamentBody::PolicyJury,
        117,
        2,
        1,
        10,
        20,
        beacon_session(116),
        None,
    );
    assert_eq!(
        state.register_sortition_request(id, 0, request, two_candidates),
        Err(ParliamentReducerErrorV1::InvalidAssignmentPlan)
    );

    let (request, two_candidates) = sortition_request(
        id,
        0,
        ParliamentBody::PolicyJury,
        117,
        2,
        3,
        10,
        20,
        beacon_session(116),
        None,
    );
    assert_eq!(
        state.register_sortition_request(id, 0, request, two_candidates),
        Err(ParliamentReducerErrorV1::InvalidCandidateSnapshot)
    );

    let (request, three_candidates) = sortition_request(
        id,
        0,
        ParliamentBody::PolicyJury,
        118,
        3,
        3,
        10,
        20,
        beacon_session(116),
        None,
    );
    state
        .register_sortition_request(id, 0, request, three_candidates)
        .expect("the anonymity-floor candidate set can enter hidden sortition");
    state.validate().expect("minimum hidden capacity persists");
}

#[test]
fn hidden_sortition_capacity_failure_is_typed_bounded_and_consumes_no_pulse() {
    let mut state = policy_only_state();
    let id = state.attempt.id;
    state
        .complete_qualification(id)
        .expect("enter Policy Jury stage");

    let mut previous_id = None;
    for sequence in 0..=MAX_PARLIAMENT_SORTITION_RETRIES_V1 {
        let snapshot = if sequence % 2 == 0 {
            Vec::new()
        } else {
            candidates(115, 1)
        };
        let request_height = 10_u64 + u64::from(sequence);
        let request = sortition_request_intent(
            id,
            sequence,
            ParliamentBody::PolicyJury,
            snapshot.clone(),
            3,
            request_height,
            request_height + state.sortition_pulse_delay_blocks(),
            beacon_session(116),
        );
        let election_id = request.body_election_attempt_id;
        state
            .record_hidden_sortition_capacity_failure_batch(
                id,
                vec![ParliamentSortitionRequestRegistrationV1 { sequence, request }],
                snapshot,
            )
            .expect("record objective hidden-electorate capacity failure");

        let failure = state
            .sortition_capacity_failure(&election_id)
            .expect("typed pre-request capacity evidence");
        assert_eq!(failure.sequence(), sequence);
        assert_eq!(failure.failure_height(), request_height);
        assert_eq!(
            failure.candidate_count(),
            usize::try_from(sequence % 2).expect("fixture candidate count fits usize")
        );
        assert_eq!(failure.status(), BodyElectionAttemptStatusV1::NoRoster);
        assert!(state.election(&election_id).is_none());
        assert!(state.used_pulse_ids.is_empty());
        assert!(state.used_pulse_slots.is_empty());
        if let Some(previous_id) = previous_id {
            assert_eq!(
                state
                    .sortition_capacity_failure(&previous_id)
                    .expect("retained prior failure")
                    .status(),
                BodyElectionAttemptStatusV1::Superseded
            );
        }
        if sequence == MAX_PARLIAMENT_SORTITION_RETRIES_V1 {
            assert_eq!(state.attempt.status, GovernanceAttemptStatusV1::Rejected);
        } else {
            assert_eq!(state.attempt.status, GovernanceAttemptStatusV1::Active);
        }
        state
            .validate()
            .expect("typed capacity evidence survives canonical restore validation");
        previous_id = Some(election_id);
    }
}

#[test]
fn hidden_sortition_capacity_restore_rejects_mutated_evidence() {
    let mut state = policy_only_state();
    let id = state.attempt.id;
    state
        .complete_qualification(id)
        .expect("enter Policy Jury stage");
    let snapshot = Vec::new();
    let request = sortition_request_intent(
        id,
        0,
        ParliamentBody::PolicyJury,
        snapshot.clone(),
        3,
        10,
        20,
        beacon_session(116),
    );
    let election_id = request.body_election_attempt_id;
    state
        .record_hidden_sortition_capacity_failure_batch(
            id,
            vec![ParliamentSortitionRequestRegistrationV1 {
                sequence: 0,
                request,
            }],
            snapshot,
        )
        .expect("record zero-candidate evidence");
    state.validate().expect("baseline capacity evidence");

    let mut mutated = state;
    mutated
        .sortition_capacity_failures
        .get_mut(&election_id)
        .expect("capacity evidence")
        .candidate_root = root(0xF1);
    assert_eq!(
        mutated.validate(),
        Err(ParliamentReducerErrorV1::InvalidCandidateSnapshot)
    );
}

#[test]
fn live_sortition_candidates_retain_bonds_until_terminal_or_superseded() {
    let mut state = policy_only_state();
    let id = state.attempt.id;
    state
        .complete_qualification(id)
        .expect("enter Policy Jury stage");
    let (request, first_candidates) = sortition_request(
        id,
        0,
        ParliamentBody::PolicyJury,
        120,
        3,
        3,
        10,
        20,
        beacon_session(121),
        None,
    );
    let first_election_id = request.body_election_attempt_id;
    state
        .register_sortition_request(id, 0, request, first_candidates.clone())
        .expect("register live candidate snapshot");
    assert!(
        first_candidates
            .iter()
            .all(|candidate| state.retains_citizenship_bond(candidate))
    );
    state
        .fail_body_election_no_roster(id, first_election_id, false, 21)
        .expect("terminally fail missing pulse");
    assert!(
        first_candidates
            .iter()
            .all(|candidate| !state.retains_citizenship_bond(candidate)),
        "terminal NoRoster must release every unseated candidate bond"
    );

    let (retry, retry_candidates) = sortition_request(
        id,
        1,
        ParliamentBody::PolicyJury,
        130,
        3,
        3,
        21,
        31,
        beacon_session(121),
        None,
    );
    state
        .register_sortition_request(id, 1, retry, retry_candidates.clone())
        .expect("register fresh retry snapshot");
    assert_eq!(
        state
            .election(&first_election_id)
            .expect("superseded first election")
            .attempt()
            .status,
        BodyElectionAttemptStatusV1::Superseded
    );
    assert!(
        first_candidates
            .iter()
            .all(|candidate| !state.retains_citizenship_bond(candidate)),
        "superseded snapshots must stay released"
    );
    assert!(
        retry_candidates
            .iter()
            .all(|candidate| state.retains_citizenship_bond(candidate)),
        "the live retry snapshot must retain every candidate bond"
    );
}

#[test]
fn terminal_attempt_drops_transient_candidates_but_retains_sealed_member_references() {
    let mut transient = policy_only_state();
    let transient_id = transient.attempt.id;
    transient
        .complete_qualification(transient_id)
        .expect("enter Policy Jury stage");
    let (request, candidates) = sortition_request(
        transient_id,
        0,
        ParliamentBody::PolicyJury,
        124,
        3,
        3,
        10,
        20,
        beacon_session(125),
        None,
    );
    transient
        .register_sortition_request(transient_id, 0, request, candidates.clone())
        .expect("register a transient candidate snapshot");
    assert!(
        candidates
            .iter()
            .all(|candidate| transient.references_parliament_member(candidate))
    );
    transient.attempt.status = GovernanceAttemptStatusV1::Rejected;
    assert!(
        candidates
            .iter()
            .all(|candidate| !transient.references_parliament_member(candidate)),
        "terminal outer attempts must release every unseated candidate reference"
    );

    let BodyFixture {
        mut state, body_id, ..
    } = sealed_policy_body(3);
    let sealed_members = state
        .body(&body_id)
        .expect("sealed body fixture")
        .assignments()
        .iter()
        .map(|assignment| assignment.member.clone())
        .collect::<Vec<_>>();
    state.attempt.status = GovernanceAttemptStatusV1::Rejected;
    assert!(
        sealed_members
            .iter()
            .all(|member| state.references_parliament_member(member)),
        "sealed assignments remain immutable historical references"
    );
    assert!(
        sealed_members
            .iter()
            .all(|member| !state.retains_citizenship_bond(member)),
        "terminal historical references do not retain citizenship bonds"
    );
}

#[test]
fn retryable_singleton_capacity_failure_retains_only_its_live_candidate_bond() {
    let mut state = policy_only_state();
    let id = state.attempt.id;
    state
        .complete_qualification(id)
        .expect("enter Policy Jury stage");

    let mut previous_candidate = None;
    for sequence in 0..=MAX_PARLIAMENT_SORTITION_RETRIES_V1 {
        let candidate_tag = 140_u8
            .checked_add(u8::try_from(sequence).expect("retry sequence fits u8"))
            .expect("fixture candidate tag does not overflow");
        let snapshot = candidates(candidate_tag, 1);
        let candidate = snapshot[0].clone();
        let request_height = 30_u64 + u64::from(sequence);
        let request = sortition_request_intent(
            id,
            sequence,
            ParliamentBody::PolicyJury,
            snapshot.clone(),
            3,
            request_height,
            request_height + state.sortition_pulse_delay_blocks(),
            beacon_session(141),
        );
        state
            .record_hidden_sortition_capacity_failure_batch(
                id,
                vec![ParliamentSortitionRequestRegistrationV1 { sequence, request }],
                snapshot,
            )
            .expect("record singleton capacity failure");

        if let Some(previous_candidate) = previous_candidate {
            assert!(
                !state.retains_citizenship_bond(&previous_candidate),
                "superseded capacity evidence must release its historical candidate"
            );
        }
        if sequence < MAX_PARLIAMENT_SORTITION_RETRIES_V1 {
            assert!(
                state.retains_citizenship_bond(&candidate),
                "the active retryable singleton must retain its candidate bond"
            );
        } else {
            assert_eq!(state.attempt.status, GovernanceAttemptStatusV1::Rejected);
            assert!(
                !state.retains_citizenship_bond(&candidate),
                "final exhaustion must release the terminal singleton candidate"
            );
        }
        state.validate().expect("capacity bond-retention fixture");
        previous_candidate = Some(candidate);
    }
}

#[test]
fn subfloor_hidden_roster_is_an_objective_no_roster_retry() {
    let mut state = policy_only_state();
    let id = state.attempt.id;
    state
        .complete_qualification(id)
        .expect("enter Policy Jury stage");
    let (request, candidates) = sortition_request(
        id,
        0,
        ParliamentBody::PolicyJury,
        117,
        3,
        3,
        10,
        20,
        beacon_session(116),
        None,
    );
    let election_id = request.body_election_attempt_id;
    let request_id = request.id;
    state
        .register_sortition_request(id, 0, request, candidates)
        .expect("register anonymity-floor hidden draw");
    consume_sortition(
        &mut state,
        id,
        vec![request_id],
        beacon_session(116),
        20,
        pulse_id(118),
    )
    .expect("draw three hidden seats");
    state
        .begin_invitation_acceptance(id, election_id, 20, 1)
        .expect("open one-block invitation window");
    let members = state
        .election(&election_id)
        .expect("drawn election")
        .primary_assignments()
        .iter()
        .map(|assignment| assignment.member.clone())
        .collect::<Vec<_>>();
    state
        .record_invitation_response(id, election_id, &members[0], true, 20)
        .expect("accept one hidden seat");
    state
        .record_invitation_response(id, election_id, &members[1], false, 20)
        .expect("decline one hidden seat");
    state
        .record_invitation_response(id, election_id, &members[2], true, 20)
        .expect("accept a second hidden seat");
    assert_eq!(
        state.seal_body_roster(id, election_id, 21),
        Err(ParliamentReducerErrorV1::InvalidRoster)
    );
    state
        .fail_body_election_no_roster(id, election_id, false, 21)
        .expect("two hidden seats cannot form an exact-tally body");
    let election = state.election(&election_id).expect("failed election");
    assert_eq!(
        election.failure_kind,
        Some(ParliamentElectionFailureKindV1::InsufficientHiddenBallotRoster)
    );
    assert_eq!(election.failure_height, Some(21));
    assert_eq!(
        election.attempt.status,
        BodyElectionAttemptStatusV1::NoRoster
    );
    state
        .validate()
        .expect("insufficient hidden roster remains a canonical retry point");
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
        state
            .elections
            .values()
            .all(|election| { election.attempt.status == BodyElectionAttemptStatusV1::Drawing })
    );
    assert!(state.validate().is_ok());
}

#[test]
fn sortition_registration_batch_is_atomic_shared_and_retries_as_one_generation() {
    let required = vec![
        RequiredParliamentBodyV1 {
            body: ParliamentBody::RulesCommittee,
            decision_mode: ParliamentDecisionModeV1::PublicFinding,
        },
        RequiredParliamentBodyV1 {
            body: ParliamentBody::PolicyJury,
            decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
        },
    ];
    let mut base = state(required);
    let id = base.attempt.id;
    base.complete_qualification(id).expect("enter rules stage");

    let initial_candidates = candidates(120, 3);
    let initial = [ParliamentBody::RulesCommittee, ParliamentBody::PolicyJury]
        .into_iter()
        .map(|body| {
            let election_id = BodyElectionAttemptId::derive_v1(id, body, 0);
            let request = SortitionRequestV1::try_new_canonical(
                id,
                election_id,
                body,
                parliament_candidate_root_v1(id, body, &initial_candidates),
                3,
                3,
                10,
                20,
                beacon_session(121),
                None,
            )
            .expect("canonical initial batch request");
            ParliamentSortitionRequestRegistrationV1 {
                sequence: 0,
                request,
            }
        })
        .collect::<Vec<_>>();

    let mut partial = base.clone();
    assert_eq!(
        partial.register_sortition_request_batch(id, vec![initial[0]], initial_candidates.clone(),),
        Err(ParliamentReducerErrorV1::InvalidAssignmentPlan)
    );
    assert_eq!(
        partial, base,
        "a rejected partial batch must not mutate state"
    );

    let mut wrong_order = base.clone();
    assert_eq!(
        wrong_order.register_sortition_request_batch(
            id,
            vec![initial[1], initial[0]],
            initial_candidates.clone(),
        ),
        Err(ParliamentReducerErrorV1::InvalidAssignmentPlan)
    );
    assert_eq!(wrong_order, base, "a rejected ordering must be atomic");

    base.register_sortition_request_batch(id, initial, initial_candidates)
        .expect("register exact full initial batch");
    assert_eq!(base.elections.len(), 2);
    assert_eq!(base.candidate_snapshots.len(), 1);
    base.validate()
        .expect("shared initial snapshot persists once");

    let rules_id = *base
        .active_elections
        .get(&ParliamentBody::RulesCommittee)
        .expect("active rules election");
    base.fail_body_election_no_roster(id, rules_id, false, 21)
        .expect("one missing-slot trigger fails the complete initial generation");
    assert!(base.active_elections.values().all(|election_id| {
        base.elections.get(election_id).is_some_and(|election| {
            election.attempt.status == BodyElectionAttemptStatusV1::NoRoster
        })
    }));

    let retry_candidates = candidates(124, 3);
    let retry = [ParliamentBody::RulesCommittee, ParliamentBody::PolicyJury]
        .into_iter()
        .map(|body| {
            let election_id = BodyElectionAttemptId::derive_v1(id, body, 1);
            let request = SortitionRequestV1::try_new_canonical(
                id,
                election_id,
                body,
                parliament_candidate_root_v1(id, body, &retry_candidates),
                3,
                3,
                21,
                31,
                beacon_session(121),
                None,
            )
            .expect("canonical retry batch request");
            ParliamentSortitionRequestRegistrationV1 {
                sequence: 1,
                request,
            }
        })
        .collect();
    base.register_sortition_request_batch(id, retry, retry_candidates)
        .expect("register one complete fresh initial-draw generation");
    assert_eq!(base.elections.len(), 4);
    assert_eq!(base.candidate_snapshots.len(), 2);
    base.validate()
        .expect("fresh retry generation is persistable");
}

#[test]
fn final_sortition_retry_failure_rejects_and_bounds_persisted_history() {
    let mut state = policy_only_state();
    let id = state.attempt.id;
    state
        .complete_qualification(id)
        .expect("enter policy stage");
    let candidate_snapshot = candidates(130, 3);
    let session = beacon_session(131);

    for sequence in 0..=MAX_PARLIAMENT_SORTITION_RETRIES_V1 {
        let request_height = 10 + u64::from(sequence) * 11;
        let pulse_height = request_height + 10;
        let election_id =
            BodyElectionAttemptId::derive_v1(id, ParliamentBody::PolicyJury, sequence);
        let request = SortitionRequestV1::try_new_canonical(
            id,
            election_id,
            ParliamentBody::PolicyJury,
            parliament_candidate_root_v1(id, ParliamentBody::PolicyJury, &candidate_snapshot),
            3,
            3,
            request_height,
            pulse_height,
            session,
            None,
        )
        .expect("canonical bounded retry request");
        state
            .register_sortition_request_batch(
                id,
                vec![ParliamentSortitionRequestRegistrationV1 { sequence, request }],
                candidate_snapshot.clone(),
            )
            .expect("retry within the hard sortition bound");
        state
            .fail_body_election_no_roster(id, election_id, false, pulse_height + 1)
            .expect("objectively absent retry pulse");
        assert_eq!(
            state.attempt.status,
            if sequence == MAX_PARLIAMENT_SORTITION_RETRIES_V1 {
                GovernanceAttemptStatusV1::Rejected
            } else {
                GovernanceAttemptStatusV1::Active
            }
        );
    }

    assert_eq!(
        state.elections.len(),
        usize::try_from(MAX_PARLIAMENT_SORTITION_RETRIES_V1 + 1).expect("retry bound fits usize")
    );
    assert_eq!(state.candidate_snapshots.len(), 1);
    state
        .validate()
        .expect("exhausted sortition is a canonical terminal attempt");
    state
        .validate_restored_height_v1(10 + u64::from(MAX_PARLIAMENT_SORTITION_RETRIES_V1) * 11 + 11)
        .expect("exhausted sortition restores after its failure height");

    let mut over_limit = policy_only_state();
    over_limit
        .complete_qualification(id)
        .expect("enter policy stage for over-limit admission");
    let sequence = MAX_PARLIAMENT_SORTITION_RETRIES_V1 + 1;
    let election_id = BodyElectionAttemptId::derive_v1(id, ParliamentBody::PolicyJury, sequence);
    let request = SortitionRequestV1::try_new_canonical(
        id,
        election_id,
        ParliamentBody::PolicyJury,
        parliament_candidate_root_v1(id, ParliamentBody::PolicyJury, &candidate_snapshot),
        3,
        3,
        10,
        20,
        session,
        None,
    )
    .expect("structurally valid over-limit request");
    assert_eq!(
        over_limit.register_sortition_request(id, sequence, request, candidate_snapshot),
        Err(ParliamentReducerErrorV1::SortitionRetryLimitExceeded)
    );
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
        3,
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
    let third_primary = election.primary_assignments()[2].clone();
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
    state
        .record_invitation_response(id, election_id, &late_alternate.member, true, 21)
        .expect("second ranked alternate accepts");
    assert_eq!(
        state.record_invitation_response(id, election_id, &third_primary.member, true, 22),
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
    let expected_members: BTreeSet<_> = [
        first_primary.member,
        first_alternate.member,
        late_alternate.member,
    ]
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
        state.fail_body_election_no_roster(id, first_election, false, 22),
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
        .fail_body_election_no_roster(id, first_election_id, false, 21)
        .expect("record no roster");
    let (retry, retry_candidates) = sortition_request(
        id,
        1,
        ParliamentBody::PolicyJury,
        17,
        3,
        3,
        21,
        31,
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
            31,
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
fn restore_rejects_partial_body_creation_and_reducer_impossible_statuses() {
    let fixture = sealed_policy_body(3);
    fixture
        .state
        .validate()
        .expect("sealed body fixture is canonical");

    let mut orphaned_election = fixture.state.clone();
    orphaned_election.bodies.remove(&fixture.body_id);
    orphaned_election
        .active_bodies
        .remove(&ParliamentBody::PolicyJury);
    assert_eq!(
        orphaned_election.validate(),
        Err(ParliamentReducerErrorV1::ImmutableBindingMismatch),
        "Sealed election and body creation are one atomic reducer transition"
    );

    for impossible_status in [
        BodyInstanceStatusV1::AwaitingSortition,
        BodyInstanceStatusV1::AcceptingInvitations,
        BodyInstanceStatusV1::Superseded,
    ] {
        let mut malformed = fixture.state.clone();
        malformed
            .bodies
            .get_mut(&fixture.body_id)
            .expect("fixture body")
            .instance
            .status = impossible_status;
        assert!(matches!(
            malformed.validate(),
            Err(ParliamentReducerErrorV1::InvalidLifecycleTransition(
                ParliamentReducerEntityV1::BodyInstance
            ))
        ));
    }
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
