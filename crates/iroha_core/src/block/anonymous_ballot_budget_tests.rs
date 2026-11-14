// The per-block anonymous-ballot budget of block validation
// (`specs/parliament_private_ballot_design.md` §3.6, §3.10, §12.3 slice 4): every ballot input
// is charged against the `gov.parliament_ballot` value and the boards the block's parent
// commits, before any input executes.

#[test]
fn anonymous_ballot_budget_is_judged_against_the_committed_parent_world() {
    use crate::governance::parliament_anonymous::ballot_input::tests::{
        AYE_0, AYE_1, AYE_2, CastingFixtureV1, authorization, envelope, pinned_offset, stamped,
    };
    use iroha_data_model::{
        governance::anonymous_ballot::{BallotScopeV1, CanonicalAnonymousBallotEnvelopeV1},
        parameter::{Parameter, ParliamentBallotParametersV1},
    };

    let fixture = CastingFixtureV1::open();
    let state = fixture.state();
    let key_pair = KeyPair::try_from_seed(vec![0x57; 32], Algorithm::Ed25519)
        .expect("deterministic leader key");
    let carrier = |ballots: Vec<CanonicalAnonymousBallotEnvelopeV1>| {
        let mut builder = iroha_data_model::block::builder::BlockBuilder::new(
            fixture.next_header(fixture.closing_ms() - 5),
        );
        for ballot in ballots {
            builder.push_anonymous_ballot(ballot);
        }
        builder.build_with_signature(0, key_pair.private_key())
    };
    let refusal = |block: &SignedBlock| {
        let state_block = state.block(block.header());
        match ValidBlock::validate_anonymous_ballot_budget(block, &state_block) {
            Ok(()) => None,
            Err(BlockValidationError::ExecutionContextInvalid(message)) => Some(message),
            Err(other) => panic!("a verdict on the block, never a local refusal: {other:?}"),
        }
    };

    // A block without ballots reads nothing, and the default budget carries two ballots.
    assert_eq!(refusal(&carrier(Vec::new())), None);
    let full = carrier(vec![envelope(AYE_0), envelope(AYE_1)]);
    assert_eq!(refusal(&full), None);

    // One ballot more than `B_work`, one carrier twice and a scope without a sealed board
    // each invalidate the block; nothing is executed to find out.
    let default = ParliamentBallotParametersV1::default();
    assert_eq!(
        refusal(&carrier(vec![
            envelope(AYE_0),
            envelope(AYE_1),
            envelope(AYE_2)
        ])),
        Some(format!(
            "the block carries more than {} anonymous ballots",
            default.max_verification_reservations_per_block
        ))
    );
    assert_eq!(
        refusal(&carrier(vec![envelope(AYE_0), envelope(AYE_0)])),
        Some("the block carries one anonymous-ballot carrier twice".to_owned())
    );
    let mut foreign = authorization(AYE_1);
    foreign.scope = BallotScopeV1::new([0xee; 64]);
    assert_eq!(
        refusal(&carrier(vec![stamped(&foreign, pinned_offset(AYE_1))])),
        Some("an anonymous-ballot input names a scope without a sealed board".to_owned())
    );

    // A smaller budget written by the block itself is invisible: the parent's value judges.
    let mut state_block = state.block(full.header());
    state_block
        .world
        .set_parameter_for_testing(Parameter::Custom(
            ParliamentBallotParametersV1 {
                max_verification_reservations_per_block: 1,
                ..default
            }
            .to_custom_parameter()
            .expect("ballot parameters"),
        ))
        .expect("the block overlay takes the value");
    ValidBlock::validate_anonymous_ballot_budget(&full, &state_block)
        .expect("the budget of the committed parent");
}
