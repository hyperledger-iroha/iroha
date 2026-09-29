// Native payload packing uses the committed source policy without reserving transactions.
#[test]
fn native_source_cap_bounds_selection_while_all_inputs_remain_pending() {
    use iroha_data_model::parameter::{BlockParameter, FastpqSourcePolicyV1, Parameter};
    let (_, time) = TimeSource::new_mock(Duration::default());
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    {
        let mut world = state.world.block();
        let parameters = world.parameters.get_mut();
        let block = parameters.block();
        let policy = block.fastpq_source();
        let one = FastpqSourcePolicyV1::from_sizing(
            block.execution_output(),
            policy.intrinsic,
            policy.mandatory,
            1,
        )
        .unwrap();
        parameters.set_parameter(Parameter::Block(BlockParameter::FastpqSource(one)));
        world.commit();
    }
    let queue = Arc::new(Queue::test(config_factory(), &time));
    let transactions = (0..3)
        .map(|_| accepted_tx_by_someone(&time))
        .collect::<Vec<_>>();
    for tx in &transactions {
        queue.push(tx.clone(), state.view()).unwrap();
    }
    for _ in 0..2 {
        let selected = crate::sumeragi::payload::select(&state, &queue, usize::MAX, 0);
        assert_eq!(selected.len(), 1);
        assert_eq!(
            selected[0].hash_as_entrypoint(),
            transactions[0].hash_as_entrypoint()
        );
        assert_eq!((queue.active_len(), queue.queued_len()), (3, 3));
    }
    assert!(crate::sumeragi::payload::select(&state, &queue, usize::MAX, 1).is_empty());
    assert_eq!(
        queue.remove_committed_hashes([transactions[0].hash_as_entrypoint()], None),
        1
    );
    let selected = crate::sumeragi::payload::select(&state, &queue, usize::MAX, 0);
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0].hash_as_entrypoint(),
        transactions[1].hash_as_entrypoint()
    );
    assert_eq!((queue.active_len(), queue.queued_len()), (2, 2));
}
