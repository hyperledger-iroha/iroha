// Execute-level coverage of consensus-derived initial sortition. These fixtures
// use the real persisted attempt reducer and ordinary manager authorization.

fn seed_parliament_initial_sortition_fixture(
    state_transaction: &mut StateTransaction<'_, '_>,
    qualified: bool,
    pinned_delay: u64,
) -> (GovernanceAttemptId, Vec<RequiredParliamentBodyV1>) {
    let proposal = parliament_permissionless_progress_proposal();
    let proposal_id = proposal.fingerprint();
    let proposal_content_id = ProposalContentId::new(proposal_id);
    let governance_attempt_id = GovernanceAttemptId::derive_v1(proposal_content_id, 0);
    let (risk_tier, requirements) = parliament_attempt_policy_v1(&proposal);
    let mut governance = parliament_test_governance(&requirements);
    governance.citizenship_bond_amount = Quantity::from(10_u64);
    governance.parliament_sortition_pulse_delay_blocks = pinned_delay;
    state_transaction.gov = governance;
    state_transaction.world.account_permissions.insert(
        ALICE_ID.clone(),
        BTreeSet::from([Permission::from(CanManageParliament)]),
    );
    let expected_head = parliament_expected_head_v1(&proposal, state_transaction)
        .expect("derive the actual proposal subject head");
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
        pinned_delay,
        proposal.effect_preimage_hash_v1(),
        expected_head,
        requirements.clone(),
    )
    .expect("create initial-sortition attempt");
    if qualified {
        attempt
            .complete_qualification(governance_attempt_id)
            .expect("complete qualification");
    }
    state_transaction
        .world
        .put_governance_proposal(
            proposal_id,
            crate::state::GovernanceProposalRecord {
                proposer: ALICE_ID.clone(),
                kind: proposal,
                created_height: 1,
                status: crate::state::GovernanceProposalStatus::Proposed,
            },
        )
        .expect("store proposal");
    state_transaction
        .world
        .put_parliament_attempt(attempt)
        .expect("store attempt");
    (governance_attempt_id, requirements)
}

fn insert_parliament_initial_sortition_citizen(
    state_transaction: &mut StateTransaction<'_, '_>,
    candidate: &AccountId,
    bond: u64,
) {
    state_transaction.world.citizens.insert(
        candidate.clone(),
        crate::state::CitizenshipRecord {
            owner: candidate.clone(),
            amount: Quantity::from(bond),
            bonded_height: 0,
        },
    );
}

fn initial_parliament_sortition_intent(
    governance_attempt_id: GovernanceAttemptId,
) -> gov::SubmitParliamentLifecycleTransitionV1 {
    gov::SubmitParliamentLifecycleTransitionV1 {
        governance_attempt_id,
        transition: gov::ParliamentLifecycleTransitionV1::RegisterInitialSortition,
    }
}

#[test]
fn parliament_initial_sortition_derives_all_bodies_from_one_bond_filtered_snapshot() {
    let state = blank_test_state();
    let mut block = state.block(first_test_block_header());
    let mut state_transaction = block.transaction();
    let (governance_attempt_id, requirements) =
        seed_parliament_initial_sortition_fixture(&mut state_transaction, true, 5);
    let candidates = parliament_test_candidates();
    // Different insertion order and exact-threshold/over-threshold bonds must
    // produce the same complete canonical account ordering for every body.
    for (index, candidate) in candidates.iter().rev().enumerate() {
        insert_parliament_initial_sortition_citizen(
            &mut state_transaction,
            candidate,
            if index % 2 == 0 { 10 } else { 11 },
        );
    }
    let underbonded = parliament_test_account(200);
    insert_parliament_initial_sortition_citizen(&mut state_transaction, &underbonded, 9);
    state_transaction.gov.rules_committee_size = 4;
    state_transaction.gov.agenda_council_size = 5;
    state_transaction.gov.policy_jury_size = 6;
    // An attempt pins its delay; subsequent global configuration cannot move
    // that attempt's future pulse while it waits for initial registration.
    state_transaction
        .gov
        .parliament_sortition_pulse_delay_blocks = 99;
    let request_height = state_transaction.block_height();
    let pulse_height = request_height + 5;
    let beacon_session_id = BeaconSessionId::for_network_v1(&state_transaction.network_id);

    initial_parliament_sortition_intent(governance_attempt_id)
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("qualified manager can request the canonical initial generation");

    let attempt = state_transaction
        .world
        .parliament_attempts
        .get(&governance_attempt_id)
        .expect("persisted initial generation");
    assert_eq!(attempt.required_bodies(), requirements.as_slice());
    for required in &requirements {
        assert_ne!(required.body, ParliamentBody::ConfirmationJury);
        let election_id = BodyElectionAttemptId::derive_v1(governance_attempt_id, required.body, 0);
        let election = attempt.election(&election_id).expect("every initial body");
        let native = election.attempt();
        let request = native.request;
        assert_eq!(native.id, election_id);
        assert_eq!(native.sequence, 0);
        assert_eq!(native.status, BodyElectionAttemptStatusV1::AwaitingPulse);
        assert_eq!(request.body, required.body);
        assert_eq!(request.governance_attempt_id, governance_attempt_id);
        assert_eq!(request.body_election_attempt_id, election_id);
        assert_eq!(request.candidate_count, 24);
        assert_eq!(
            request.candidate_root,
            parliament_candidate_root_v1(governance_attempt_id, required.body, &candidates)
        );
        assert_eq!(request.request_height, request_height);
        assert_eq!(request.pulse_height, pulse_height);
        assert_eq!(request.beacon_session_id, beacon_session_id);
        assert_eq!(request.id, request.canonical_id());
        assert_eq!(
            request.target_seats,
            match required.body {
                ParliamentBody::RulesCommittee => 4,
                ParliamentBody::AgendaCouncil => 5,
                ParliamentBody::PolicyJury => 6,
                _ => 3,
            }
        );
        assert!(election.pulse_id().is_none());
        assert!(attempt.sortition_capacity_failure(&election_id).is_none());
    }
    assert!(
        attempt
            .election(&BodyElectionAttemptId::derive_v1(
                governance_attempt_id,
                ParliamentBody::ConfirmationJury,
                0,
            ))
            .is_none()
    );
    assert!(attempt.requires_beacon_pulse_at(beacon_session_id, pulse_height));
    attempt
        .validate()
        .expect("persisted generation survives reducer validation");
}

#[test]
fn parliament_initial_sortition_requires_manager_and_completed_qualification() {
    let state = blank_test_state();
    let mut block = state.block(first_test_block_header());
    let mut state_transaction = block.transaction();
    let (governance_attempt_id, _) =
        seed_parliament_initial_sortition_fixture(&mut state_transaction, false, 5);
    let before = state_transaction
        .world
        .parliament_attempts
        .get(&governance_attempt_id)
        .expect("fixture attempt")
        .clone();
    let unauthorized = initial_parliament_sortition_intent(governance_attempt_id)
        .execute(&BOB_ID, &mut state_transaction)
        .expect_err("an ordinary account cannot set sortition intent");
    assert!(format!("{unauthorized:?}").contains("CanManageParliament"));
    assert_eq!(
        state_transaction
            .world
            .parliament_attempts
            .get(&governance_attempt_id),
        Some(&before)
    );
    initial_parliament_sortition_intent(governance_attempt_id)
        .execute(&ALICE_ID, &mut state_transaction)
        .expect_err("manager must complete qualification first");
    assert_eq!(
        state_transaction
            .world
            .parliament_attempts
            .get(&governance_attempt_id),
        Some(&before)
    );
    gov::SubmitParliamentLifecycleTransitionV1 {
        governance_attempt_id,
        transition: gov::ParliamentLifecycleTransitionV1::CompleteQualification,
    }
    .execute(&ALICE_ID, &mut state_transaction)
    .expect("manager completes qualification using the actual instruction");
    initial_parliament_sortition_intent(governance_attempt_id)
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("qualified empty electorate records capacity evidence");
}

#[test]
fn parliament_initial_sortition_replay_cannot_replace_the_frozen_electorate() {
    let state = blank_test_state();
    let mut block = state.block(first_test_block_header());
    let mut state_transaction = block.transaction();
    let (governance_attempt_id, _) =
        seed_parliament_initial_sortition_fixture(&mut state_transaction, true, 5);
    for candidate in parliament_test_candidates() {
        insert_parliament_initial_sortition_citizen(&mut state_transaction, &candidate, 10);
    }
    initial_parliament_sortition_intent(governance_attempt_id)
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("initial generation");
    let before = state_transaction
        .world
        .parliament_attempts
        .get(&governance_attempt_id)
        .expect("frozen generation")
        .clone();
    // Both registry growth and a different live eligibility policy would alter
    // a recomputed snapshot. Neither authorizes an implicit generation retry.
    insert_parliament_initial_sortition_citizen(
        &mut state_transaction,
        &parliament_test_account(201),
        20,
    );
    state_transaction.gov.citizenship_bond_amount = Quantity::from(11_u64);
    initial_parliament_sortition_intent(governance_attempt_id)
        .execute(&ALICE_ID, &mut state_transaction)
        .expect_err("a repeated intent must not redraw an existing generation");
    assert_eq!(
        state_transaction
            .world
            .parliament_attempts
            .get(&governance_attempt_id),
        Some(&before)
    );
    before
        .validate()
        .expect("rejected replay preserves valid frozen state");
}

#[test]
fn parliament_initial_sortition_preserves_all_sub_anonymity_capacity_evidence() {
    for candidate_count in 0..usize::try_from(
        iroha_data_model::governance::types::MIN_PARLIAMENT_HIDDEN_BALLOT_ANONYMITY_V1,
    )
    .expect("the protocol anonymity floor fits usize")
    {
        let state = blank_test_state();
        let mut block = state.block(first_test_block_header());
        let mut state_transaction = block.transaction();
        let (governance_attempt_id, requirements) =
            seed_parliament_initial_sortition_fixture(&mut state_transaction, true, 5);
        for candidate in parliament_test_candidates().iter().take(candidate_count) {
            insert_parliament_initial_sortition_citizen(&mut state_transaction, candidate, 10);
        }
        let request_height = state_transaction.block_height();
        let beacon_session_id = BeaconSessionId::for_network_v1(&state_transaction.network_id);
        initial_parliament_sortition_intent(governance_attempt_id)
            .execute(&ALICE_ID, &mut state_transaction)
            .expect("insufficient population is typed evidence, never a hidden-ballot waiver");
        let before = state_transaction
            .world
            .parliament_attempts
            .get(&governance_attempt_id)
            .expect("capacity generation")
            .clone();
        for required in &requirements {
            let election_id =
                BodyElectionAttemptId::derive_v1(governance_attempt_id, required.body, 0);
            let failure = before
                .sortition_capacity_failure(&election_id)
                .expect("complete atomic capacity generation");
            assert_eq!(failure.body(), required.body);
            assert_eq!(failure.body_election_attempt_id(), election_id);
            assert_eq!(failure.sequence(), 0);
            assert_eq!(failure.candidate_count(), candidate_count);
            assert_eq!(failure.failure_height(), request_height);
            assert_eq!(failure.status(), BodyElectionAttemptStatusV1::NoRoster);
            assert!(before.election(&election_id).is_none());
        }
        assert!(!before.requires_beacon_pulse_at(beacon_session_id, request_height + 5));
        before
            .validate()
            .expect("capacity evidence survives restore validation");
        for candidate in parliament_test_candidates() {
            insert_parliament_initial_sortition_citizen(&mut state_transaction, &candidate, 10);
        }
        initial_parliament_sortition_intent(governance_attempt_id)
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("new citizens require an explicit retry, not replay of initial intent");
        assert_eq!(
            state_transaction
                .world
                .parliament_attempts
                .get(&governance_attempt_id),
            Some(&before)
        );
    }
}

#[test]
fn parliament_initial_sortition_bad_seat_configuration_rolls_back_the_whole_batch() {
    // Policy Jury is the last initially required body and uses a hidden ballot.
    // One/two seats reach the reducer after earlier valid entries; zero seats
    // fails static validation. No path may retain a partial generation.
    for target_seats in [0_usize, 1, 2] {
        let state = blank_test_state();
        let mut block = state.block(first_test_block_header());
        let mut state_transaction = block.transaction();
        let (governance_attempt_id, requirements) =
            seed_parliament_initial_sortition_fixture(&mut state_transaction, true, 5);
        assert_eq!(
            requirements.last().expect("nonempty pipeline").body,
            ParliamentBody::PolicyJury
        );
        for candidate in parliament_test_candidates() {
            insert_parliament_initial_sortition_citizen(&mut state_transaction, &candidate, 10);
        }
        state_transaction.gov.policy_jury_size = target_seats;
        let before = state_transaction
            .world
            .parliament_attempts
            .get(&governance_attempt_id)
            .expect("qualified attempt")
            .clone();
        initial_parliament_sortition_intent(governance_attempt_id)
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("invalid hidden-body size must reject the entire generation");
        assert_eq!(
            state_transaction
                .world
                .parliament_attempts
                .get(&governance_attempt_id),
            Some(&before)
        );
        before
            .validate()
            .expect("failed generation preserves valid attempt");
    }
}

#[test]
fn parliament_initial_sortition_pulse_overflow_cannot_mutate_the_attempt() {
    let state = blank_test_state();
    let mut block = state.block(first_test_block_header());
    let mut state_transaction = block.transaction();
    let (governance_attempt_id, _) =
        seed_parliament_initial_sortition_fixture(&mut state_transaction, true, u64::MAX);
    let before = state_transaction
        .world
        .parliament_attempts
        .get(&governance_attempt_id)
        .expect("qualified attempt")
        .clone();
    let error = initial_parliament_sortition_intent(governance_attempt_id)
        .execute(&ALICE_ID, &mut state_transaction)
        .expect_err("containing height plus pinned delay must not wrap");
    assert!(format!("{error:?}").contains("pulse height overflow"));
    assert_eq!(
        state_transaction
            .world
            .parliament_attempts
            .get(&governance_attempt_id),
        Some(&before)
    );
}
