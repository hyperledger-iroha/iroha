// Source capacity must bind the real durable autonomous Queue reservation.

#[test]
fn autonomous_queue_reservation_caps_source_before_durable_fifo_ownership() {
    use iroha_data_model::parameter::{BlockParameter, FastpqSourcePolicyV1, Parameter};
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
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
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("source reservation journal");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let transactions = (0..3)
        .map(|_| accepted_queue_plan_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    for tx in &transactions {
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx.clone());
    }
    let selected = queue
        .reserve_transactions_for_lane_bounded(
            &state,
            AutonomousLaneReservationSelectionAuthorization::single_validator_for_test(
                lane_reservation_scope(&state, b"source-cap-owner", b"source-cap-proposal"),
            ),
            LaneQueueReservationSelectionLimits {
                max_transactions: nonzero!(3_usize),
                max_scan: nonzero!(3_usize),
                max_encoded_bytes: NonZeroU64::new(u64::MAX).unwrap(),
                max_gas: NonZeroU64::new(u64::MAX).unwrap(),
            },
            &BTreeSet::new(),
            LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
        )
        .expect("real Queue selection uses its pinned State source profile");
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0].as_accepted().hash_as_entrypoint(),
        transactions[0].hash_as_entrypoint()
    );
    assert_eq!(queue.live_lane_reservations().len(), 1);
    assert_eq!(queue.queued_len(), 2);
    queue
        .release_lane_reservation(selected[0].key())
        .expect("release only the actual bounded reservation");
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.queued_len(), 3);
}
