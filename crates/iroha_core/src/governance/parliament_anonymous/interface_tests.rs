//! The anonymous family's attempt-creation and transition interfaces executed against World
//! (design §8.1, §9 and §12.3): what Core derives at creation, the admission cap, the retry
//! rule, and the account-signed facts and intents with their authorities.

use std::num::NonZeroU64;

use iroha_data_model::{
    account::AccountId,
    block::BlockHeader,
    governance::{
        anonymous_ballot::{
            AnonymousParliamentTransitionV1 as Transition, CreateAnonymousParliamentAttemptV1,
            ParliamentBallotParametersV1, ParliamentCandidateTestingPolicyV1,
            SubmitAnonymousParliamentTransitionV1, digests::proposal_digest_v1,
        },
        types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ProposalKind,
            RiskTierV1,
        },
    },
    isi::governance::{
        ParliamentEscalateRiskV1, ParliamentInvitationDecisionV1,
        ParliamentRecordInvitationResponseV1,
    },
    parameter::Parameter,
    permission::Permission,
};
use iroha_executor_data_model::permission::governance::CanManageParliament;
use iroha_model_base::chain::ChainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;

use super::{
    AnonymousLifecycleItemV1,
    test_driver::{Driver, candidate, network},
};
use crate::{
    runtime::catalog::additions::PARLIAMENT_ANONYMOUS,
    smartcontracts::Execute,
    state::{CitizenshipRecord, State, StateTransaction, World},
};

/// Height and time of the block every creation test executes in.
const HEIGHT: u64 = 5;
const NOW_MS: u64 = 1_700_000;

/// A World on which the anonymous family is active, optionally with the candidate-testing
/// selection and a committed admission cap.
fn world(candidate_selected: bool, limit: Option<u16>) -> World {
    let world = World::new();
    crate::runtime::activate_additions_for_testing(&world, PARLIAMENT_ANONYMOUS.module)
        .expect("activate the anonymous family");
    let mut block = world.block();
    if candidate_selected {
        block
            .set_parameter_for_testing(Parameter::Custom(
                ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
                    .to_custom_parameter()
                    .expect("candidate policy"),
            ))
            .expect("select candidate testing");
    }
    if let Some(limit) = limit {
        let ballot = ParliamentBallotParametersV1 {
            max_nonterminal_attempts: limit,
            ..ParliamentBallotParametersV1::default()
        };
        block
            .set_parameter_for_testing(Parameter::Custom(
                ballot.to_custom_parameter().expect("ballot parameters"),
            ))
            .expect("commit the ballot parameters");
    }
    block.commit();
    world
}

fn state(world: World) -> State {
    State::new_with_chain_and_network_id_for_testing(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        ChainId::from("parliament-anonymous-interface-tests"),
        network(),
    )
}

fn header(height: u64, timestamp_ms: u64) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero height"),
        None,
        None,
        timestamp_ms,
        0,
    )
}

/// A contract-deployment proposal, distinct for every `byte`.
fn proposal(byte: u8) -> ProposalKind {
    ProposalKind::DeployContract(DeployContractProposal {
        proposal_operator: ALICE_ID.clone(),
        contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
            .parse()
            .expect("contract address"),
        code_hash: ContractCodeHash::new([byte; 32]),
        abi_hash: ContractAbiHash::new([0x41; 32]),
        abi_version: AbiVersion::new(1),
        manifest_provenance: None,
    })
}

fn create(byte: u8, attempt_sequence: u32) -> CreateAnonymousParliamentAttemptV1 {
    CreateAnonymousParliamentAttemptV1 {
        proposal: proposal(byte),
        attempt_sequence,
    }
}

/// Execute `instruction` as `authority`, returning the refusal's text.
fn run<I: Execute>(
    tx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: I,
) -> Result<(), String> {
    instruction
        .execute(authority, tx)
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn creation_records_what_core_derives_and_nothing_the_caller_chose() {
    let state = state(world(true, None));
    let mut block = state.block(header(HEIGHT, NOW_MS));
    let mut tx = block.transaction();
    crate::governance::parliament::seed_npos_sortition_cadence_for_tests(&mut tx, 100);
    let instruction = create(0x31, 0);
    run(&mut tx, &ALICE_ID, instruction.clone()).expect("any signer creates an attempt");

    let stored = tx
        .world
        .parliament_anonymous_attempts
        .get(&instruction.governance_attempt_id())
        .expect("the attempt is stored under the family's identifier")
        .clone();
    assert_eq!(stored.purpose(), &candidate());
    assert_eq!(
        stored.attempt(),
        &instruction.canonical_attempt(RiskTierV1::Standard)
    );
    assert_eq!(
        stored.proposal_digest(),
        &proposal_digest_v1(&instruction.proposal).expect("digest")
    );
    assert_eq!(
        stored.effect_preimage_hash(),
        instruction.proposal.effect_preimage_hash_v1()
    );
    assert_eq!(stored.randomness_redraws_before_attempt(), 0);
    // The binding pipeline ends in the conditional Confirmation Jury.
    let bodies: Vec<_> = stored
        .policy()
        .required_bodies
        .iter()
        .map(|required| required.body)
        .collect();
    assert_eq!(
        bodies.last(),
        Some(&iroha_data_model::governance::types::ParliamentBody::ConfirmationJury)
    );
    assert!(bodies.contains(&iroha_data_model::governance::types::ParliamentBody::PolicyJury));
    assert!(stored.policy().binding.is_some());
    assert_eq!(stored.qualification().created_at_ms(), NOW_MS);
    // `StartQualification` is due at the creating block's time.
    let items = stored.lifecycle_items().expect("items");
    assert_eq!(items[0].item, AnonymousLifecycleItemV1::StartQualification);
    assert_eq!(items[0].due_ms, NOW_MS);
    stored
        .validate(&network())
        .expect("the stored attempt restores");
}

#[test]
fn creation_without_a_candidate_selection_or_with_a_bad_retry_stores_nothing() {
    // An unqualified release on a network whose genesis selected no candidate testing.
    let state = state(world(false, None));
    let mut block = state.block(header(HEIGHT, NOW_MS));
    let mut tx = block.transaction();
    crate::governance::parliament::seed_npos_sortition_cadence_for_tests(&mut tx, 100);
    let error = run(&mut tx, &ALICE_ID, create(0x31, 0)).expect_err("no purpose");
    assert!(
        error.contains("no anonymous Parliament attempt may be created"),
        "{error}"
    );
    assert_eq!(tx.world.parliament_anonymous_attempts.iter().count(), 0);
    drop(tx);
    drop(block);

    let state = self::state(world(true, None));
    let mut block = state.block(header(HEIGHT, NOW_MS));
    let mut tx = block.transaction();
    crate::governance::parliament::seed_npos_sortition_cadence_for_tests(&mut tx, 100);
    // A retry needs its exact predecessor.
    let error = run(&mut tx, &ALICE_ID, create(0x31, 1)).expect_err("no predecessor");
    assert!(error.contains("requires its exact predecessor"), "{error}");
    run(&mut tx, &ALICE_ID, create(0x31, 0)).expect("first attempt");
    // The same attempt again.
    let error = run(&mut tx, &ALICE_ID, create(0x31, 0)).expect_err("duplicate");
    assert!(error.contains("already exists"), "{error}");
    // A retry while its predecessor is still active.
    let error = run(&mut tx, &ALICE_ID, create(0x31, 1)).expect_err("active predecessor");
    assert!(
        error.contains("terminal non-enacted predecessor"),
        "{error}"
    );
    assert_eq!(tx.world.parliament_anonymous_attempts.iter().count(), 1);
}

#[test]
fn the_admission_cap_refuses_an_attempt_beyond_k_nonterminal_ones() {
    let state = state(world(true, Some(2)));
    let mut block = state.block(header(HEIGHT, NOW_MS));
    let mut tx = block.transaction();
    crate::governance::parliament::seed_npos_sortition_cadence_for_tests(&mut tx, 100);
    run(&mut tx, &ALICE_ID, create(0x31, 0)).expect("first of two");
    run(&mut tx, &ALICE_ID, create(0x32, 0)).expect("second of two");
    let error = run(&mut tx, &ALICE_ID, create(0x33, 0)).expect_err("beyond the cap");
    assert!(error.contains("admission cap of 2"), "{error}");
    assert_eq!(tx.world.parliament_anonymous_attempts.iter().count(), 2);
    assert_eq!(
        crate::state::parliament_anonymous_tables::nonterminal_attempts(
            &tx.world.parliament_anonymous_attempts
        ),
        2
    );
}

/// A World holding `driver`'s attempt, with every driver citizen bonded at the policy's
/// minimum and one more citizen bonded below it.
fn world_with(driver: &Driver) -> (World, AccountId) {
    let world = world(true, None);
    let minimum = driver
        .state
        .policy()
        .sortition_policy
        .minimum_citizenship_bond
        .clone();
    let count = u32::try_from(driver.citizens.len()).expect("few citizens") + 1;
    let below = super::test_driver::citizens(count)
        .into_iter()
        .find(|account| !driver.citizens.contains(account))
        .expect("one more citizen");
    let mut block = world.block();
    block
        .parliament_anonymous_attempts
        .insert(driver.id(), driver.state.clone());
    for citizen in &driver.citizens {
        block.citizens.insert(
            citizen.clone(),
            CitizenshipRecord::new(citizen.clone(), minimum.clone(), 1),
        );
    }
    let short = minimum
        .clone()
        .try_sub(&Quantity::from(1_u64))
        .expect("a positive minimum bond");
    block.citizens.insert(
        below.clone(),
        CitizenshipRecord::new(below.clone(), short, 1),
    );
    block.commit();
    (world, below)
}

fn submit(driver: &Driver, transition: Transition) -> SubmitAnonymousParliamentTransitionV1 {
    SubmitAnonymousParliamentTransitionV1 {
        governance_attempt_id: driver.id(),
        transition,
    }
}

#[test]
fn transitions_record_facts_and_intents_under_their_authorities() {
    let mut driver = Driver::new(RiskTierV1::Standard, 40);
    driver.qualify();
    let (world, below) = world_with(&driver);
    let state = state(world);
    let height = driver.height + 1;
    let now_ms = driver.now_ms + 1;
    let mut block = state.block(header(height, now_ms));
    let mut tx = block.transaction();

    // Risk escalation is a manager's intent, and only before the tier locks.
    let escalate = submit(
        &driver,
        Transition::EscalateRisk(ParliamentEscalateRiskV1 {
            target: RiskTierV1::Constitutional,
        }),
    );
    let error = run(&mut tx, &ALICE_ID, escalate.clone()).expect_err("no permission");
    assert!(error.contains("CanManageParliament"), "{error}");
    tx.world.account_permissions.insert(
        ALICE_ID.clone(),
        std::collections::BTreeSet::from([Permission::from(CanManageParliament)]),
    );
    run(&mut tx, &ALICE_ID, escalate).expect("the manager escalates the tier");
    let stored = |tx: &StateTransaction<'_, '_>| {
        tx.world
            .parliament_anonymous_attempts
            .get(&driver.id())
            .expect("stored attempt")
            .clone()
    };
    assert_eq!(stored(&tx).attempt().risk_tier, RiskTierV1::Constitutional);

    // The initial registration is permissionless and derives its snapshot from committed
    // State: every citizen at or above the frozen bond, never the one below it.
    let mut expected = stored(&tx);
    let mut candidates = driver.citizens.clone();
    candidates.sort_unstable();
    expected
        .register_initial_sortition(driver.id(), &network(), height, now_ms, candidates)
        .expect("the reducer registers");
    run(
        &mut tx,
        &below,
        submit(&driver, Transition::RegisterInitialSortition),
    )
    .expect("any signer registers the initial generation");
    assert_eq!(stored(&tx), expected);
    let error = run(
        &mut tx,
        &ALICE_ID,
        submit(
            &driver,
            Transition::EscalateRisk(ParliamentEscalateRiskV1 {
                target: RiskTierV1::Emergency,
            }),
        ),
    )
    .expect_err("the registration locked the tier");
    assert!(error.contains("refused"), "{error}");

    // After the draw, an invited member answers its own invitation of a public body.
    driver.state = stored(&tx);
    drop(tx);
    drop(block);
    driver.height = height;
    driver.now_ms = now_ms;
    driver.advance_to_slot();
    let body = driver.state.policy().required_bodies[0].body;
    let (generation, election) = driver
        .state
        .active_election(body)
        .expect("the first body's election");
    let election = &driver.state.generations()[generation].elections()[election];
    let election_attempt_id = election.id();
    let member = election.primaries()[0].member.clone();
    let mut block = state.block(header(driver.height + 1, driver.now_ms + 1));
    let mut tx = block.transaction();
    tx.world
        .parliament_anonymous_attempts
        .insert(driver.id(), driver.state.clone());
    let response = submit(
        &driver,
        Transition::RecordInvitationResponse(ParliamentRecordInvitationResponseV1 {
            election_attempt_id,
            body,
            decision: ParliamentInvitationDecisionV1::Accept,
        }),
    );
    let error = run(&mut tx, &below, response.clone()).expect_err("not this signer's seat");
    assert!(error.contains("refused"), "{error}");
    run(&mut tx, &member, response.clone()).expect("the invited member accepts");
    let error = run(&mut tx, &member, response).expect_err("one response per assignment");
    assert!(error.contains("refused"), "{error}");

    // A binding seat is never accepted by a bare response.
    let jury = iroha_data_model::governance::types::ParliamentBody::PolicyJury;
    let error = run(
        &mut tx,
        &member,
        submit(
            &driver,
            Transition::RecordInvitationResponse(ParliamentRecordInvitationResponseV1 {
                election_attempt_id,
                body: jury,
                decision: ParliamentInvitationDecisionV1::Accept,
            }),
        ),
    )
    .expect_err("a binding seat needs its credential");
    assert!(error.contains("credential"), "{error}");
}

#[test]
fn an_unknown_attempt_and_an_early_close_without_permission_are_refused() {
    let driver = Driver::new(RiskTierV1::Standard, 40);
    let (world, _) = world_with(&driver);
    let state = state(world);
    let mut block = state.block(header(driver.height + 1, driver.now_ms + 1));
    let mut tx = block.transaction();
    let unknown = SubmitAnonymousParliamentTransitionV1 {
        governance_attempt_id: iroha_data_model::governance::types::GovernanceAttemptId::new(
            [0x99; 32],
        ),
        transition: Transition::RegisterInitialSortition,
    };
    let error = run(&mut tx, &ALICE_ID, unknown).expect_err("unknown attempt");
    assert!(
        error.contains("unknown anonymous Parliament attempt"),
        "{error}"
    );
    let error = run(
        &mut tx,
        &ALICE_ID,
        submit(&driver, Transition::CloseQualificationEarly),
    )
    .expect_err("a manager's intent");
    assert!(error.contains("CanManageParliament"), "{error}");
    assert_eq!(
        tx.world
            .parliament_anonymous_attempts
            .get(&driver.id())
            .expect("unchanged attempt"),
        &driver.state
    );
}
