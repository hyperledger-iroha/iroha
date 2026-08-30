// Textually included test-corridor fixtures retain the parent module's private access.
#[cfg(any(test, feature = "iroha-core-tests"))]
fn enacted_fixture_governance(
    requirements: &[RequiredParliamentBodyV1],
) -> iroha_config::parameters::actual::Governance {
    let mut governance = iroha_config::parameters::actual::Governance {
        parliament_alternate_size: 0,
        ..iroha_config::parameters::actual::Governance::default()
    };
    for requirement in requirements {
        match requirement.body {
            ParliamentBody::RulesCommittee => governance.rules_committee_size = 3,
            ParliamentBody::AgendaCouncil => governance.agenda_council_size = 3,
            ParliamentBody::InterestPanel => governance.interest_panel_size = 3,
            ParliamentBody::ReviewPanel => governance.review_panel_size = 3,
            ParliamentBody::CoordinationCouncil => governance.coordination_council_size = 3,
            ParliamentBody::MpcCommittee => governance.mpc_committee_size = 3,
            ParliamentBody::FmaCommittee => governance.fma_committee_size = 3,
            ParliamentBody::OversightCommittee => governance.oversight_committee_size = 3,
            ParliamentBody::PolicyJury => governance.policy_jury_size = 3,
            ParliamentBody::ConfirmationJury => governance.confirmation_jury_size = 3,
        }
    }
    governance
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn prepare_enacted_fixture_body(
    attempt: &mut ParliamentAttemptStateV1,
    requirement: RequiredParliamentBodyV1,
    election_attempt_id: BodyElectionAttemptId,
) -> (BodyInstanceId, Vec<AccountId>) {
    let governance_attempt_id = attempt.attempt().id;
    attempt
        .begin_invitation_acceptance(governance_attempt_id, election_attempt_id, 2, 1)
        .expect("open enacted-attempt fixture invitation window");
    let members = attempt
        .election(&election_attempt_id)
        .expect("drawn enacted-attempt fixture election")
        .primary_assignments()
        .iter()
        .map(|assignment| assignment.member.clone())
        .collect::<Vec<_>>();
    for member in &members {
        attempt
            .record_invitation_response(governance_attempt_id, election_attempt_id, member, true, 2)
            .expect("accept enacted-attempt fixture invitation");
    }
    let body_instance_id = attempt
        .seal_body_roster(governance_attempt_id, election_attempt_id, 3)
        .expect("seal enacted-attempt fixture roster");
    let mut phases = vec![
        DeliberationPhaseV1::Orientation,
        DeliberationPhaseV1::Evidence,
        DeliberationPhaseV1::Questions,
        DeliberationPhaseV1::Responses,
        DeliberationPhaseV1::Deliberation,
        DeliberationPhaseV1::Reflection,
    ];
    if requirement.decision_mode == ParliamentDecisionModeV1::HiddenBindingBallot {
        phases.push(DeliberationPhaseV1::Vote);
    }
    for phase in phases {
        attempt
            .advance_body_phase(governance_attempt_id, body_instance_id, phase, 3, 1)
            .expect("advance enacted-attempt fixture deliberation");
    }
    (body_instance_id, members)
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn complete_enacted_fixture_body(
    attempt: &mut ParliamentAttemptStateV1,
    requirement: RequiredParliamentBodyV1,
    election_attempt_id: BodyElectionAttemptId,
    result_tag: u8,
) {
    let governance_attempt_id = attempt.attempt().id;
    let (body_instance_id, members) =
        prepare_enacted_fixture_body(attempt, requirement, election_attempt_id);
    match requirement.decision_mode {
        ParliamentDecisionModeV1::PublicFinding => {
            let result_root = [result_tag.max(1); 32];
            let mut finalized = false;
            for member in &members {
                finalized = attempt
                    .endorse_public_finding(
                        governance_attempt_id,
                        body_instance_id,
                        result_root,
                        member,
                        3,
                    )
                    .expect("endorse enacted-attempt fixture public finding");
                if finalized {
                    break;
                }
            }
            assert!(
                finalized,
                "fixture seats must reach the public-finding quorum"
            );
        }
        ParliamentDecisionModeV1::HiddenBindingBallot => {
            let root = |tag: u8| [tag.max(1); 32];
            let ballot_attempt_id = BallotAttemptId::derive_v1(body_instance_id, 0);
            let release_beacon_session_id = BeaconSessionId::new(root(0xD0));
            let tle_key_session_id = TleKeySessionId::new(root(0xD1));
            let release_height = 12;
            let tle_session_id = TleSessionId::derive_v1(
                ballot_attempt_id,
                tle_key_session_id,
                release_beacon_session_id,
                release_height,
            );
            attempt
                .register_ballot_attempt(
                    governance_attempt_id,
                    body_instance_id,
                    ballot_attempt_id,
                    0,
                    tle_session_id,
                    tle_key_session_id,
                    release_beacon_session_id,
                    3,
                    ParliamentTimedOvn {
                        registration_phase_blocks: 4,
                        survivor_freeze_phase_blocks: 3,
                        commitment_phase_blocks: 1,
                        release_delay_blocks: 1,
                        opening_phase_blocks: 1,
                        max_ballot_retries: 2,
                        max_corpus_entries: 3,
                    },
                    release_height,
                )
                .expect("register enacted-attempt fixture ballot");
            let registration_root = root(0xD2);
            let dropout_root = root(0xD3);
            let survivor_root = root(0xD4);
            let no_recovery_root = root(0xD5);
            let corpus_root = root(0xD6);
            let timed_commitment_root = root(0xD7);
            attempt
                .close_ballot_registration(
                    governance_attempt_id,
                    ballot_attempt_id,
                    registration_root,
                    3,
                    7,
                )
                .expect("close enacted-attempt fixture ballot registration");
            attempt
                .freeze_ballot_survivors(
                    governance_attempt_id,
                    ballot_attempt_id,
                    dropout_root,
                    survivor_root,
                    3,
                    no_recovery_root,
                    10,
                )
                .expect("freeze enacted-attempt fixture ballot survivors");
            attempt
                .freeze_timed_ovn_corpus(
                    governance_attempt_id,
                    ballot_attempt_id,
                    corpus_root,
                    survivor_root,
                    3,
                    timed_commitment_root,
                    11,
                )
                .expect("freeze enacted-attempt fixture timed corpus");
            attempt
                .begin_ballot_opening_batch(
                    governance_attempt_id,
                    vec![ballot_attempt_id],
                    release_beacon_session_id,
                    release_height,
                    release_height,
                    BeaconPulseId::new(root(0xD8)),
                )
                .expect("open enacted-attempt fixture ballot");
            let outcome = attempt
                .finalize_opened_ballot(
                    governance_attempt_id,
                    ballot_attempt_id,
                    corpus_root,
                    no_recovery_root,
                    tle_session_id,
                    root(0xD9),
                    3,
                    ParliamentAggregateTallyV1 {
                        original_seats: 3,
                        accepted_ballots: 3,
                        aye: 2,
                        nay: 1,
                        abstain: 0,
                    },
                    2,
                    release_height,
                )
                .expect("finalize enacted-attempt fixture ballot");
            assert_eq!(outcome, ParliamentAggregateOutcomeV1::Approved);
        }
    }
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn build_enacted_parliament_attempt_for_testing<F>(
    proposal: &ProposalKind,
    mut candidates: Vec<AccountId>,
    network_id: &NetworkId,
    enact_at_height: u64,
    mut complete_body: F,
) -> ParliamentAttemptStateV1
where
    F: FnMut(&mut ParliamentAttemptStateV1, RequiredParliamentBodyV1, BodyElectionAttemptId, u8),
{
    assert!(
        enact_at_height > 9,
        "fixture enactment must follow the complete reducer transcript"
    );
    candidates.sort_unstable();
    candidates.dedup();
    assert!(candidates.len() >= 3, "fixture requires three candidates");
    let proposal_content_id = ProposalContentId::new(proposal.fingerprint());
    let governance_attempt_id = GovernanceAttemptId::derive_v1(proposal_content_id, 0);
    let (risk_tier, requirements) = parliament_attempt_policy_v1(proposal);
    let mut attempt = ParliamentAttemptStateV1::try_new(
        GovernanceAttemptV1 {
            id: governance_attempt_id,
            proposal_content_id,
            sequence: 0,
            risk_tier,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        },
        PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
        1,
        proposal.effect_preimage_hash_v1(),
        GovernanceExpectedHeadV1::Absent(
            iroha_data_model::governance::types::GovernanceExpectedHeadAbsentV1 {
                subject_id: proposal
                    .governed_subject_id_v1()
                    .expect("derive fixture proposal subject"),
            },
        ),
        requirements.clone(),
    )
    .expect("create proposal-bound enacted-attempt fixture");
    attempt
        .complete_qualification(governance_attempt_id)
        .expect("complete enacted-attempt fixture qualification");
    let candidate_count = u32::try_from(candidates.len()).expect("candidate count fits u32");
    let sortition_session = BeaconSessionId::new([0xB0; 32]);
    let mut request_ids = Vec::with_capacity(requirements.len());
    for requirement in &requirements {
        let election_attempt_id =
            BodyElectionAttemptId::derive_v1(governance_attempt_id, requirement.body, 0);
        let request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            requirement.body,
            parliament_candidate_root_v1(governance_attempt_id, requirement.body, &candidates),
            candidate_count,
            3,
            1,
            2,
            sortition_session,
            None,
        )
        .expect("construct enacted-attempt fixture sortition request");
        request_ids.push(request.id);
        attempt
            .register_sortition_request(governance_attempt_id, 0, request, candidates.clone())
            .expect("register enacted-attempt fixture sortition request");
    }
    request_ids.sort_unstable();
    let sortition_pulse_id = BeaconPulseId::new([0xB1; 32]);
    attempt
        .consume_sortition_pulse_batch(
            governance_attempt_id,
            request_ids,
            sortition_session,
            2,
            sortition_pulse_id,
            *sortition_pulse_id.as_bytes(),
            network_id,
            &enacted_fixture_governance(&requirements),
        )
        .expect("consume enacted-attempt fixture sortition pulse");
    for (index, requirement) in requirements.iter().copied().enumerate() {
        complete_body(
            &mut attempt,
            requirement,
            BodyElectionAttemptId::derive_v1(governance_attempt_id, requirement.body, 0),
            0xC0_u8
                .checked_add(u8::try_from(index).expect("body index fits u8"))
                .expect("result tag does not overflow"),
        );
    }
    attempt
        .construct_certificate(governance_attempt_id, enact_at_height - 1, enact_at_height)
        .expect("construct enacted-attempt fixture certificate");
    attempt
        .mark_enacted(governance_attempt_id, enact_at_height)
        .expect("mark enacted-attempt fixture enacted");
    attempt
        .validate_proposal_bindings_v1(proposal)
        .expect("enacted-attempt fixture retains exact proposal bindings");
    attempt
}

/// Build one complete, proposal-bound enacted Parliament attempt for integration fixtures.
///
/// This helper is available only to Core's explicit test corridor. It deliberately exercises the
/// reducer instead of manufacturing certificate-only compatibility state.
#[cfg(any(test, feature = "iroha-core-tests"))]
#[doc(hidden)]
pub fn enacted_parliament_attempt_for_testing(
    proposal: &ProposalKind,
    candidates: Vec<AccountId>,
    network_id: &NetworkId,
    enact_at_height: u64,
) -> ParliamentAttemptStateV1 {
    build_enacted_parliament_attempt_for_testing(
        proposal,
        candidates,
        network_id,
        enact_at_height,
        complete_enacted_fixture_body,
    )
}
