//! Signerless anonymous ballots under the Network output owner, inside a block
//! (`specs/parliament_private_ballot_design.md` §3.6 and §12.3 slice 4).
//!
//! The carrier is a component fixture, like the other Network output tests: it carries
//! genuine fixture ballots over the sealed jury's World at the block after the fixture's
//! last one, and the governance pass is run on it as the global chain's block start runs it.
//! These tests establish execution behavior, not carrier finality, lanes or DA.

use iroha_data_model::{
    governance::anonymous_ballot::AnonymousBallotRejectionV1,
    transaction::error::TransactionRejectionReason,
};

use super::*;
use crate::governance::parliament_anonymous::ballot_input::tests::{
    AYE_0, AYE_1, CastingFixtureV1, FORGED_1, NAY_0, authorization, envelope,
};

/// The fixture's State with what a signed genesis commits for every carrier: the ordinary
/// crypto policy, which Network admission of every input reads (a ballot carries no
/// signature it would judge), and the output phases of the other component fixtures (no
/// Pipeline trigger, one Time slot), so that a complete block resolves every obligation.
fn casting_state(fixture: &CastingFixtureV1) -> State {
    let state = fixture.state();
    let mut world = state.world.block();
    let mut policy = ExecutionOutputPolicyV1::bootstrap();
    policy.max_pipeline_triggers = 0;
    policy.max_time_invocations = 1;
    policy.validate().expect("feasible fixture policy");
    world
        .set_parameter_for_testing(Parameter::Block(BlockParameter::ExecutionOutput(policy)))
        .expect("commit the output policy");
    world
        .set_parameter_for_testing(Parameter::Block(BlockParameter::MaxTimeTriggerInvocations(
            NonZeroU32::MIN,
        )))
        .expect("commit the Time slot");
    world
        .set_parameter_for_testing(
            iroha_data_model::crypto_admission_policy::CryptoAdmissionPolicyV1::new(vec![
                iroha_crypto::Algorithm::Ed25519,
            ])
            .expect("one admitted algorithm")
            .into_parameter()
            .expect("a committed policy"),
        )
        .expect("commit the ordinary crypto policy");
    world.commit();
    state
}

/// The carrier after the fixture's last block, created at `time_ms`, whose inputs are the
/// fixture ballots `ballots` in order.
fn ballot_carrier(fixture: &CastingFixtureV1, time_ms: u64, ballots: &[usize]) -> SignedBlock {
    let inputs: Vec<TransactionEntrypoint> = ballots
        .iter()
        .map(|index| TransactionEntrypoint::AnonymousBallot(envelope(*index)))
        .collect();
    let context = iroha_data_model::block::BlockExecutionContextBundle::new(
        inputs
            .iter()
            .map(|input| {
                iroha_data_model::block::ExternalExecutionContext::new(
                    input.hash(),
                    iroha_model_base::topology::LaneId::SINGLE,
                    iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                )
            })
            .collect(),
    );
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(fixture.next_height()).expect("nonzero height"),
        Some(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"anonymous ballot carrier parent"),
        )),
        None,
        time_ms,
        0,
    ));
    for input in inputs {
        let TransactionEntrypoint::AnonymousBallot(envelope) = input else {
            unreachable!("ballot inputs only")
        };
        builder.push_anonymous_ballot(envelope);
    }
    builder.set_execution_context(Some(context));
    builder.build_with_signature(0, ALICE_KEYPAIR.private_key())
}

/// Run the governance pass of `block` as the block start of the global chain runs it.
fn governance_pass(block: &mut StateBlock<'_>) {
    crate::state::governance_start::run_on_global_chain::<()>(block, |_| Ok(()))
        .expect("the governance pass");
    assert!(block.parliament_pass_ran());
}

/// Execute `source` on `state` after `prepare`, and judge the block and its result.
fn executed<R>(
    state: &State,
    source: &SignedBlock,
    prepare: impl FnOnce(&mut StateBlock<'_>),
    judge: impl FnOnce(&StateBlock<'_>, Result<(), ExecutionOutputAttemptError>) -> R,
) -> R {
    let (mut block, _recording) = recorded_network_block(state, source);
    prepare(&mut block);
    let result = execute(&mut block, source);
    judge(&block, result)
}

/// The Network rows `block` retained, in input order.
fn ballot_rows<'a>(block: &'a StateBlock<'_>) -> Vec<&'a NetworkExecutionOutputV1> {
    retained(block)
        .rows
        .iter()
        .filter_map(|row| match row {
            ExecutionOutputV1::Network(row) => Some(row),
            _ => None,
        })
        .collect()
}

/// The corpus entries the board of the fixture's scope keeps in `block`.
fn corpus_len(block: &StateBlock<'_>, fixture: &CastingFixtureV1) -> usize {
    block
        .world
        .parliament_anonymous_boards
        .get(&fixture.scope())
        .expect("the jury's board")
        .entries
        .len()
}

#[test]
fn a_block_accepts_a_ballot_and_records_a_duplicate_nullifier_in_input_order() {
    let fixture = CastingFixtureV1::open();
    let state = casting_state(&fixture);
    // Seat 0 votes Aye and then Nay under the same nullifier, in one block.
    let source = ballot_carrier(&fixture, fixture.closing_ms() - 5, &[AYE_0, NAY_0]);
    executed(&state, &source, governance_pass, |block, result| {
        result.expect("a valid block");
        let rows = ballot_rows(block);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].input_index, 0);
        assert_eq!(rows[0].result, TransactionResult::new(Ok(Vec::new())));
        assert!(rows[0].completions.is_empty());
        assert_eq!(rows[1].input_index, 1);
        assert!(
            matches!(
                rows[1].result.as_ref(),
                Err(TransactionRejectionReason::AnonymousBallot(
                    AnonymousBallotRejectionV1::DuplicateNullifier
                ))
            ),
            "{:?}",
            rows[1].result
        );
        // The accepted ballot alone joined the corpus, at this block and its input index.
        let board = block
            .world
            .parliament_anonymous_boards
            .get(&fixture.scope())
            .expect("the jury's board");
        assert_eq!(board.entries.len(), 1);
        assert_eq!(board.entries[0].nullifier, authorization(AYE_0).nullifier);
        // A ballot spends no ordinary gas.
        assert_eq!(block.gas_used_in_block, 0);
    });
}

#[test]
fn a_ballot_of_the_closing_block_is_recorded_outside_its_window() {
    let fixture = CastingFixtureV1::open();
    let state = casting_state(&fixture);
    // The pass of the closing block freezes the corpus before any input executes.
    let source = ballot_carrier(&fixture, fixture.closing_ms(), &[AYE_1]);
    executed(&state, &source, governance_pass, |block, result| {
        result.expect("a valid block");
        let rows = ballot_rows(block);
        assert_eq!(rows.len(), 1);
        assert!(
            matches!(
                rows[0].result.as_ref(),
                Err(TransactionRejectionReason::AnonymousBallot(
                    AnonymousBallotRejectionV1::OutsideCastingWindow
                ))
            ),
            "{:?}",
            rows[0].result
        );
        assert_eq!(corpus_len(block, &fixture), 0);
    });
}

#[test]
fn an_invalid_ballot_invalidates_its_block_and_leaves_no_row() {
    let fixture = CastingFixtureV1::open();
    let state = casting_state(&fixture);
    // A well-stamped ballot whose proof does not verify, after a valid one.
    let source = ballot_carrier(&fixture, fixture.closing_ms() - 5, &[AYE_0, FORGED_1]);
    executed(&state, &source, governance_pass, |block, result| {
        let error = result.expect_err("a block that carries an invalid ballot");
        assert!(
            matches!(&error, ExecutionOutputAttemptError::Owner(_)),
            "a verdict on the block, never a local refusal or a recorded rejection: {error:?}"
        );
        assert!(!matches!(
            block.execution_output_plan,
            Some(ExecutionOutputPlanState::Retained(_))
        ));
    });
}

#[test]
fn a_ballot_needs_a_block_whose_governance_pass_ran_parliaments_items() {
    let fixture = CastingFixtureV1::open();
    let state = casting_state(&fixture);
    let source = ballot_carrier(&fixture, fixture.closing_ms() - 5, &[AYE_0]);
    // This component block has no native schedule request: its block start ran no pass.
    executed(
        &state,
        &source,
        |block| assert!(!block.parliament_pass_ran()),
        |block, result| {
            let Err(error) = result else {
                panic!("a ballot executed in a block whose pass ran no Parliament item");
            };
            assert!(
                matches!(&error, ExecutionOutputAttemptError::Owner(reason)
                    if reason.contains("governance pass ran Parliament's items")),
                "{error:?}"
            );
            assert_eq!(corpus_len(block, &fixture), 0);
        },
    );
}

#[test]
fn a_board_the_node_cannot_recompute_defers_the_block() {
    let fixture = CastingFixtureV1::open();
    let state = casting_state(&fixture);
    let source = ballot_carrier(&fixture, fixture.closing_ms() - 5, &[AYE_0]);
    let scope = fixture.scope();
    executed(
        &state,
        &source,
        |block| {
            governance_pass(block);
            // An opening time before the end of the notice: no transition produces it, so
            // the stored record does not recompute when the input looks its scope up.
            let mut board = block
                .world
                .parliament_anonymous_boards
                .get(&scope)
                .expect("the jury's board")
                .clone();
            board.opening_at_ms = Some(0);
            block.world.parliament_anonymous_boards.insert(scope, board);
        },
        |_, result| {
            let error = result.expect_err("World's corruption is never a verdict");
            assert!(
                matches!(&error, ExecutionOutputAttemptError::Deferred(original)
                    if original.reason()
                        == ivm::error::ExecutionDeferral::VerifierArtifactsUnavailable),
                "{error:?}"
            );
        },
    );
}
