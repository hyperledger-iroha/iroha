#[tokio::test]
async fn push_tx() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("Failed to push tx into queue");
}
#[test]
fn enforce_lane_teu_limits_defers_when_capacity_exceeded() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    // Prepare a representative transaction so we can size the lane capacity to its TEU weight.
    let first_tx = accepted_tx_by_someone(&time_source);
    let lane_capacity = Queue::compute_teu_weight(&first_tx);
    assert!(
        lane_capacity > 0,
        "expected positive TEU weight for sample transaction"
    );
    let test_lane = LaneId::new(7);
    let test_dataspace = DataSpaceId::new(1);
    install_test_nexus_routes(&mut state, &[(test_lane, test_dataspace)]);
    let state = Arc::new(state);
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: test_lane,
        dataspace: test_dataspace,
    });
    let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
    let queue_inner = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router,
        &[(test_lane, test_dataspace)],
    );
    let queue = Arc::new(queue_inner);
    let first_hash = first_tx.as_ref().hash();
    queue
        .push(first_tx, state.view())
        .expect("first push should succeed");
    let second_tx = accepted_tx_by_someone(&time_source);
    let second_hash = second_tx.as_ref().hash();
    queue
        .push(second_tx, state.view())
        .expect("second push should succeed");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
    assert_eq!(
        guards.len(),
        2,
        "expected both transactions before TEU gating"
    );
    *queue.nexus_limits.write() = QueueLimits {
        fallback: scheduling,
        per_lane: BTreeMap::from([(test_lane, scheduling)]),
    };
    let mut deferred = queue.enforce_lane_teu_limits(&mut guards);
    assert_eq!(
        guards.len(),
        1,
        "only one transaction should remain after enforcement"
    );
    assert_eq!(deferred.len(), 1, "one transaction should be deferred");
    assert_eq!(
        guards[0].as_ref().hash(),
        first_hash,
        "first transaction should remain executable"
    );
    assert_eq!(
        deferred[0].as_ref().hash(),
        second_hash,
        "second transaction should be deferred"
    );
    assert_eq!(guards[0].routing().lane_id, test_lane);
    drop(guards);
    *queue.nexus_limits.write() = QueueLimits::from_nexus(&state.nexus_snapshot());
    queue
        .return_transaction_guards(&mut deferred, state.as_ref())
        .expect("returning deferred guard should succeed once capacity resets");
    let next = queue.collect_transactions_for_block(&state.view(), nonzero!(1usize));
    assert_eq!(
        next.len(),
        1,
        "deferred transaction should be available next slot"
    );
    assert_eq!(next[0].as_ref().hash(), second_hash);
}
#[test]
fn enforce_lane_teu_limits_with_routing_plans_preserves_guard_ownership() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let (account_id, key_pair) = gen_account_in("wonderland");
    let first_tx = accepted_tx_with(
        account_id.clone(),
        &key_pair,
        &time_source,
        vec![InstructionBox::from(Log::new(
            Level::INFO,
            "teu requeue first".into(),
        ))],
        Metadata::default(),
    );
    let lane_capacity = Queue::compute_teu_weight(&first_tx);
    assert!(
        lane_capacity > 0,
        "expected positive TEU weight for sample transaction"
    );
    let test_lane = LaneId::new(9);
    let test_dataspace = DataSpaceId::new(3);
    let (lane_catalog, dataspace_catalog) =
        Queue::test_catalogs_for_routes(&[(test_lane, test_dataspace)]);
    let mut lanes = lane_catalog.lanes().to_vec();
    lanes
        .iter_mut()
        .find(|lane| lane.id == test_lane)
        .expect("test lane should exist")
        .metadata
        .insert(
            "scheduler.teu_capacity".to_string(),
            lane_capacity.to_string(),
        );
    let lane_catalog = LaneCatalog::new(lane_catalog.lane_count(), lanes).expect("lane catalog");
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.lane_catalog = lane_catalog;
    nexus.dataspace_catalog = (*dataspace_catalog).clone();
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    nexus.routing_policy.default_lane = test_lane;
    nexus.routing_policy.default_dataspace = test_dataspace;
    state.set_nexus(nexus).expect("set Nexus config");
    let state = Arc::new(state);
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let first_hash = first_tx.as_ref().hash();
    queue
        .push(first_tx, state.view())
        .expect("first push should succeed");
    time_handle.advance(Duration::from_millis(1));
    let second_tx = accepted_tx_with(
        account_id,
        &key_pair,
        &time_source,
        vec![InstructionBox::from(Log::new(
            Level::INFO,
            "teu requeue second".into(),
        ))],
        Metadata::default(),
    );
    let second_hash = second_tx.as_ref().hash();
    queue
        .push(second_tx, state.view())
        .expect("second push should succeed");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
    assert_eq!(guards.len(), 2, "expected both transactions before gating");
    let mut consumed = BTreeMap::new();
    let mut deferred = queue
        .enforce_lane_teu_limits_with_consumption_and_routing_plans(&mut guards, &mut consumed);
    assert_eq!(guards.len(), 1, "one transaction should remain");
    assert_eq!(deferred.len(), 1, "one transaction should defer");
    assert_eq!(
        guards[0].as_ref().hash(),
        first_hash,
        "first transaction should remain executable"
    );
    let deferred_tx = deferred[0].clone_accepted();
    let deferred_plan = deferred[0].routing_plan();
    let deferred_routing = deferred_plan.coordinator_route();
    assert_eq!(deferred_tx.as_ref().hash(), second_hash);
    assert_eq!(deferred_routing.lane_id, test_lane);
    assert_eq!(deferred_routing.dataspace_id, test_dataspace);
    drop(guards);
    queue
        .return_transaction_guards(&mut deferred, state.as_ref())
        .expect("deferred guard should return atomically");
    let next = queue.collect_transactions_for_block(&state.view(), nonzero!(1usize));
    assert_eq!(next.len(), 1, "deferred transaction should be queued next");
    assert_eq!(next[0].as_ref().hash(), second_hash);
}
#[test]
fn enforce_lane_teu_limits_preserves_native_amx_requeue_plan() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_fixture = native_amx_participant_drift_fixture(&time_source);
    time_handle.advance(Duration::from_millis(1));
    let second_fixture = native_amx_participant_drift_fixture(&time_source);
    let mut state = first_fixture.state;
    let first_tx = first_fixture.tx;
    let second_tx = second_fixture.tx;
    let second_authority = second_tx
        .external()
        .expect("Native AMX fixture transaction should be external")
        .authority()
        .clone();
    register_test_authority(&mut state, &second_authority);
    let first_plan = first_fixture.current_plan;
    let second_plan = second_fixture.current_plan;
    let coordinator = first_plan.coordinator_route();
    assert_eq!(coordinator, second_plan.coordinator_route());
    assert!(matches!(second_plan, RoutingPlan::NativeAmx(_)));
    let first_teu = Queue::compute_teu_weight(&first_tx);
    assert!(first_teu > 0, "expected positive TEU weight");
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        let mut lanes = nexus.lane_catalog.lanes().to_vec();
        lanes
            .iter_mut()
            .find(|lane| lane.id == coordinator.lane_id)
            .expect("coordinator lane should exist")
            .metadata
            .insert("scheduler.teu_capacity".to_string(), first_teu.to_string());
        nexus.lane_catalog =
            LaneCatalog::new(nexus.lane_catalog.lane_count(), lanes).expect("lane catalog");
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    }
    let state = Arc::new(state);
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let first_hash = first_tx.hash();
    queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            first_tx,
            state.as_ref(),
            first_plan,
            None,
        )
        .expect("first Native AMX transaction should enqueue");
    let second_hash = second_tx.hash();
    queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            second_tx,
            state.as_ref(),
            second_plan.clone(),
            None,
        )
        .expect("second Native AMX transaction should enqueue");
    assert_eq!(
        queue
            .queue_limits()
            .for_lane(coordinator.lane_id)
            .teu_capacity,
        first_teu
    );
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
    assert_eq!(guards.len(), 2, "expected both transactions before gating");
    let mut consumed = BTreeMap::new();
    let mut deferred = queue
        .enforce_lane_teu_limits_with_consumption_and_routing_plans(&mut guards, &mut consumed);
    assert_eq!(guards.len(), 1, "one Native AMX transaction should remain");
    assert_eq!(deferred.len(), 1, "one Native AMX transaction should defer");
    assert_eq!(guards[0].as_ref().hash(), first_hash);
    let deferred_tx = deferred[0].clone_accepted();
    let deferred_plan = deferred[0].routing_plan();
    assert_eq!(deferred_tx.as_ref().hash(), second_hash);
    assert_eq!(
        deferred_plan, second_plan,
        "TEU deferral must preserve the full Native AMX routing plan"
    );
    assert!(matches!(deferred_plan, RoutingPlan::NativeAmx(_)));
    drop(guards);
    queue
        .return_transaction_guards(&mut deferred, state.as_ref())
        .expect("deferred Native AMX guard should return atomically");
    let next = queue.collect_transactions_for_block(&state.view(), nonzero!(1usize));
    assert_eq!(
        next.len(),
        1,
        "deferred Native AMX transaction should requeue"
    );
    assert_eq!(next[0].as_ref().hash(), second_hash);
    assert_eq!(next[0].routing_plan(), second_plan);
}
#[test]
fn overweight_lane_not_starved_when_not_first_in_batch() {
    struct SequenceRouter {
        decisions: parking_lot::Mutex<Vec<RoutingDecision>>,
    }
    impl LaneRouter for SequenceRouter {
        fn route(&self, _tx: &dyn TransactionRoutingView) -> RoutingDecision {
            let mut decisions = self.decisions.lock();
            decisions.remove(0)
        }
    }
    let lane_a = LaneId::new(1);
    let lane_b = LaneId::new(2);
    let dataspace_a = DataSpaceId::new(11);
    let dataspace_b = DataSpaceId::new(12);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    install_test_nexus_routes(&mut state, &[(lane_a, dataspace_a), (lane_b, dataspace_b)]);
    let state = Arc::new(state);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_tx = accepted_tx_by_someone(&time_source);
    let second_tx = accepted_tx_by_someone(&time_source);
    let overweight_teu = Queue::compute_teu_weight(&second_tx);
    assert!(overweight_teu > 1, "expected non-trivial TEU weight");
    let lane_a_limits = LaneSchedulingLimits::new(overweight_teu.saturating_mul(2), 0);
    let lane_b_bounds = LaneSchedulingLimits::new(overweight_teu.saturating_sub(1), 0);
    let router: Arc<dyn LaneRouter> = Arc::new(SequenceRouter {
        decisions: parking_lot::Mutex::new(vec![
            RoutingDecision::new(lane_a, dataspace_a),
            RoutingDecision::new(lane_b, dataspace_b),
            RoutingDecision::new(lane_a, dataspace_a),
            RoutingDecision::new(lane_b, dataspace_b),
        ]),
    });
    let queue_inner = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router,
        &[(lane_a, dataspace_a), (lane_b, dataspace_b)],
    );
    let queue = Arc::new(queue_inner);
    queue
        .push(first_tx, state.view())
        .expect("lane A push should succeed");
    queue
        .push(second_tx, state.view())
        .expect("lane B push should succeed");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
    assert_eq!(guards.len(), 2, "expected both transactions before gating");
    *queue.nexus_limits.write() = QueueLimits {
        fallback: lane_b_bounds,
        per_lane: BTreeMap::from([(lane_a, lane_a_limits), (lane_b, lane_b_bounds)]),
    };
    let mut consumed = BTreeMap::new();
    let deferred = queue.enforce_lane_teu_limits_with_consumption(&mut guards, &mut consumed);
    assert!(
        deferred.is_empty(),
        "overweight transaction for lane B must not be starved"
    );
    assert!(
        guards.iter().any(|guard| guard.routing().lane_id == lane_b),
        "lane B transaction should be retained"
    );
}
#[test]
fn enforce_lane_teu_limits_with_consumption_respects_existing_usage() {
    let test_lane = LaneId::new(11);
    let test_dataspace = DataSpaceId::new(5);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    install_test_nexus_routes(&mut state, &[(test_lane, test_dataspace)]);
    let state = Arc::new(state);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_tx = accepted_tx_by_someone(&time_source);
    let lane_capacity = Queue::compute_teu_weight(&first_tx);
    assert!(lane_capacity > 0, "expected positive TEU weight");
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: test_lane,
        dataspace: test_dataspace,
    });
    let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
    let queue_inner = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router,
        &[(test_lane, test_dataspace)],
    );
    let queue = Arc::new(queue_inner);
    queue
        .push(first_tx, state.view())
        .expect("first push should succeed");
    let second_tx = accepted_tx_by_someone(&time_source);
    queue
        .push(second_tx, state.view())
        .expect("second push should succeed");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
    assert_eq!(guards.len(), 2, "expected both transactions before gating");
    *queue.nexus_limits.write() = QueueLimits {
        fallback: scheduling,
        per_lane: BTreeMap::from([(test_lane, scheduling)]),
    };
    let mut consumed = BTreeMap::new();
    consumed.insert(test_lane, lane_capacity);
    let deferred = queue.enforce_lane_teu_limits_with_consumption(&mut guards, &mut consumed);
    assert!(guards.is_empty(), "all transactions should be deferred");
    assert_eq!(deferred.len(), 2, "both transactions should defer");
    assert_eq!(
        consumed.get(&test_lane),
        Some(&lane_capacity),
        "lane consumption should remain capped at the configured capacity"
    );
}
#[test]
fn enforce_lane_teu_limits_is_deterministic_across_guard_order() {
    fn run_with_order<F>(
        mut reorder: F,
        first_tx: &AcceptedTransaction<'static>,
        second_tx: &AcceptedTransaction<'static>,
    ) -> (Vec<SignedTxHash>, Vec<SignedTxHash>)
    where
        F: FnMut(&mut Vec<TransactionGuard>),
    {
        let test_lane = LaneId::new(17);
        let test_dataspace = DataSpaceId::new(4);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state = State::new(world_with_test_domains(), kura, query_handle);
        install_test_nexus_routes(&mut state, &[(test_lane, test_dataspace)]);
        let state = Arc::new(state);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let lane_capacity = Queue::compute_teu_weight(first_tx);
        assert!(lane_capacity > 0, "expected positive TEU weight");
        let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
            lane: test_lane,
            dataspace: test_dataspace,
        });
        let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
        let queue_inner = Queue::test_with_router_for_routes(
            config_factory(),
            &time_source,
            router,
            &[(test_lane, test_dataspace)],
        );
        let queue = Arc::new(queue_inner);
        queue
            .push(first_tx.clone(), state.view())
            .expect("first push should succeed");
        queue
            .push(second_tx.clone(), state.view())
            .expect("second push should succeed");
        let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
        assert_eq!(guards.len(), 2, "expected both transactions before gating");
        reorder(&mut guards);
        *queue.nexus_limits.write() = QueueLimits {
            fallback: scheduling,
            per_lane: BTreeMap::from([(test_lane, scheduling)]),
        };
        let mut consumed = BTreeMap::new();
        let deferred = queue.enforce_lane_teu_limits_with_consumption(&mut guards, &mut consumed);
        let retained_hashes = guards
            .iter()
            .map(|guard| guard.tx.as_ref().hash())
            .collect::<Vec<_>>();
        let deferred_hashes = deferred
            .iter()
            .map(|guard| guard.as_ref().hash())
            .collect::<Vec<_>>();
        (retained_hashes, deferred_hashes)
    }
    let (_seed_handle, seed_time_source) = TimeSource::new_mock(Duration::default());
    let (account_id, key_pair) = gen_account_in("wonderland");
    let first_template = accepted_tx_by(account_id.clone(), &key_pair, &seed_time_source);
    let second_template = accepted_tx_by(account_id, &key_pair, &seed_time_source);
    let (retained_normal, deferred_normal) =
        run_with_order(|_| {}, &first_template, &second_template);
    let (retained_reversed, deferred_reversed) =
        run_with_order(|guards| guards.reverse(), &first_template, &second_template);
    assert_eq!(
        retained_normal, retained_reversed,
        "lane gating should retain the same transactions regardless of guard order"
    );
    assert_eq!(
        deferred_normal, deferred_reversed,
        "lane gating should defer the same transactions regardless of guard order"
    );
}
#[cfg(feature = "telemetry")]
#[test]
fn enforce_lane_teu_limits_updates_telemetry_counters() {
    use std::num::NonZeroU32;
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let metrics = Arc::new(Metrics::default());
    let telemetry = StateTelemetry::new(metrics.clone(), true);
    let test_lane = LaneId::new(11);
    let test_dataspace = DataSpaceId::new(3);
    let test_dataspace_alias = "dataspace3";
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_tx = accepted_tx_in_dataspace_by_someone(test_dataspace_alias, &time_source);
    let lane_capacity = Queue::compute_teu_weight(&first_tx);
    assert!(lane_capacity > 0, "expected positive TEU weight");
    let lane_metadata = LaneConfig {
        id: test_lane,
        dataspace_id: test_dataspace,
        alias: "lane11".to_string(),
        metadata: BTreeMap::from([(
            "scheduler.teu_capacity".to_string(),
            lane_capacity.to_string(),
        )]),
        ..LaneConfig::default()
    };
    let dataspace_metadata = DataSpaceMetadata {
        id: test_dataspace,
        alias: test_dataspace_alias.to_string(),
        description: None,
        fault_tolerance: 1,
    };
    let lane_catalog = LaneCatalog::new(
        NonZeroU32::new(16).expect("nonzero lane count"),
        vec![lane_metadata.clone()],
    )
    .expect("valid lane catalog");
    let dataspace_catalog =
        DataSpaceCatalog::new(vec![dataspace_metadata.clone()]).expect("valid dataspace catalog");
    telemetry.set_nexus_catalogs(&lane_catalog, &dataspace_catalog);
    let mut state = State::with_telemetry(
        world_with_test_domains(),
        kura.clone(),
        query_handle.clone(),
        telemetry.clone(),
    );
    let mut nexus = state.nexus_snapshot();
    let state_lane_catalog = LaneCatalog::new(
        NonZeroU32::new(16).expect("nonzero lane count"),
        vec![LaneConfig::default(), lane_metadata.clone()],
    )
    .expect("valid state lane catalog");
    let state_dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        dataspace_metadata.clone(),
    ])
    .expect("valid state dataspace catalog");
    nexus.enabled = true;
    nexus.lane_catalog = state_lane_catalog;
    nexus.lane_config = LaneGeometry::from_catalog(&nexus.lane_catalog);
    nexus.dataspace_catalog = state_dataspace_catalog;
    nexus.routing_policy.default_lane = test_lane;
    nexus.routing_policy.default_dataspace = test_dataspace;
    state
        .set_nexus(nexus)
        .expect("apply telemetry test Nexus state");
    let state = Arc::new(state);
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: test_lane,
        dataspace: test_dataspace,
    });
    let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
    let queue_inner = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router,
        &[(test_lane, test_dataspace)],
    );
    *queue_inner.nexus_limits.write() = QueueLimits {
        fallback: scheduling,
        per_lane: BTreeMap::from([(test_lane, scheduling)]),
    };
    queue_inner
        .lane_teu_pending
        .insert(test_lane, PendingTeu::default());
    queue_inner
        .dataspace_teu_pending
        .insert((test_lane, test_dataspace), PendingTeu::default());
    let queue = Arc::new(queue_inner);
    let first_hash = first_tx.as_ref().hash();
    queue
        .push(first_tx, state.view())
        .expect("first push should succeed");
    let second_tx = accepted_tx_in_dataspace_by_someone(test_dataspace_alias, &time_source);
    let second_hash = second_tx.as_ref().hash();
    queue
        .push(second_tx, state.view())
        .expect("second push should succeed");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2usize));
    assert_eq!(guards.len(), 2);
    let deferred = queue.enforce_lane_teu_limits(&mut guards);
    assert_eq!(guards.len(), 1);
    assert_eq!(deferred.len(), 1);
    assert_eq!(guards[0].as_ref().hash(), first_hash);
    assert_eq!(deferred[0].as_ref().hash(), second_hash);
    let lane_label = test_lane.as_u32().to_string();
    let recorded = metrics
        .nexus_scheduler_lane_teu_deferral_total
        .with_label_values(&[lane_label.as_str(), "cap_exceeded"])
        .get();
    assert_eq!(recorded, lane_capacity);
    let lane_snapshots = metrics
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned");
    let snapshot = lane_snapshots
        .get(&test_lane.as_u32())
        .expect("lane snapshot missing");
    assert_eq!(snapshot.deferrals.cap_exceeded, lane_capacity);
}
#[cfg(feature = "telemetry")]
#[test]
fn queue_backlog_reports_available_lane_headroom() {
    use std::num::NonZeroU32;
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let metrics = Arc::new(Metrics::default());
    let telemetry = StateTelemetry::new(metrics.clone(), true);
    let test_lane = LaneId::new(13);
    let test_dataspace = DataSpaceId::new(9);
    let test_dataspace_alias = "dataspace9";
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_tx = accepted_tx_in_dataspace_by_someone(test_dataspace_alias, &time_source);
    let second_tx = accepted_tx_in_dataspace_by_someone(test_dataspace_alias, &time_source);
    let first_teu = Queue::compute_teu_weight(&first_tx);
    let second_teu = Queue::compute_teu_weight(&second_tx);
    let lane_capacity = first_teu.saturating_mul(10);
    assert!(
        lane_capacity > first_teu,
        "expected lane capacity to exceed TEU"
    );
    let lane_metadata = LaneConfig {
        id: test_lane,
        dataspace_id: test_dataspace,
        alias: "lane13".to_string(),
        metadata: BTreeMap::from([(
            "scheduler.teu_capacity".to_string(),
            lane_capacity.to_string(),
        )]),
        ..LaneConfig::default()
    };
    let dataspace_metadata = DataSpaceMetadata {
        id: test_dataspace,
        alias: test_dataspace_alias.to_string(),
        description: None,
        fault_tolerance: 1,
    };
    let lane_catalog = LaneCatalog::new(
        NonZeroU32::new(16).expect("nonzero lane count"),
        vec![lane_metadata.clone()],
    )
    .expect("valid lane catalog");
    let dataspace_catalog =
        DataSpaceCatalog::new(vec![dataspace_metadata.clone()]).expect("valid dataspace catalog");
    telemetry.set_nexus_catalogs(&lane_catalog, &dataspace_catalog);
    let mut state = State::with_telemetry(
        world_with_test_domains(),
        kura.clone(),
        query_handle.clone(),
        telemetry.clone(),
    );
    let mut nexus = state.nexus_snapshot();
    let state_lane_catalog = LaneCatalog::new(
        NonZeroU32::new(16).expect("nonzero lane count"),
        vec![LaneConfig::default(), lane_metadata.clone()],
    )
    .expect("valid state lane catalog");
    let state_dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        dataspace_metadata.clone(),
    ])
    .expect("valid state dataspace catalog");
    nexus.enabled = true;
    nexus.lane_catalog = state_lane_catalog;
    nexus.lane_config = LaneGeometry::from_catalog(&nexus.lane_catalog);
    nexus.dataspace_catalog = state_dataspace_catalog;
    nexus.routing_policy.default_lane = test_lane;
    nexus.routing_policy.default_dataspace = test_dataspace;
    state
        .set_nexus(nexus)
        .expect("apply backlog test Nexus state");
    let state = Arc::new(state);
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: test_lane,
        dataspace: test_dataspace,
    });
    let scheduling = LaneSchedulingLimits::new(lane_capacity, 0);
    let queue_inner = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router,
        &[(test_lane, test_dataspace)],
    );
    *queue_inner.nexus_limits.write() = QueueLimits {
        fallback: scheduling,
        per_lane: BTreeMap::from([(test_lane, scheduling)]),
    };
    queue_inner
        .lane_teu_pending
        .insert(test_lane, PendingTeu::default());
    queue_inner
        .dataspace_teu_pending
        .insert((test_lane, test_dataspace), PendingTeu::default());
    let queue = Arc::new(queue_inner);
    queue
        .push(first_tx, state.view())
        .expect("first push should succeed");
    queue
        .push(second_tx, state.view())
        .expect("second push should succeed");
    let pending_teu = first_teu.saturating_add(second_teu);
    let expected_committed = pending_teu.min(lane_capacity);
    let expected_headroom = lane_capacity.saturating_sub(expected_committed);
    let lane_label = test_lane.as_u32().to_string();
    let headroom_events = metrics
        .nexus_scheduler_lane_headroom_events_total
        .with_label_values(&[lane_label.as_str()])
        .get();
    assert_eq!(
        headroom_events, 0,
        "backlog snapshots should not emit warnings"
    );
    let lane_snapshots = metrics
        .nexus_scheduler_lane_teu_status
        .read()
        .expect("lane TEU cache poisoned");
    let snapshot = lane_snapshots
        .get(&test_lane.as_u32())
        .expect("lane snapshot missing");
    assert_eq!(snapshot.committed, expected_committed);
    assert_eq!(snapshot.buckets.headroom, expected_headroom);
}
