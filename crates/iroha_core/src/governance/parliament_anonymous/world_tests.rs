//! The anonymous family in World (design §12.3, slice 2): `RegisterParliamentCredentialV1`
//! executes against the three tables from its direct signed original, and restore checks
//! what they keep. Genuine registration proofs come from the credential tests' cache, except
//! for the certified chain's, which binds that chain's own network.

use std::num::NonZeroU64;

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    block::BlockHeader,
    governance::{
        anonymous_ballot::{
            BallotScopeV1, ParliamentBallotBoardRecordV1, ParliamentCandidateTestingPolicyV1,
            ParliamentCredentialRecordV1, ParliamentCredentialSourceV1,
            RegisterParliamentCredentialV1, RegistrationAuthorizationV1,
        },
        types::{AssignmentId, GovernanceAttemptId},
    },
    isi::InstructionBox,
    parameter::Parameter,
    transaction::{Executable, FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_model_base::chain::ChainId;
use iroha_parliament_ballot::{
    board::{BallotBoardV1, SealErrorV1},
    context::ContextErrorV1,
    inputs::ProofInputErrorV1,
    wallet::{CredentialHandleV1, prepare_registration_v1},
};
use mv::storage::StorageReadOnly;

use super::{
    AnonymousParliamentErrorV1,
    credentials_tests::{
        JURY, answer_public_bodies, close_invitations, drawn_small_jury, input, jury_effect,
        jury_invitations, register, registration,
    },
    is_binding,
    test_driver::{Driver, network},
};
use crate::{
    runtime::catalog::additions::PARLIAMENT_ANONYMOUS,
    smartcontracts::{Execute, isi::parliament_anonymous::anonymous_refusal},
    state::{
        State, StateTransaction, World, WorldReadOnly,
        parliament_anonymous_tables::{AnonymousTablesRestoreError, validate_cut},
    },
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};

/// The key of the driver citizen `member`.
fn citizen_key(member: &AccountId) -> KeyPair {
    (0..1024_u32)
        .map(|index| {
            let mut seed = vec![0x77; 32];
            seed[..4].copy_from_slice(&index.to_be_bytes());
            KeyPair::try_from_seed(seed, Algorithm::Ed25519).expect("citizen key")
        })
        .find(|key| AccountId::new(key.public_key().clone()) == *member)
        .expect("a driver citizen")
}

/// A World on which the anonymous family is active and candidate testing is selected,
/// keeping `driver`'s attempt.
fn world_with(driver: &Driver) -> World {
    let world = World::new();
    crate::runtime::activate_additions_for_testing(&world, PARLIAMENT_ANONYMOUS.module)
        .expect("activate the anonymous family");
    let mut block = world.block();
    block
        .set_parameter_for_testing(Parameter::Custom(
            ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
                .to_custom_parameter()
                .expect("candidate policy"),
        ))
        .expect("select candidate testing");
    block
        .parliament_anonymous_attempts
        .insert(driver.id(), driver.state.clone());
    block.commit();
    world
}

/// A State on the driver's network over [`world_with`].
fn state_with(driver: &Driver) -> State {
    State::new_with_chain_and_network_id_for_testing(
        world_with(driver),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        ChainId::from("parliament-anonymous-world-tests"),
        network(),
    )
}

/// `member`'s direct signed input carrying `input` alone.
fn signed(member: &AccountId, input: RegisterParliamentCredentialV1) -> SignedTransaction {
    TransactionBuilder::new(
        network(),
        member.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(input)])
    .sign(citizen_key(member).private_key())
}

/// Arm `signed` as the executor does right before its sole instruction, at input 3.
fn arm(tx: &mut StateTransaction<'_, '_>, signed: &SignedTransaction) {
    tx.current_tx_hash = Some(signed.hash());
    tx.tx_call_hash = Some(iroha_crypto::Hash::from(signed.hash_as_entrypoint()));
    tx.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
    tx.current_entrypoint_index = Some(3);
    let Executable::Instructions(instructions) = signed.instructions() else {
        unreachable!("a registration is an instruction executable")
    };
    tx.arm_direct_parliament_registration(signed, signed.authority(), &instructions[0], 0, true);
}

/// The block after the driver's, at the driver's time: the jury's invitations are open.
fn header(driver: &Driver) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(driver.height + 1).expect("nonzero height"),
        None,
        None,
        driver.now_ms,
        0,
    )
}

#[test]
fn a_registration_records_the_acceptance_and_the_credential_together() {
    let driver = drawn_small_jury();
    let (election, invited) = jury_invitations(&driver);
    let (assignment, member) = invited[0].clone();
    let state = state_with(&driver);
    let first: RegisterParliamentCredentialV1 = input(&driver, 0);
    let original = signed(&member, first.clone());
    let mut block = state.block(header(&driver));
    let mut tx = block.transaction();

    // Without its one-use original the instruction is refused and changes nothing.
    assert!(first.clone().execute(&member, &mut tx).is_err());
    assert!(!tx.is_execution_deferred(), "a refusal is a verdict");
    assert!(
        tx.world
            .parliament_anonymous_registrations
            .iter()
            .next()
            .is_none()
    );

    arm(&mut tx, &original);
    first
        .execute(&member, &mut tx)
        .expect("the invited seat registers its credential");
    let record = *tx
        .world
        .parliament_anonymous_registrations
        .get(&assignment)
        .expect("the credential is stored");
    assert_eq!(record.commitment, registration(0).commitment);
    assert_eq!(record.authorization, registration(0).authorization);
    assert_eq!(
        record.source,
        ParliamentCredentialSourceV1 {
            height: driver.height + 1,
            input_index: 3,
            instruction_index: 0,
            entrypoint_hash: original.hash_as_entrypoint(),
        }
    );
    let accepted = |tx: &StateTransaction<'_, '_>| {
        tx.world
            .parliament_anonymous_attempts
            .get(&driver.id())
            .expect("the attempt")
            .generations()
            .iter()
            .flat_map(|generation| generation.elections())
            .find(|candidate| candidate.id() == election)
            .expect("the jury's election")
            .accepted()
            .clone()
    };
    assert_eq!(accepted(&tx).into_iter().collect::<Vec<_>>(), [assignment]);

    // The same registration again is refused: the response is recorded.
    arm(&mut tx, &original);
    assert!(input(&driver, 0).execute(&member, &mut tx).is_err());
    assert!(
        !tx.is_execution_deferred(),
        "a recorded response is a verdict"
    );
    assert_eq!(
        tx.world.parliament_anonymous_registrations.iter().count(),
        1
    );

    // Another seat cannot register the first seat's credential: its commitment is taken.
    let (other_assignment, other_member) = invited[1].clone();
    let copied = input(&driver, 0);
    arm(&mut tx, &signed(&other_member, copied.clone()));
    assert!(copied.execute(&other_member, &mut tx).is_err());
    // An authorization that is not the value the proof proves is refused.
    let mut forged = input(&driver, 1);
    forged.registration.authorization = RegistrationAuthorizationV1::new([0xa1; 64]);
    arm(&mut tx, &signed(&other_member, forged.clone()));
    assert!(forged.execute(&other_member, &mut tx).is_err());
    assert!(
        !tx.is_execution_deferred(),
        "a proof that does not verify is a verdict"
    );
    assert_eq!(
        tx.world.parliament_anonymous_registrations.iter().count(),
        1
    );
    assert_eq!(
        accepted(&tx).into_iter().collect::<Vec<_>>(),
        [assignment],
        "a refused registration accepts nothing"
    );

    // The seat's own genuine registration is accepted.
    arm(&mut tx, &signed(&other_member, input(&driver, 1)));
    input(&driver, 1)
        .execute(&other_member, &mut tx)
        .expect("the second seat registers its credential");
    assert_eq!(accepted(&tx).len(), 2);
    assert!(
        tx.world
            .parliament_anonymous_registrations
            .get(&other_assignment)
            .is_some()
    );
}

/// A World whose selection installs the anonymous family's tables.
fn activated_world() -> World {
    let world = World::new();
    crate::runtime::activate_additions_for_testing(&world, PARLIAMENT_ANONYMOUS.module)
        .expect("activate the anonymous family");
    world
}

/// Write `driver`'s attempt, the credentials `records` and `boards` into `world`.
fn fill(
    world: &World,
    driver: &Driver,
    records: &[(AssignmentId, ParliamentCredentialRecordV1)],
    boards: &[(BallotScopeV1, ParliamentBallotBoardRecordV1)],
) {
    let mut block = world.block();
    block
        .parliament_anonymous_attempts
        .insert(driver.id(), driver.state.clone());
    for (assignment, record) in records {
        block
            .parliament_anonymous_registrations
            .insert(*assignment, *record);
    }
    for (scope, board) in boards {
        block
            .parliament_anonymous_boards
            .insert(*scope, board.clone());
    }
    block.commit();
}

/// Restore refuses `driver`'s tables with `records` and `boards`.
fn refused(
    driver: &Driver,
    records: &[(AssignmentId, ParliamentCredentialRecordV1)],
    boards: &[(BallotScopeV1, ParliamentBallotBoardRecordV1)],
    what: &str,
) {
    let world = activated_world();
    fill(&world, driver, records, boards);
    assert!(
        matches!(
            validate_cut(&world.view(), &network(), driver.height),
            Err(AnonymousTablesRestoreError::Invalid(_))
        ),
        "{what} is refused"
    );
}

#[test]
fn restore_accepts_exactly_the_tables_the_reducer_produces() {
    let mut driver = drawn_small_jury();
    let mut records = Vec::new();
    register(&mut driver, &mut records, 0..4);
    answer_public_bodies(&mut driver);
    let outcomes = close_invitations(&mut driver, &records);
    let board = jury_effect(&outcomes)
        .board
        .as_ref()
        .expect("the jury's board")
        .board
        .clone();
    let scope = *BallotBoardV1::restore(board.clone())
        .expect("a stored board restores")
        .scope();
    assert!(driver.state.sealed_body(JURY).is_some());

    // The attempt, its four credentials and the jury's board.
    let world = activated_world();
    fill(&world, &driver, &records, &[(scope, board.clone())]);
    validate_cut(&world.view(), &network(), driver.height)
        .expect("the reducer's own tables restore");
    // Even with every credential already in history, this earlier cut did not seal its
    // juries. The cut check must inspect attempt anchors, not only credential locators.
    assert!(matches!(
        validate_cut(&world.view(), &network(), driver.height - 1),
        Err(AnonymousTablesRestoreError::Invalid(message)) if message.contains("height")
    ));
    // A cut below the registrations' blocks did not execute them.
    assert!(matches!(
        validate_cut(&world.view(), &network(), driver.height - 2),
        Err(AnonymousTablesRestoreError::Invalid(_))
    ));
    // A selection that does not install the family holds none of its rows.
    let uninstalled = World::new();
    fill(&uninstalled, &driver, &records, &[(scope, board.clone())]);
    assert!(matches!(
        validate_cut(&uninstalled.view(), &network(), driver.height),
        Err(AnonymousTablesRestoreError::Invalid(_))
    ));

    // On another network the attempt itself is not one the reducer produces.
    let other = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0x5b; 32])),
    );
    assert!(matches!(
        validate_cut(&world.view(), &other, driver.height),
        Err(AnonymousTablesRestoreError::Invalid(_))
    ));

    refused(&driver, &records, &[], "a sealed jury without its board");
    refused(
        &driver,
        &records,
        &[(BallotScopeV1::new([0x7a; 64]), board.clone())],
        "a board under another scope",
    );
    refused(
        &driver,
        &records[1..],
        &[(scope, board.clone())],
        "a lost credential",
    );
    let mut unaccepted = records.clone();
    unaccepted.push((AssignmentId::new([0x99; 32]), records[0].1));
    refused(
        &driver,
        &unaccepted,
        &[(scope, board.clone())],
        "a credential of an assignment no attempt accepted",
    );
    let public_assignment = driver
        .state
        .generations()
        .iter()
        .flat_map(|generation| generation.elections())
        .filter(|candidate| !is_binding(candidate.body()))
        .find_map(|candidate| candidate.accepted().first().copied())
        .expect("a public body accepted a seat");
    let mut public = records.clone();
    public.push((public_assignment, records[0].1));
    refused(
        &driver,
        &public,
        &[(scope, board.clone())],
        "a credential of a public body's seat",
    );
    // Each credential names the sole instruction of an input between the draw and the close.
    let close_height = driver.height;
    let mut late = records.clone();
    late[0].1.source.height = close_height;
    refused(
        &driver,
        &late,
        &[(scope, board.clone())],
        "a credential registered after the close",
    );
    let mut second = records.clone();
    second[0].1.source.instruction_index = 1;
    refused(
        &driver,
        &second,
        &[(scope, board.clone())],
        "a credential that is not its input's sole instruction",
    );
    refused(
        &driver,
        &records,
        &[
            (scope, board.clone()),
            (BallotScopeV1::new([0x7b; 64]), board.clone()),
        ],
        "two boards of one jury",
    );
    let world = activated_world();
    fill(&world, &driver, &records, &[(scope, board.clone())]);
    {
        let mut block = world.block();
        block
            .parliament_anonymous_attempts
            .insert(GovernanceAttemptId::new([0x77; 32]), driver.state.clone());
        block.commit();
    }
    assert!(
        matches!(
            validate_cut(&world.view(), &network(), driver.height),
            Err(AnonymousTablesRestoreError::Invalid(_))
        ),
        "an attempt stored under another identifier is refused"
    );
    let mut moved = board.clone();
    moved.sealed_timestamp_ms += 1;
    refused(
        &driver,
        &records,
        &[(scope, moved)],
        "a board with other seal-time facts",
    );

    // A credential or board of no stored attempt.
    let world = activated_world();
    {
        let mut block = world.block();
        block
            .parliament_anonymous_registrations
            .insert(records[0].0, records[0].1);
        block.commit();
    }
    assert!(matches!(
        validate_cut(&world.view(), &network(), driver.height),
        Err(AnonymousTablesRestoreError::Invalid(_))
    ));
    let world = activated_world();
    {
        let mut block = world.block();
        block.parliament_anonymous_boards.insert(scope, board);
        block.commit();
    }
    assert!(matches!(
        validate_cut(&world.view(), &network(), driver.height),
        Err(AnonymousTablesRestoreError::Invalid(_))
    ));
}

#[test]
fn restore_refuses_future_board_anchors_and_corpus_before_rebuilding() {
    use super::test_driver::{anchor, public_bodies};
    use iroha_data_model::governance::anonymous_ballot::{
        AdmissionNonceV1, AdmissionStampV1, AnonymousBallotCorpusEntryV1, BallotChoiceV1,
        Digest512, NullifierV1, ProofProfileIdV1, VoteAuthorizationV1,
    };

    let mut driver = drawn_small_jury();
    let mut records = Vec::new();
    register(&mut driver, &mut records, 0..4);
    answer_public_bodies(&mut driver);
    let outcomes = close_invitations(&mut driver, &records);
    let sealed = jury_effect(&outcomes).board.as_ref().unwrap().board.clone();
    for body in public_bodies() {
        driver.conclude_public(body);
    }
    driver.deliberate(JURY);
    let mut board = BallotBoardV1::restore(sealed).expect("sealed board");
    let scope = *board.scope();
    let opening_ms = board.schedule_opening(driver.now_ms).expect("opening time");
    board
        .open(anchor(driver.height + 1, 0, opening_ms))
        .expect("open");
    let opened = board.record();
    let world = activated_world();
    fill(&world, &driver, &records, &[(scope, opened)]);
    validate_cut(&world.view(), &network(), driver.height + 1)
        .expect("the opening block does not yet have a casting clock");
    assert!(matches!(
        validate_cut(&world.view(), &network(), driver.height + 2),
        Err(AnonymousTablesRestoreError::Invalid(message))
            if message.contains("missing its mandatory casting clock")
    ));
    board
        .establish_casting_clock(driver.height + 2, opening_ms + 1)
        .expect("clock");
    let height = driver.height + 3;
    board
        .close(anchor(height, 0, board.closing_ms().unwrap()))
        .expect("close");
    driver
        .state
        .close_ballot(driver.id(), *board.closure().unwrap(), &network(), Vec::new)
        .expect("the empty ballot rejects the attempt");
    let record = board.record();
    let world = activated_world();
    fill(&world, &driver, &records, &[(scope, record.clone())]);
    validate_cut(&world.view(), &network(), height).expect("all board heights are in the cut");

    for field in [
        "opening",
        "clock",
        "closure opening",
        "closure clock",
        "closure",
        "corpus",
    ] {
        let mut future = record.clone();
        match field {
            "opening" => future.opening.as_mut().unwrap().height = height + 1,
            "clock" => future.casting_clock.as_mut().unwrap().height = height + 1,
            "closure opening" => {
                future.closure.as_mut().unwrap().opening_anchor.height = height + 1
            }
            "closure clock" => future.closure.as_mut().unwrap().casting_clock_height = height + 1,
            "closure" => future.closure.as_mut().unwrap().closure_anchor.height = height + 1,
            _ => future.entries.push(AnonymousBallotCorpusEntryV1 {
                scope,
                vote: BallotChoiceV1::Aye,
                nullifier: NullifierV1::new([7; 128]),
                auth_v: VoteAuthorizationV1::new([8; 64]),
                profile_id: ProofProfileIdV1::V1,
                stamp: AdmissionStampV1 {
                    nonce: AdmissionNonceV1::new([9; 32]),
                },
                envelope_digest: Digest512::new([10; 64]),
                height: height + 1,
                input_index: 0,
            }),
        }
        let world = activated_world();
        fill(&world, &driver, &records, &[(scope, future)]);
        assert!(
            matches!(
                validate_cut(&world.view(), &network(), height),
                Err(AnonymousTablesRestoreError::Invalid(message))
                    if message.contains("above restored height")
            ),
            "{field}"
        );
    }
}

#[test]
fn only_a_refusal_the_node_cannot_decide_defers() {
    let driver = drawn_small_jury();
    let state = state_with(&driver);
    let local = AnonymousParliamentErrorV1::Seal(Box::new(SealErrorV1::Context(
        ContextErrorV1::Inputs(ProofInputErrorV1::ProfileIdentity),
    )));
    for (error, defers) in [
        (AnonymousParliamentErrorV1::StaleItem, false),
        (AnonymousParliamentErrorV1::CredentialSource, false),
        (AnonymousParliamentErrorV1::CredentialStore, true),
        (local, true),
    ] {
        let mut block = state.block(header(&driver));
        let mut tx = block.transaction();
        let _ = anonymous_refusal(&error, &mut tx);
        assert_eq!(tx.is_execution_deferred(), defers, "{error:?}");
    }
}

/// The position of `signed` among the network inputs of the chain's last block, and the
/// debug form of its result.
fn committed_input(chain: &CertifiedTestChain, signed: &SignedTransaction) -> (u32, String) {
    let committed = chain.committed(chain.height());
    let block = committed.block();
    let position = block
        .external_transactions()
        .position(|candidate| candidate.hash() == signed.hash())
        .expect("the input is in the last block");
    let index = u32::try_from(position).expect("a small block");
    let (_, output) = block
        .network_output_at(index)
        .expect("the input's network output");
    (index, format!("{:?}", output.result))
}

#[test]
fn the_executor_runs_a_registration_only_as_the_sole_instruction_of_its_input() {
    let driver = drawn_small_jury();
    let (election, invited) = jury_invitations(&driver);
    let (assignment, member) = invited[0].clone();
    // The member's account, the active family and the attempt, as the family's activation
    // and the attempt's creation leave them.
    let world = World::with([], [Account::new(member.clone()).build(&member)], []);
    crate::runtime::activate_additions_for_testing(&world, PARLIAMENT_ANONYMOUS.module)
        .expect("activate the anonymous family");
    let mut block = world.block();
    block
        .parliament_anonymous_attempts
        .insert(driver.id(), driver.state.clone());
    block.commit();
    let mut config = TestChainConfig::new(world, 1_000);
    config.genesis_parameters.push(Parameter::Custom(
        ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
            .to_custom_parameter()
            .expect("candidate policy"),
    ));
    let mut chain = CertifiedTestChain::start(config).expect("the chain starts");

    // A genuine registration of the first seat, bound to this chain's network.
    let context = driver
        .state
        .binding_body_context(&chain.network_id(), election)
        .expect("the jury's context");
    let handle =
        CredentialHandleV1::import_v1(context, &[0x61; 48], None).expect("fixture credential");
    let input = RegisterParliamentCredentialV1 {
        governance_attempt_id: driver.id(),
        election_attempt_id: election,
        registration: prepare_registration_v1(&handle, &member, &assignment)
            .expect("genuine registration"),
    };
    let key = citizen_key(&member);

    // Beside another instruction the registration has no original: the executor arms only
    // the sole instruction of a direct input, so the input is refused and nothing changes.
    let batch = chain.sign(
        &key,
        [
            InstructionBox::from(input.clone()),
            InstructionBox::from(input.clone()),
        ],
        driver.now_ms,
    );
    assert_eq!(
        chain.commit_at(driver.now_ms + 1, vec![batch.clone()]),
        vec![false]
    );
    let (_, refusal) = committed_input(&chain, &batch);
    assert!(
        refusal.contains("one-use direct signed original"),
        "{refusal}"
    );
    assert!(
        chain
            .state()
            .view()
            .world()
            .parliament_anonymous_registrations()
            .is_empty()
    );

    // As the sole instruction of its input it registers, and the credential names that
    // input as its source.
    let sole = chain.sign(&key, [InstructionBox::from(input)], driver.now_ms + 1);
    assert_eq!(
        chain.commit_at(driver.now_ms + 2, vec![sole.clone()]),
        vec![true]
    );
    let (input_index, _) = committed_input(&chain, &sole);
    let view = chain.state().view();
    let record = *view
        .world()
        .parliament_anonymous_registrations()
        .get(&assignment)
        .expect("the credential is stored");
    assert_eq!(
        record.source,
        ParliamentCredentialSourceV1 {
            height: chain.height(),
            input_index: u64::from(input_index),
            instruction_index: 0,
            entrypoint_hash: sole.hash_as_entrypoint(),
        }
    );
    assert!(
        view.world()
            .parliament_anonymous_attempts()
            .get(&driver.id())
            .expect("the attempt")
            .accepted_assignments(election)
            .is_ok_and(|accepted| accepted.contains(&assignment)),
        "the seat is accepted with its credential"
    );
    crate::state::parliament_registration::assert_registration_history(
        &chain, &context, assignment, &record, &sole, &batch,
    );
}
