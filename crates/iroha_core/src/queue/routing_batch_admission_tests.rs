#[test]
fn push_with_gossip_payload_with_state_and_routing_validates_precomputed_plan() {
    struct CountingRouter {
        calls: Arc<AtomicUsize>,
    }
    impl LaneRouter for CountingRouter {
        fn try_route(
            &self,
            _tx: &dyn TransactionRoutingView,
        ) -> Result<RoutingDecision, RoutingResolveError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(RoutingDecision::new(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
            ))
        }
    }
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let calls = Arc::new(AtomicUsize::new(0));
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(CountingRouter {
            calls: Arc::clone(&calls),
        }),
    );
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash_as_entrypoint();
    let payload = tx.entrypoint_bytes();
    queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            tx,
            state.as_ref(),
            RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)),
            Some(Arc::clone(&payload)),
        )
        .expect("push with precomputed routing should succeed");
    assert!(
        calls.load(Ordering::Relaxed) > 0,
        "precomputed plan admission should validate against current routing"
    );
    let routing = queue
        .routing_plans
        .get(&hash)
        .expect("routing plan should exist")
        .coordinator_route();
    assert_eq!(routing.lane_id, LaneId::SINGLE);
    assert_eq!(routing.dataspace_id, DataSpaceId::UNIVERSAL);
    assert_eq!(
        queue.tx_gossip.pop(),
        Some(hash),
        "successful gossip admission should still enqueue the gossip side channel"
    );
}
struct NativeAmxParticipantDriftFixture {
    state: State,
    tx: AcceptedTransaction<'static>,
    stale_plan: RoutingPlan,
    current_plan: RoutingPlan,
}
fn native_amx_participant_drift_fixture(
    time_source: &TimeSource,
) -> NativeAmxParticipantDriftFixture {
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let first_dataspace = DataSpaceId::new(7);
    let second_dataspace = DataSpaceId::new(8);
    let policy = LaneRoutingPolicy {
        default_lane: LaneId::SINGLE,
        default_dataspace: DataSpaceId::UNIVERSAL,
        rules: vec![],
    };
    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata {
            id: DataSpaceId::UNIVERSAL,
            alias: "universal".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: first_dataspace,
            alias: "acme".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: second_dataspace,
            alias: "bank".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    let stale_lane_catalog = LaneCatalog::new(
        nonzero!(4_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                dataspace_id: first_dataspace,
                alias: "acme-primary".to_owned(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: LaneId::new(2),
                dataspace_id: second_dataspace,
                alias: "bank-primary".to_owned(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: LaneId::new(3),
                dataspace_id: second_dataspace,
                alias: "bank-secondary".to_owned(),
                ..LaneConfig::default()
            },
        ],
    )
    .expect("stale lane catalog");
    let mut current_lanes = stale_lane_catalog.lanes().to_vec();
    let stale_participant_lane = current_lanes
        .iter_mut()
        .find(|lane| lane.id == LaneId::new(2))
        .expect("stale participant lane");
    stale_participant_lane.alias = "elastic-lane-2".to_owned();
    stale_participant_lane
        .metadata
        .insert(AUTOSCALE_META_MANAGED.to_string(), "true".to_string());
    stale_participant_lane
        .metadata
        .insert(AUTOSCALE_META_CREATED_HEIGHT.to_string(), "10".to_string());
    crate::state::attach_synthetic_autoscale_committee_for_test(stale_participant_lane);
    let current_lane_catalog = LaneCatalog::new(stale_lane_catalog.lane_count(), current_lanes)
        .expect("current lane catalog");
    {
        let nexus = state.nexus.get_mut();
        nexus.routing_policy = policy.clone();
        nexus.lane_catalog = current_lane_catalog.clone();
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        nexus.dataspace_catalog = dataspace_catalog.clone();
    }
    let (authority_id, authority_keypair) = gen_account_in("wonderland");
    register_test_authority(&mut state, &authority_id);
    let tx = accepted_tx_with(
        authority_id,
        &authority_keypair,
        time_source,
        vec![
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("merchant", "acme").expect("domain id"),
            ))),
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("treasury", "bank").expect("domain id"),
            ))),
        ],
        Metadata::default(),
    );
    let stale_plan = ConfigLaneRouter::new(
        policy.clone(),
        dataspace_catalog.clone(),
        stale_lane_catalog,
    )
    .try_route_plan(&tx)
    .expect("stale Native AMX plan should resolve");
    let current_plan = ConfigLaneRouter::new(policy, dataspace_catalog, current_lane_catalog)
        .try_route_plan(&tx)
        .expect("current Native AMX plan should resolve");
    NativeAmxParticipantDriftFixture {
        state,
        tx,
        stale_plan,
        current_plan,
    }
}
#[test]
fn reconfigure_nexus_with_state_fails_closed_on_corrupt_native_amx_plan_index() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut fixture = native_amx_participant_drift_fixture(&time_source);
    {
        let nexus = fixture.state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
    }
    assert_eq!(
        fixture.stale_plan.coordinator_route(),
        fixture.current_plan.coordinator_route()
    );
    assert_ne!(fixture.stale_plan, fixture.current_plan);
    let queue = Queue::test(config_factory(), &time_source);
    let hash = fixture.tx.hash_as_entrypoint();
    queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            fixture.tx.clone(),
            &fixture.state,
            fixture.current_plan.clone(),
            None,
        )
        .expect("current Native AMX plan should enqueue");
    queue.routing_plans.insert(hash, fixture.stale_plan.clone());
    let nexus = fixture.state.nexus_snapshot();
    queue.reconfigure_nexus_with_state(&nexus, &fixture.state, None);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().coordinator_route()),
        Some(fixture.stale_plan.coordinator_route())
    );
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().clone()),
        Some(fixture.stale_plan.clone())
    );
    assert_eq!(queue.routing_plan_hint(&hash), Some(fixture.stale_plan));
    assert!(queue.accepted_work_validation_faulted());
}
#[test]
fn reconfigure_nexus_with_view_fails_closed_on_corrupt_native_amx_plan_index() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut fixture = native_amx_participant_drift_fixture(&time_source);
    {
        let nexus = fixture.state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
    }
    assert_eq!(
        fixture.stale_plan.coordinator_route(),
        fixture.current_plan.coordinator_route()
    );
    assert_ne!(fixture.stale_plan, fixture.current_plan);
    let queue = Queue::test(config_factory(), &time_source);
    let hash = fixture.tx.hash_as_entrypoint();
    queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            fixture.tx.clone(),
            &fixture.state,
            fixture.current_plan.clone(),
            None,
        )
        .expect("current Native AMX plan should enqueue");
    queue.routing_plans.insert(hash, fixture.stale_plan.clone());
    let nexus = fixture.state.nexus_snapshot();
    let state_view = fixture.state.view();
    queue.reconfigure_nexus(&nexus, &state_view, None);
    drop(state_view);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().coordinator_route()),
        Some(fixture.stale_plan.coordinator_route())
    );
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().clone()),
        Some(fixture.stale_plan.clone())
    );
    assert_eq!(queue.routing_plan_hint(&hash), Some(fixture.stale_plan));
    assert!(queue.accepted_work_validation_faulted());
}
#[test]
fn proposal_pop_restores_fifo_and_fails_closed_on_corrupt_native_amx_plan_index() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut fixture = native_amx_participant_drift_fixture(&time_source);
    {
        let nexus = fixture.state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
    }
    assert_eq!(
        fixture.stale_plan.coordinator_route(),
        fixture.current_plan.coordinator_route()
    );
    assert_ne!(fixture.stale_plan, fixture.current_plan);
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let hash = fixture.tx.hash_as_entrypoint();
    queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            fixture.tx.clone(),
            &fixture.state,
            fixture.current_plan.clone(),
            None,
        )
        .expect("current Native AMX plan should enqueue");
    let second = accepted_tx_by_someone(&time_source);
    let second_authority = second
        .external()
        .expect("later FIFO transaction should be external")
        .authority()
        .clone();
    register_test_authority(&mut fixture.state, &second_authority);
    let second_hash = second.hash_as_entrypoint();
    queue
        .push(second, fixture.state.view())
        .expect("enqueue later FIFO transaction");
    queue.routing_plans.insert(hash, fixture.stale_plan.clone());
    let state_view = fixture.state.view();
    let mut expired = Vec::new();
    let guard = queue.pop_from_queue(&state_view, &mut expired);
    drop(state_view);
    assert!(expired.is_empty());
    assert!(guard.is_none());
    assert_eq!(queue.active_len(), 2);
    assert_eq!(queue.queued_len(), 2);
    assert_eq!(
        queue.fifo_snapshot_locked(),
        vec![hash, second_hash],
        "failed selection must restore the original hash ahead of later FIFO ownership"
    );
    assert!(queue.accepted_work_validation_faulted());
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().clone()),
        Some(fixture.stale_plan.clone())
    );
    assert_eq!(queue.routing_plan_hint(&hash), Some(fixture.stale_plan));
}
#[test]
fn push_with_gossip_payload_with_state_and_routing_rejects_native_amx_participant_drift() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let fixture = native_amx_participant_drift_fixture(&time_source);
    assert_eq!(
        fixture.stale_plan.coordinator_route(),
        fixture.current_plan.coordinator_route()
    );
    assert_ne!(fixture.stale_plan.digest(), fixture.current_plan.digest());
    let queue = Queue::test(config_factory(), &time_source);
    let hash = fixture.tx.hash_as_entrypoint();
    let payload = fixture.tx.entrypoint_bytes();
    let err = queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            fixture.tx.clone(),
            &fixture.state,
            fixture.stale_plan,
            Some(Arc::clone(&payload)),
        )
        .expect_err("direct admission must reject stale Native AMX participant legs");
    assert!(
        matches!(
            &err,
            Failure {
                err: Error::UnresolvedRoute { .. },
                ..
            }
        ),
        "unexpected direct admission rejection: {err:?}"
    );
    if let Error::UnresolvedRoute { reason } = &err.err {
        assert!(
            reason.contains("does not match the current Nexus routing policy")
                || reason.contains("not active"),
            "stale-plan rejection should explain current-policy or active-height mismatch: {reason}"
        );
    }
    assert_eq!(
        queue
            .route_plan_with_state(&fixture.tx, &fixture.state)
            .expect("current Native AMX plan should resolve"),
        fixture.current_plan
    );
    assert!(!queue.txs.contains_key(&hash));
    assert!(queue.routing_plans.get(&hash).is_none());
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    assert!(
        queue.tx_gossip.pop().is_none(),
        "rejected direct admission must not publish gossip notifications"
    );
}
#[test]
fn push_with_gossip_payload_with_state_and_routing_rejects_future_created_autoscale_plan() {
    let NexusRoutingFixture {
        mut state,
        authority_id,
        authority_keypair,
        ..
    } = nexus_routing_fixture();
    let mut future_elastic = LaneConfig {
        id: LaneId::new(1),
        alias: "elastic-lane-1".to_owned(),
        dataspace_id: DataSpaceId::UNIVERSAL,
        visibility: LaneVisibility::Public,
        ..LaneConfig::default()
    };
    future_elastic
        .metadata
        .insert(AUTOSCALE_META_MANAGED.to_owned(), "true".to_owned());
    future_elastic
        .metadata
        .insert(AUTOSCALE_META_CREATED_HEIGHT.to_owned(), "7".to_owned());
    crate::state::attach_synthetic_autoscale_committee_for_test(&mut future_elastic);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lane_id = nonzero!(1_u32);
        nexus.autoscale.max_lane_id_exclusive = nonzero!(8_u32);
        nexus.lane_catalog =
            LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), future_elastic])
                .expect("future-created lane catalog");
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    }
    seed_committed_height_for_queue_test(&state, 6);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_with(
        authority_id,
        &authority_keypair,
        &time_source,
        vec![InstructionBox::from(Log::new(
            Level::INFO,
            "forged future autoscale plan".into(),
        ))],
        Metadata::default(),
    );
    let hash = tx.hash_as_entrypoint();
    let forged_plan =
        RoutingPlan::single(RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL));
    assert_eq!(
        queue
            .route_plan_with_state(&tx, &state)
            .expect("live route should resolve")
            .coordinator_route(),
        RoutingDecision::default(),
        "live routing must not select the future-created elastic lane"
    );
    let err = queue
        .push_with_gossip_payload_with_state_and_routing_plan(tx, &state, forged_plan, None)
        .expect_err("forged future-created autoscale plan must reject");
    assert!(
        matches!(
            &err,
            Failure {
                err: Error::UnresolvedRoute { .. },
                ..
            }
        ),
        "unexpected future-created plan rejection: {err:?}"
    );
    if let Error::UnresolvedRoute { reason } = &err.err {
        assert!(
            reason.contains("not active"),
            "future-created plan rejection should explain active-height mismatch: {reason}"
        );
    }
    assert!(!queue.txs.contains_key(&hash));
    assert!(queue.routing_plans.get(&hash).is_none());
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(
        queue.routing_plan_hint(&hash),
        None,
        "rejected future-created plan must not enter the queue plan index"
    );
    assert!(
        queue.tx_gossip.pop().is_none(),
        "rejected direct admission must not publish gossip notifications"
    );
}
#[test]
fn batch_push_with_precomputed_routing_rejects_native_amx_participant_drift() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let fixture = native_amx_participant_drift_fixture(&time_source);
    assert_eq!(
        fixture.stale_plan.coordinator_route(),
        fixture.current_plan.coordinator_route()
    );
    assert_ne!(fixture.stale_plan.digest(), fixture.current_plan.digest());
    let queue = Queue::test(config_factory(), &time_source);
    let hash = fixture.tx.hash_as_entrypoint();
    let err = queue
        .push_batch_with_lane_with_state_and_routing_plans(
            vec![(fixture.tx.clone(), fixture.stale_plan)],
            &fixture.state,
        )
        .expect_err("batch admission must reject stale Native AMX participant legs");
    assert!(
        matches!(
            &err,
            Failure {
                err: Error::UnresolvedRoute { .. },
                ..
            }
        ),
        "unexpected batch rejection: {err:?}"
    );
    if let Error::UnresolvedRoute { reason } = &err.err {
        assert!(
            reason.contains("does not match the current Nexus routing policy")
                || reason.contains("not active"),
            "stale-plan rejection should explain current-policy or active-height mismatch: {reason}"
        );
    }
    assert_eq!(
        queue
            .route_plan_with_state(&fixture.tx, &fixture.state)
            .expect("current Native AMX plan should resolve"),
        fixture.current_plan
    );
    assert!(!queue.txs.contains_key(&hash));
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    assert!(
        queue.tx_gossip.pop().is_none(),
        "rejected batch must not publish gossip notifications"
    );
}
#[test]
fn batch_push_with_precomputed_routing_enqueues_in_order_without_side_payload_cache() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let routing = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let first = accepted_tx_by_someone(&time_source);
    let first_hash = first.as_ref().hash_as_entrypoint();
    let first_payload = first.entrypoint_bytes();
    time_handle.advance(Duration::from_millis(1));
    let second = accepted_tx_by_someone(&time_source);
    let second_hash = second.as_ref().hash_as_entrypoint();
    let second_payload = second.entrypoint_bytes();
    let accepted = queue
        .push_batch_with_lane_with_state_and_routing_plans(
            vec![
                (first, RoutingPlan::single(routing)),
                (second, RoutingPlan::single(routing)),
            ],
            &state,
        )
        .expect("batch should be accepted");
    assert_eq!(accepted, 2);
    assert_eq!(queue.active_len(), 2);
    assert_eq!(queue.queued_len(), 2);
    assert_eq!(queue.current_backpressure().queued(), 2);
    let batch = queue.gossip_batch_with_state(2, &state);
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].tx.as_ref().hash_as_entrypoint(), first_hash);
    assert_eq!(batch[0].payload.as_slice(), first_payload.as_slice());
    assert!(Arc::ptr_eq(&batch[0].payload, &first_payload));
    assert_eq!(batch[0].routing, routing);
    assert_eq!(batch[1].tx.as_ref().hash_as_entrypoint(), second_hash);
    assert_eq!(batch[1].payload.as_slice(), second_payload.as_slice());
    assert!(Arc::ptr_eq(&batch[1].payload, &second_payload));
    assert_eq!(batch[1].routing, routing);
}
#[test]
fn batch_push_duplicate_matches_single_push_prefix_semantics() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let routing = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash_as_entrypoint();
    let result = queue.push_batch_with_lane_with_state_and_routing_plans(
        vec![
            (tx.clone(), RoutingPlan::single(routing)),
            (tx, RoutingPlan::single(routing)),
        ],
        &state,
    );
    assert!(
        matches!(
            result,
            Err(Failure {
                err: Error::IsInQueue,
                ..
            })
        ),
        "unexpected duplicate result: {result:?}"
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(queue.current_backpressure().queued(), 1);
    assert_eq!(
        queue.tx_gossip.pop(),
        Some(hash),
        "accepted prefix should publish one gossip notification"
    );
    assert!(
        queue.tx_gossip.pop().is_none(),
        "duplicate suffix must not publish a notification"
    );
    let batch = queue.gossip_batch_with_state(2, &state);
    assert_eq!(batch.len(), 0);
}
#[test]
fn batch_push_full_queue_preserves_successful_prefix() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut cfg = config_factory();
    cfg.capacity = nonzero!(1_usize);
    let queue = Queue::test(cfg, &time_source);
    let routing = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let first = accepted_tx_by_someone(&time_source);
    let first_hash = first.as_ref().hash_as_entrypoint();
    time_handle.advance(Duration::from_millis(1));
    let second = accepted_tx_by_someone(&time_source);
    let result = queue.push_batch_with_lane_with_state_and_routing_plans(
        vec![
            (first, RoutingPlan::single(routing)),
            (second, RoutingPlan::single(routing)),
        ],
        &state,
    );
    assert!(
        matches!(
            result,
            Err(Failure {
                err: Error::Full,
                ..
            })
        ),
        "unexpected full-queue result: {result:?}"
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert!(queue.current_backpressure().is_saturated());
    let batch = queue.gossip_batch_with_state(2, &state);
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].tx.as_ref().hash_as_entrypoint(), first_hash);
}
#[test]
fn batch_push_per_user_limit_preserves_successful_prefix() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut cfg = config_factory();
    cfg.capacity_per_user = nonzero!(1_usize);
    let queue = Queue::test(cfg, &time_source);
    let routing = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (account_id, key_pair) = gen_account_in("wonderland");
    let first = accepted_tx_by(account_id.clone(), &key_pair, &time_source);
    let first_hash = first.as_ref().hash_as_entrypoint();
    time_handle.advance(Duration::from_millis(1));
    let second = accepted_tx_by(account_id.clone(), &key_pair, &time_source);
    let result = queue.push_batch_with_lane_with_state_and_routing_plans(
        vec![
            (first, RoutingPlan::single(routing)),
            (second, RoutingPlan::single(routing)),
        ],
        &state,
    );
    assert!(
        matches!(
            result,
            Err(Failure {
                err: Error::MaximumTransactionsPerUser,
                ..
            })
        ),
        "unexpected per-user result: {result:?}"
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(queue.queued_tx_count_for_user(&account_id), 1);
    assert_eq!(queue.current_backpressure().queued(), 1);
    let batch = queue.gossip_batch_with_state(2, &state);
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].tx.as_ref().hash_as_entrypoint(), first_hash);
}
