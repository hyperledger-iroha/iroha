/// Driver plans over fixture attempts.
mod planner_tests {
    use super::*;
    use iroha_data_model::isi::governance::{
        ParliamentBeginInvitationAcceptanceV1, ParliamentCloseBallotRegistrationV1,
        ParliamentConsumeSortitionPulseBatchV1, ParliamentFailBallotNoResultV1,
        ParliamentFailBodyElectionNoRosterV1, ParliamentLifecycleTransitionV1,
    };

    /// World inputs of a driver plan served from fixed maps.
    #[derive(Default)]
    struct FixedPlanWorld {
        pulses: BTreeMap<(BeaconSessionId, u64), (BeaconPulseId, [u8; 32])>,
        key_session: Option<TleKeySessionId>,
    }

    impl ParliamentPlanWorldV1 for FixedPlanWorld {
        fn verified_pulse(
            &self,
            session: BeaconSessionId,
            height: u64,
        ) -> Option<(BeaconPulseId, [u8; 32])> {
            self.pulses.get(&(session, height)).copied()
        }

        fn fresh_ballot_tle_key_session(&self, _height: u64) -> Option<TleKeySessionId> {
            self.key_session
        }
    }

    fn plan_governance() -> Governance {
        Governance {
            parliament_invitation_phase_blocks: 1,
            parliament_public_finding_phase_blocks: 10,
            parliament_timed_ovn: timed_ovn_policy(),
            ..Governance::default()
        }
    }

    fn policy_sortition_registered() -> (
        ParliamentAttemptStateV1,
        BodyElectionAttemptId,
        SortitionRequestId,
    ) {
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
            3,
            3,
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
        (state, election_id, request_id)
    }

    #[test]
    fn planner_completes_qualification_then_defers_to_the_initial_sortition() {
        let state = policy_only_state();
        let plan = state.plan_driver_v1(
            &FixedPlanWorld::default(),
            &network_id(),
            &plan_governance(),
            7,
            10,
        );
        assert_eq!(
            plan.due,
            vec![
                ParliamentLifecycleTransitionV1::CompleteQualification,
                ParliamentLifecycleTransitionV1::RegisterInitialSortition,
            ]
        );
        assert!(plan.exact.is_empty());
    }

    #[test]
    fn planner_consumes_committed_pulses_and_fails_missing_ones() {
        let (state, election_id, request_id) = policy_sortition_registered();
        let mut governance = governance_for_pending_draws(&state);
        governance.parliament_invitation_phase_blocks = 1;
        let world = FixedPlanWorld {
            pulses: [(
                (beacon_session(13), 20),
                (pulse_id(14), *pulse_id(14).as_bytes()),
            )]
            .into(),
            ..FixedPlanWorld::default()
        };
        // The pulse height is not committed yet: nothing is due and nothing fails.
        let early = state.plan_driver_v1(&world, &network_id(), &governance, 19, 22);
        assert!(early.due.is_empty(), "{early:?}");
        let plan = state.plan_driver_v1(&world, &network_id(), &governance, 20, 23);
        assert_eq!(
            plan.due,
            vec![
                ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(
                    ParliamentConsumeSortitionPulseBatchV1 {
                        request_ids: vec![request_id],
                        beacon_session_id: beacon_session(13),
                        pulse_height: 20,
                        pulse_id: pulse_id(14),
                    }
                ),
                ParliamentLifecycleTransitionV1::BeginInvitationAcceptance(
                    ParliamentBeginInvitationAcceptanceV1 {
                        election_attempt_id: election_id,
                    }
                ),
            ]
        );
        let missing = state.plan_driver_v1(
            &FixedPlanWorld::default(),
            &network_id(),
            &governance,
            20,
            23,
        );
        assert_eq!(
            missing.due,
            vec![ParliamentLifecycleTransitionV1::FailBodyElectionNoRoster(
                ParliamentFailBodyElectionNoRosterV1 {
                    election_attempt_id: election_id,
                }
            )]
        );
    }

    #[test]
    fn planner_deliberates_to_vote_and_registers_the_ballot_at_the_execution_height() {
        let BodyFixture { state, body_id, .. } = sealed_policy_body(3);
        let world = FixedPlanWorld {
            key_session: Some(tle_key_session(5)),
            ..FixedPlanWorld::default()
        };
        let plan = state.plan_driver_v1(&world, &network_id(), &plan_governance(), 19, 22);
        let phases: Vec<_> = plan
            .due
            .iter()
            .map(|transition| match transition {
                ParliamentLifecycleTransitionV1::AdvanceBodyPhase(advance) => {
                    assert_eq!(advance.body_instance_id, body_id);
                    advance.target
                }
                other => panic!("unexpected transition {other:?}"),
            })
            .collect();
        assert_eq!(
            phases,
            vec![
                DeliberationPhaseV1::Orientation,
                DeliberationPhaseV1::Evidence,
                DeliberationPhaseV1::Questions,
                DeliberationPhaseV1::Responses,
                DeliberationPhaseV1::Deliberation,
                DeliberationPhaseV1::Reflection,
                DeliberationPhaseV1::Vote,
            ]
        );
        let [exact] = plan.exact.as_slice() else {
            panic!("one ballot registration: {:?}", plan.exact);
        };
        assert_eq!(exact.height, 22);
        let ParliamentLifecycleTransitionV1::RegisterBallotAttempt(register) = &exact.transition
        else {
            panic!("a ballot registration: {exact:?}");
        };
        let release_height = timed_ballot_schedule(22, timed_ovn_policy())
            .expect("schedule")
            .3;
        let logical = BeaconSessionId::for_network_v1(&network_id());
        assert_eq!(register.sequence, 0);
        assert_eq!(
            register.ballot_attempt_id,
            BallotAttemptId::derive_v1(body_id, 0)
        );
        assert_eq!(register.release_height, release_height);
        assert_eq!(register.release_beacon_session_id, logical);
        assert_eq!(
            register.tle_session_id,
            TleSessionId::derive_v1(
                register.ballot_attempt_id,
                tle_key_session(5),
                logical,
                release_height
            )
        );
        // Without a selectable TLE key session no ballot can be registered.
        let keyless = state.plan_driver_v1(
            &FixedPlanWorld::default(),
            &network_id(),
            &plan_governance(),
            19,
            22,
        );
        assert!(keyless.exact.is_empty());
    }

    #[test]
    fn planner_times_ballot_checkpoints_and_fails_missed_ones() {
        let state = active_timed_ovn_reservation_attempt_fixture_v1(2, 5, 30);
        let (&body_id, &ballot_id) = state.active_ballots.iter().next().expect("active ballot");
        let close = state.ballots[&ballot_id].registration_close_height;
        let plan = state.plan_driver_v1(
            &FixedPlanWorld::default(),
            &network_id(),
            &plan_governance(),
            29,
            32,
        );
        assert!(plan.due.is_empty(), "{plan:?}");
        assert_eq!(
            plan.exact,
            vec![ParliamentExactTransitionV1 {
                height: close,
                transition: ParliamentLifecycleTransitionV1::CloseBallotRegistration(
                    ParliamentCloseBallotRegistrationV1 {
                        ballot_attempt_id: ballot_id,
                    }
                ),
            }]
        );
        let missed = state.plan_driver_v1(
            &FixedPlanWorld::default(),
            &network_id(),
            &plan_governance(),
            close,
            close + 3,
        );
        assert_eq!(
            missed.due,
            vec![ParliamentLifecycleTransitionV1::FailBallotNoResult(
                ParliamentFailBallotNoResultV1 {
                    ballot_attempt_id: ballot_id,
                }
            )]
        );
        assert!(missed.exact.is_empty());
        assert_ne!(body_id, BodyInstanceId::new([0; 32]));
    }

    #[test]
    fn planner_is_empty_for_terminal_attempts() {
        let mut state = policy_only_state();
        state.attempt.status = GovernanceAttemptStatusV1::Rejected;
        let plan = state.plan_driver_v1(
            &FixedPlanWorld::default(),
            &network_id(),
            &plan_governance(),
            7,
            10,
        );
        assert_eq!(plan, ParliamentDriverPlanV1::default());
    }
}
