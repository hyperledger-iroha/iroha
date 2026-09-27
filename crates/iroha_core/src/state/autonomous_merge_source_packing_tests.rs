// Whole-source packing regressions using the authenticated gas fixture sources.

#[test]
fn autonomous_source_capacity_uses_the_complete_immutable_source_envelope() {
    use iroha_data_model::parameter::{
        BlockParameter, FastpqSourcePolicyV1, Parameter, Parameters,
    };
    let mut parameters = Parameters::default();
    let original = parameters.block().fastpq_source();
    let policy = FastpqSourcePolicyV1::from_sizing(
        parameters.block().execution_output(),
        original.intrinsic,
        original.mandatory,
        2,
    )
    .unwrap();
    parameters.set_parameter(Parameter::Block(BlockParameter::FastpqSource(policy)));
    assert_eq!(
        autonomous_source_input_capacity(parameters.block()).unwrap(),
        2
    );
    parameters.set_parameter(Parameter::Block(BlockParameter::MaxTransactions(
        NonZeroU64::MIN,
    )));
    assert_eq!(
        autonomous_source_input_capacity(parameters.block()).unwrap(),
        2,
        "an ordinary selection reduction does not redefine a certified autonomous source"
    );
    let mut invalid = policy;
    invalid.block.max_executed_entries = 0;
    parameters.set_parameter(Parameter::Block(BlockParameter::FastpqSource(invalid)));
    assert!(autonomous_source_input_capacity(parameters.block()).is_err());
}

state_test!(consensus_stack autonomous_merge_packing_keeps_complete_sources_within_shared_source_capacity
    autonomous_merge_packing_keeps_complete_sources_within_shared_source_capacity_on_consensus_stack();
);
fn autonomous_merge_packing_keeps_complete_sources_within_shared_source_capacity_on_consensus_stack()
 {
    let (state, keys, _) = autonomous_gas_budget_fixture();
    let gas_limit = gas_limit_from_parameters(state.world.view().parameters());
    let left = autonomous_gas_budget_source(&state, &keys, LaneId::SINGLE, gas_limit / 2, 0xE1);
    let right = autonomous_gas_budget_source(&state, &keys, LaneId::new(1), gas_limit / 2, 0xE2);
    let original_bytes = [left.source_bundle.clone(), right.source_bundle.clone()];
    let original_hashes = [left.bundle_hash, right.bundle_hash];
    let one = select_merge_execution_source_budget(vec![right.clone(), left.clone()], gas_limit, 1)
        .expect("the source ceiling binds before either source executes");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].bundle_hash, original_hashes[0]);
    assert_eq!(one[0].source_bundle, original_bytes[0]);
    assert_eq!(one[0].input.entrypoints, left.input.entrypoints);
    assert_eq!(one[0].input.reservation_keys, left.input.reservation_keys);
    let remaining =
        select_merge_execution_source_budget(vec![right.clone()], gas_limit, 1).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].bundle_hash, original_hashes[1]);
    assert_eq!(remaining[0].source_bundle, original_bytes[1]);
    let both =
        select_merge_execution_source_budget(vec![right.clone(), left], gas_limit, 2).unwrap();
    assert_eq!(both.len(), 2);
    assert_eq!(
        both.iter()
            .map(|source| source.input.entrypoints.len())
            .sum::<usize>(),
        2
    );
    assert!(
        select_merge_execution_source_budget(vec![right.clone()], gas_limit, 0)
            .unwrap()
            .is_empty(),
        "an indivisible certified source cannot be split to fit an exhausted budget"
    );
    assert_eq!(right.bundle_hash, original_hashes[1]);
    assert_eq!(right.source_bundle, original_bytes[1]);
}
