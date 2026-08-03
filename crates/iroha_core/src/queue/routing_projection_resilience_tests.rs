#[cfg(feature = "telemetry")]
#[tokio::test]
async fn push_records_teu_using_router_assignment() {
    struct StaticRouter {
        lane: LaneId,
        dataspace: DataSpaceId,
    }

    impl LaneRouter for StaticRouter {
        fn route(&self, _tx: &dyn TransactionRoutingView) -> RoutingDecision {
            RoutingDecision::new(self.lane, self.dataspace)
        }
    }

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let test_lane = LaneId::new(7);
    let test_dataspace = DataSpaceId::new(42);
    install_test_nexus_routes(&mut state, &[(test_lane, test_dataspace)]);
    {
        let mut nexus = state.nexus.write();
        nexus.routing_policy.default_lane = test_lane;
        nexus.routing_policy.default_dataspace = test_dataspace;
    }
    let state = Arc::new(state);
    let router = Arc::new(StaticRouter {
        lane: test_lane,
        dataspace: test_dataspace,
    });
    let queue = Arc::new(Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router,
        &[(test_lane, test_dataspace)],
    ));

    let (account_id, key_pair) = gen_account_in("wonderland");
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let domain_name = unique_test_domain_name("tagged");
    let unregister =
        Unregister::domain(DomainId::try_new(&domain_name, "test-dataspace-42").unwrap());
    let tx = TransactionBuilder::new_with_time_source(
        chain_id.clone(),
        account_id,
        &time_source,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([unregister])
    .sign(key_pair.private_key());
    let default_limits = TransactionParameters::default();
    let tx_limits = TransactionParameters::with_max_signatures(
        nonzero!(16_u64),
        nonzero!(4096_u64),
        nonzero!(1024_u64),
        default_limits.max_tx_bytes(),
        default_limits.max_decompressed_bytes(),
        default_limits.max_metadata_depth(),
    );
    let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
    let tx = AcceptedTransaction::accept(
        tx,
        &chain_id,
        Duration::from_secs(60),
        tx_limits,
        &crypto_cfg,
    )
    .expect("Failed to accept transaction.");
    let hash = tx.as_ref().hash();
    queue
        .push(tx, state.view())
        .expect("Failed to push tx into queue");

    let teu_info = queue
        .tx_teu
        .get(&hash)
        .expect("TEU info missing for routed transaction");
    assert_eq!(teu_info.lane_id, test_lane);
    assert_eq!(teu_info.dataspace_id, test_dataspace);
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn push_records_teu_from_ivm_metadata() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));

    let (account_id, key_pair) = gen_account_in("wonderland");
    let max_cycles = 42_000_u64;
    let tx = accepted_ivm_tx_by(account_id, &key_pair, &time_source, max_cycles);
    let hash = tx.as_ref().hash();

    queue
        .push(tx, state.view())
        .expect("Failed to enqueue IVM transaction");

    let info = queue
        .tx_teu
        .get(&hash)
        .expect("TEU info missing for IVM transaction");
    assert_eq!(info.teu, max_cycles);
}

#[tokio::test]
async fn block_events_carry_lane_metadata_from_queue() {
    struct TaggedRouter {
        lane: LaneId,
        dataspace: DataSpaceId,
    }

    impl LaneRouter for TaggedRouter {
        fn route(&self, _tx: &dyn TransactionRoutingView) -> RoutingDecision {
            RoutingDecision::new(self.lane, self.dataspace)
        }
    }

    let expected_lane = LaneId::new(5);
    let expected_dataspace = DataSpaceId::new(13);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    install_test_nexus_routes(&mut state, &[(expected_lane, expected_dataspace)]);
    let state = Arc::new(state);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::new(TaggedRouter {
            lane: expected_lane,
            dataspace: expected_dataspace,
        }),
        &[(expected_lane, expected_dataspace)],
    ));

    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    let routing = queue
        .push_with_lane(tx, state.view())
        .expect("Failed to enqueue transaction");
    assert_eq!(routing.lane_id, expected_lane);
    assert_eq!(routing.dataspace_id, expected_dataspace);
    assert!(
        queue.routing_decisions.contains_key(&hash),
        "routing decision missing from queue cache"
    );

    let state_view = state.view();
    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state_view, nonzero!(1_usize), &mut guards);
    drop(state_view);

    assert_eq!(guards.len(), 1);
    let cached = queue
        .routing_decisions
        .get(&hash)
        .map(|entry| *entry.value())
        .expect("routing cached");
    assert_eq!(cached.lane_id, expected_lane);
    assert_eq!(cached.dataspace_id, expected_dataspace);
    let transactions: Vec<_> = guards
        .iter()
        .map(TransactionGuard::clone_accepted)
        .collect();
    let ledger_entry =
        crate::queue::routing_ledger::take(&hash).expect("routing entry missing from ledger");
    assert_eq!(ledger_entry.lane_id, expected_lane);
    assert_eq!(ledger_entry.dataspace_id, expected_dataspace);
    crate::queue::routing_ledger::record(hash, ledger_entry);

    let new_block = BlockBuilder::new(transactions)
        .chain(0, None)
        .sign(ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let header = new_block.header();
    let signed_block: SignedBlock = new_block.into();
    let mut state_block = state.block(header);
    let valid_block = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});

    drop(guards);

    let ledger_before_event = crate::queue::routing_ledger::take(&hash)
        .expect("routing entry removed before event emission");
    assert_eq!(ledger_before_event.lane_id, expected_lane);
    assert_eq!(ledger_before_event.dataspace_id, expected_dataspace);
    crate::queue::routing_ledger::record(hash, ledger_before_event);

    let tx_event = valid_block
        .produce_events()
        .find_map(|event| match event {
            PipelineEventBox::Transaction(event) if event.hash() == &hash => Some(event),
            _ => None,
        })
        .expect("missing transaction event for routed transaction");

    assert_eq!(tx_event.lane_id(), expected_lane);
    assert_eq!(tx_event.dataspace_id(), expected_dataspace);
}

#[test]
fn proposal_pop_ignores_router_policy_drift_for_admitted_work() {
    let refreshed = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(10));
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    install_test_nexus_routes(
        &mut state,
        &[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (refreshed.lane_id, refreshed.dataspace_id),
        ],
    );
    let state = Arc::new(state);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router = Arc::new(MutableRouter::new(RoutingDecision::default()));
    let queue = Arc::new(Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router.clone(),
        &[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (refreshed.lane_id, refreshed.dataspace_id),
        ],
    ));

    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    queue.push(tx, state.view()).expect("push tx");
    assert_eq!(
        queue
            .routing_decisions
            .get(&hash)
            .map(|entry| *entry.value()),
        Some(RoutingDecision::default())
    );

    router.set(refreshed);
    let state_view = state.view();
    let mut expired = Vec::new();
    let guard = queue
        .pop_from_queue(&state_view, &mut expired)
        .expect("proposal pop should return admitted tx");
    drop(state_view);

    assert!(expired.is_empty());
    assert_eq!(guard.routing(), RoutingDecision::default());
    assert_eq!(
        queue
            .routing_decisions
            .get(&hash)
            .map(|entry| *entry.value()),
        Some(RoutingDecision::default())
    );
    assert_eq!(
        crate::queue::routing_ledger::get(&hash),
        Some(RoutingDecision::default())
    );

    let transactions = vec![guard.clone_accepted()];
    let new_block = BlockBuilder::new(transactions)
        .chain(0, None)
        .sign(ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let header = new_block.header();
    let signed_block: SignedBlock = new_block.into();
    let mut state_block = state.block(header);
    let valid_block = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});

    drop(guard);
    assert_eq!(
        crate::queue::routing_ledger::get(&hash),
        Some(RoutingDecision::default())
    );

    let tx_event = valid_block
        .produce_events()
        .find_map(|event| match event {
            PipelineEventBox::Transaction(event) if event.hash() == &hash => Some(event),
            _ => None,
        })
        .expect("missing transaction event for admitted routed transaction");

    assert_eq!(tx_event.lane_id(), LaneId::SINGLE);
    assert_eq!(tx_event.dataspace_id(), DataSpaceId::UNIVERSAL);
    let _ = crate::queue::routing_ledger::take(&hash);
}

#[test]
fn proposal_pop_ignores_replacement_router_failure_for_admitted_work() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    install_test_nexus_routes(&mut state, &[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router = Arc::new(MutableRouter::new(RoutingDecision::default()));
    let mut queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router.clone(),
        &[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)],
    );
    let (event_sender, mut event_receiver) = tokio::sync::broadcast::channel(8);
    queue.events_sender = event_sender;
    let queue = Arc::new(queue);

    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    queue.push(tx, state.view()).expect("push tx");
    router.set_error(RoutingResolveError::UnknownLane {
        lane_id: LaneId::new(99),
    });

    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);

    assert_eq!(guards.len(), 1);
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(guards[0].routing(), RoutingDecision::default());
    assert_eq!(
        queue
            .routing_decisions
            .get(&hash)
            .map(|entry| *entry.value()),
        Some(RoutingDecision::default())
    );
    assert_eq!(
        crate::queue::routing_ledger::get(&hash),
        Some(RoutingDecision::default())
    );
    assert!(!queue.accepted_work_validation_faulted());

    let mut saw_rejected = false;
    while let Ok(event) = event_receiver.try_recv() {
        let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = event else {
            continue;
        };
        if event.hash != hash {
            continue;
        }
        let TransactionStatus::Rejected(_) = &event.status else {
            continue;
        };
        saw_rejected = true;
        break;
    }
    assert!(
        !saw_rejected,
        "replacement-router failure must not reject already accepted work"
    );
}

#[test]
fn proposal_fee_drift_restores_fifo_and_retains_accepted_work() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let (authority, keypair) = gen_account_in("queue_fee_drift");
    let domain_id = DomainId::try_new("queue_fee_drift", "universal").expect("fee drift domain");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let fee_asset = AssetDefinitionId::derive_from_components(
        domain_id,
        "xor".parse().expect("fee drift asset name"),
    );
    let definition = AssetDefinition::numeric(
        fee_asset.clone(),
        "queue fee drift XOR".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let payer_asset_id = AssetId::new(fee_asset.clone(), authority.clone());
    let payer_asset = Asset::new(payer_asset_id.clone(), Quantity::from(10_u32));
    let world = World::with_assets([domain], [account], [definition], [payer_asset], []);
    let mut state = State::new(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    {
        let nexus = state.nexus.get_mut();
        nexus.enabled = true;
        nexus.fees.settlement_mode =
            iroha_config::parameters::actual::NexusFeeSettlementMode::Direct;
        nexus.fees.fee_asset_id = fee_asset.canonical_address();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
    }

    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            fee_asset,
            Quantity::from(1_u32),
        )],
        None,
    );
    let signed = TransactionBuilder::new_with_time_source(
        chain_id.clone(),
        authority,
        &time_source,
        fee_payment,
    )
    .with_instructions([InstructionBox::from(Log::new(
        Level::INFO,
        "fee drift".into(),
    ))])
    .sign(keypair.private_key());
    let default_limits = TransactionParameters::default();
    let tx_limits = TransactionParameters::with_max_signatures(
        nonzero!(16_u64),
        nonzero!(4096_u64),
        nonzero!(1024_u64),
        default_limits.max_tx_bytes(),
        default_limits.max_decompressed_bytes(),
        default_limits.max_metadata_depth(),
    );
    let tx = AcceptedTransaction::accept_with_time_source(
        signed,
        &chain_id,
        Duration::from_millis(10),
        tx_limits,
        &iroha_config::parameters::actual::Crypto::default(),
        &time_source,
    )
    .expect("accept fee drift transaction");
    let hash = tx.hash();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .push(tx, state.view())
        .expect("initial funded fee admission should succeed");

    let mut assets = state.world.assets.block();
    assets
        .remove(payer_asset_id)
        .expect("remove payer balance to force fee revalidation drift");
    assets.commit();
    let mut guards = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut guards);

    assert!(guards.is_empty());
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(queue.fifo_snapshot_locked(), vec![hash]);
    assert!(queue.txs.contains_key(&hash));
    assert!(queue.routing_plans.contains_key(&hash));
    assert!(queue.accepted_work_validation_faulted());
}

#[test]
fn expired_event_prefers_full_plan_over_divergent_legacy_route() {
    let expected = RoutingDecision::new(LaneId::new(5), DataSpaceId::new(13));
    let stale = RoutingDecision::default();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    install_test_nexus_routes(&mut state, &[(expected.lane_id, expected.dataspace_id)]);
    let state = Arc::new(state);
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut queue = Queue::test_with_router_for_routes(
        Config {
            transaction_time_to_live: Duration::from_millis(10),
            ..config_factory()
        },
        &time_source,
        Arc::new(MutableRouter::new(expected)),
        &[(expected.lane_id, expected.dataspace_id)],
    );
    let (event_sender, mut event_receiver) = tokio::sync::broadcast::channel(8);
    queue.events_sender = event_sender;
    let queue = Arc::new(queue);

    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    queue.push(tx, state.view()).expect("push tx");
    while event_receiver.try_recv().is_ok() {}

    queue.routing_decisions.insert(hash, stale);
    crate::queue::routing_ledger::record(hash, stale);
    queue
        .routing_plans
        .insert(hash, RoutingPlan::single(expected));
    crate::queue::routing_ledger::record_plan_bounded(
        hash,
        RoutingPlan::single(expected),
        queue.capacity.get(),
    );
    crate::queue::routing_ledger::record(hash, stale);

    time_handle.advance(Duration::from_millis(11));
    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);

    assert!(guards.is_empty());
    assert_eq!(queue.active_len(), 0);
    assert_eq!(crate::queue::routing_ledger::get_plan(&hash), None);
    assert_eq!(crate::queue::routing_ledger::get(&hash), None);

    let mut saw_expired = false;
    while let Ok(event) = event_receiver.try_recv() {
        let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = event else {
            continue;
        };
        if event.hash != hash {
            continue;
        }
        if !matches!(event.status, TransactionStatus::Expired) {
            continue;
        }
        assert_eq!(event.lane_id, expected.lane_id);
        assert_eq!(event.dataspace_id, expected.dataspace_id);
        saw_expired = true;
        break;
    }
    assert!(
        saw_expired,
        "expected expired event to carry full-plan route"
    );
}

#[test]
fn corrupt_route_indexes_retain_accepted_work_without_rejection() {
    let expected = RoutingDecision::new(LaneId::new(5), DataSpaceId::new(13));
    let stale = RoutingDecision::default();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    install_test_nexus_routes(&mut state, &[(expected.lane_id, expected.dataspace_id)]);
    let state = Arc::new(state);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router = Arc::new(MutableRouter::new(expected));
    let mut queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router.clone(),
        &[(expected.lane_id, expected.dataspace_id)],
    );
    let (event_sender, mut event_receiver) = tokio::sync::broadcast::channel(8);
    queue.events_sender = event_sender;
    let queue = Arc::new(queue);

    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    queue.push(tx, state.view()).expect("push tx");
    while event_receiver.try_recv().is_ok() {}

    queue.routing_decisions.insert(hash, stale);
    crate::queue::routing_ledger::record(hash, stale);
    queue
        .routing_plans
        .insert(hash, RoutingPlan::single(expected));
    crate::queue::routing_ledger::record_plan_bounded(
        hash,
        RoutingPlan::single(expected),
        queue.capacity.get(),
    );
    crate::queue::routing_ledger::record(hash, stale);
    router.set_error(RoutingResolveError::UnknownLane {
        lane_id: LaneId::new(99),
    });

    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);

    assert!(guards.is_empty());
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(
        crate::queue::routing_ledger::get_plan(&hash),
        Some(RoutingPlan::single(expected))
    );
    assert_eq!(crate::queue::routing_ledger::get(&hash), Some(stale));
    assert!(queue.accepted_work_validation_faulted());

    let mut saw_rejected = false;
    while let Ok(event) = event_receiver.try_recv() {
        let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = event else {
            continue;
        };
        if event.hash != hash {
            continue;
        }
        let TransactionStatus::Rejected(_) = &event.status else {
            continue;
        };
        saw_rejected = true;
        break;
    }
    assert!(
        !saw_rejected,
        "accepted work with corrupt routing indexes must not be rejected or tombstoned"
    );
}

#[tokio::test]
async fn dropping_transaction_guard_clears_removed_hashes() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));

    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("Failed to push tx into queue");

    let mut expired_transactions = Vec::new();
    let state_view = state.view();
    let guard = queue
        .pop_from_queue(&state_view, &mut expired_transactions)
        .expect("Expected a transaction guard");
    assert!(expired_transactions.is_empty());

    drop(guard);

    assert!(queue.removed_hashes.is_empty());
}
