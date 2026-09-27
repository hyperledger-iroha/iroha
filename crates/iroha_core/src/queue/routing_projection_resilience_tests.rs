#[cfg(feature = "telemetry")]
#[tokio::test]
async fn push_records_teu_using_router_assignment() {
    struct StaticRouter {
        lane: LaneId,
        dataspace: DataSpaceId,
    }
    impl LaneRouter for StaticRouter {
        fn try_route(
            &self,
            _tx: &dyn TransactionRoutingView,
        ) -> Result<RoutingDecision, RoutingResolveError> {
            Ok(RoutingDecision::new(self.lane, self.dataspace))
        }
    }
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let test_lane = LaneId::new(7);
    let test_dataspace = DataSpaceId::new(42);
    let mut nexus = test_nexus_for_routes(&[(test_lane, test_dataspace)]);
    nexus.routing_policy.default_lane = test_lane;
    nexus.routing_policy.default_dataspace = test_dataspace;
    let state = State::new_with_nexus_for_testing(
        world_with_test_domains(),
        nexus,
        LiveQueryStore::start_test(),
    );
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
    register_test_authority(&state, &account_id);
    let domain_name = unique_test_domain_name("tagged");
    let unregister =
        Unregister::domain(DomainId::try_new(&domain_name, "test-dataspace-42").unwrap());
    let tx = TransactionBuilder::new_with_time_source(
        state.network_id,
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
    let tx = AcceptedTransaction::accept_with_time_source(
        tx,
        state.network_id_ref(),
        Duration::from_secs(60),
        tx_limits,
        &crypto_cfg,
        &time_source,
    )
    .expect("Failed to accept transaction.");
    let hash = tx.as_ref().hash_as_entrypoint();
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
    register_test_authority(&state, &account_id);
    let max_cycles = 42_000_u64;
    let tx = accepted_ivm_tx_by(account_id, &key_pair, &time_source, max_cycles);
    let hash = tx.as_ref().hash_as_entrypoint();
    queue
        .push(tx, state.view())
        .expect("Failed to enqueue IVM transaction");
    let info = queue
        .tx_teu
        .get(&hash)
        .expect("TEU info missing for IVM transaction");
    assert_eq!(info.teu, max_cycles);
}
/// Seed structural predecessor history for ordinary routing/event execution.
///
/// The actual canonical body, State hash/membership and runtime sample agree,
/// and DA/Time readers use that body. This fixture does not execute genesis or
/// claim its finality; only the following queued Network work is executed.
#[inline(never)]
fn committed_queue_event_predecessor(state: &State) -> Arc<SignedBlock> {
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura().blocks_count(), 0);
    let parent = Arc::new(
        iroha_data_model::block::builder::BlockBuilder::new(
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, 0, 0),
        )
        .build_with_signature(0, ALICE_KEYPAIR.private_key()),
    );
    let wire = parent.encode_wire().expect("encode canonical predecessor");
    state
        .kura()
        .store_block(Arc::clone(&parent))
        .expect("persist actual predecessor body");
    let mut metadata = state.block(parent.header());
    metadata
        .evaluate_nexus_autoscale(&parent, 0)
        .expect("stage exact empty predecessor runtime sample");
    metadata
        .commit_empty_block_for_testing()
        .expect("commit structural predecessor metadata");
    // The metadata overlay first hydrated the empty WSV prefix. Rebuild through
    // the real reader now that the exact stored predecessor is committed.
    state
        .rewind_da_indexes_to_height(1)
        .expect("rebuild DA indexes from the actual committed predecessor");
    let stored = state
        .block_by_height(nonzero!(1_usize))
        .expect("canonical State predecessor is available from Kura");
    assert_eq!(stored.header(), parent.header());
    assert_eq!(stored.hash(), parent.hash());
    assert_eq!(stored.encode_wire().unwrap(), wire);
    assert_eq!(state.latest_block_hash_fast(), Some(parent.hash()));
    assert_eq!(state.committed_height(), 1);
    stored
}

#[tokio::test]
async fn block_events_carry_committed_lane_metadata_after_queue_pop() {
    struct TaggedRouter {
        lane: LaneId,
        dataspace: DataSpaceId,
    }
    impl LaneRouter for TaggedRouter {
        fn try_route(
            &self,
            _tx: &dyn TransactionRoutingView,
        ) -> Result<RoutingDecision, RoutingResolveError> {
            Ok(RoutingDecision::new(self.lane, self.dataspace))
        }
    }
    let expected_lane = LaneId::new(5);
    let expected_dataspace = DataSpaceId::new(13);
    let state = State::new_with_nexus_for_testing(
        world_with_test_domains(),
        test_nexus_for_routes(&[(expected_lane, expected_dataspace)]),
        LiveQueryStore::start_test(),
    );
    let parent = committed_queue_event_predecessor(&state);
    let parent_height = parent.header().height().get();
    let parent_hash = parent.hash();
    let parent_wire = parent.encode_wire().unwrap();
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
    let signed_hash = tx.as_ref().hash();
    let hash = tx.as_ref().hash_as_entrypoint();
    let routing = queue
        .push_with_lane(tx, state.view())
        .expect("Failed to enqueue transaction");
    assert_eq!(routing.lane_id, expected_lane);
    assert_eq!(routing.dataspace_id, expected_dataspace);
    assert!(
        queue.routing_plans.contains_key(&hash),
        "routing decision missing from queue cache"
    );
    let state_view = state.view();
    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state_view, nonzero!(1_usize), &mut guards);
    drop(state_view);
    assert_eq!(guards.len(), 1);
    let cached = queue
        .routing_plans
        .get(&hash)
        .map(|entry| entry.value().coordinator_route())
        .expect("routing cached");
    assert_eq!(cached.lane_id, expected_lane);
    assert_eq!(cached.dataspace_id, expected_dataspace);
    let transactions: Vec<_> = guards
        .iter()
        .map(TransactionGuard::clone_accepted)
        .collect();
    let indexed_plan = queue
        .routing_plan_hint(&hash)
        .expect("routing entry missing from queue plan index");
    assert_eq!(indexed_plan.coordinator_route().lane_id, expected_lane);
    assert_eq!(
        indexed_plan.coordinator_route().dataspace_id,
        expected_dataspace
    );
    let execution_context = iroha_data_model::block::BlockExecutionContextBundle::new(
        transactions
            .iter()
            .map(|tx| {
                ExternalExecutionContext::new(
                    tx.hash_as_entrypoint(),
                    expected_lane,
                    expected_dataspace,
                )
            })
            .collect(),
    );
    let new_block = BlockBuilder::new(transactions)
        .chain(0, Some(&parent))
        .with_execution_context(Some(execution_context))
        .sign(ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let header = new_block.header();
    assert_eq!(header.height().get(), parent_height + 1);
    assert_eq!(header.prev_block_hash(), Some(parent_hash));
    let signed_block: SignedBlock = new_block.into();
    let generation = state.state_view_generation();
    let mut state_block = state.block(header);
    let valid_block = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    drop(guards);
    let tx_event = valid_block
        .produce_events()
        .find_map(|event| match event {
            PipelineEventBox::Transaction(event) if event.hash() == &signed_hash => Some(event),
            _ => None,
        })
        .expect("missing transaction event for routed transaction");
    assert_eq!(tx_event.lane_id(), expected_lane);
    assert_eq!(tx_event.dataspace_id(), expected_dataspace);
    drop(state_block);
    assert_eq!(
        u64::try_from(state.committed_height()).unwrap(),
        parent_height
    );
    assert_eq!(state.latest_block_hash_fast(), Some(parent_hash));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(state.kura().blocks_count(), 1);
    assert_eq!(
        state
            .block_by_height(nonzero!(1_usize))
            .unwrap()
            .encode_wire()
            .unwrap(),
        parent_wire,
        "candidate execution cannot change its stored predecessor"
    );
}
#[test]
fn proposal_pop_routes_ordinary_input_from_committed_policy() {
    let refreshed = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(10));
    let state = State::new_with_nexus_for_testing(
        world_with_test_domains(),
        test_nexus_for_routes(&[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            (refreshed.lane_id, refreshed.dataspace_id),
        ]),
        LiveQueryStore::start_test(),
    );
    let parent = committed_queue_event_predecessor(&state);
    let parent_height = parent.header().height().get();
    let parent_hash = parent.hash();
    let parent_wire = parent.encode_wire().unwrap();
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
    let signed_hash = tx.as_ref().hash();
    let hash = tx.as_ref().hash_as_entrypoint();
    queue.push(tx, state.view()).expect("push tx");
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().coordinator_route()),
        Some(RoutingDecision::default())
    );
    router.set(refreshed);
    let mut committed_nexus = state.nexus_snapshot();
    committed_nexus.routing_policy.default_lane = refreshed.lane_id;
    committed_nexus.routing_policy.default_dataspace = refreshed.dataspace_id;
    *state.nexus.write() = committed_nexus;
    let state_view = state.view();
    let mut expired = Vec::new();
    let guard = queue
        .pop_from_queue(&state_view, &mut expired)
        .expect("proposal pop should return admitted tx");
    drop(state_view);
    assert!(expired.is_empty());
    assert_eq!(guard.routing(), refreshed);
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().coordinator_route()),
        Some(RoutingDecision::default())
    );
    assert_eq!(
        queue
            .routing_plan_hint(&hash)
            .map(|plan| plan.coordinator_route()),
        Some(RoutingDecision::default())
    );
    let transactions = vec![guard.clone_accepted()];
    let execution_context = iroha_data_model::block::BlockExecutionContextBundle::new(vec![
        ExternalExecutionContext::new(hash, guard.routing().lane_id, guard.routing().dataspace_id),
    ]);
    let new_block = BlockBuilder::new(transactions)
        .chain(0, Some(&parent))
        .with_execution_context(Some(execution_context))
        .sign(ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let header = new_block.header();
    assert_eq!(header.height().get(), parent_height + 1);
    assert_eq!(header.prev_block_hash(), Some(parent_hash));
    let signed_block: SignedBlock = new_block.into();
    let generation = state.state_view_generation();
    let mut state_block = state.block(header);
    let valid_block = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    drop(guard);
    assert_eq!(
        queue
            .routing_plan_hint(&hash)
            .map(|plan| plan.coordinator_route()),
        None
    );
    let tx_event = valid_block
        .produce_events()
        .find_map(|event| match event {
            PipelineEventBox::Transaction(event) if event.hash() == &signed_hash => Some(event),
            _ => None,
        })
        .expect("missing transaction event for admitted routed transaction");
    assert_eq!(tx_event.lane_id(), refreshed.lane_id);
    assert_eq!(tx_event.dataspace_id(), refreshed.dataspace_id);
    drop(state_block);
    assert_eq!(
        u64::try_from(state.committed_height()).unwrap(),
        parent_height
    );
    assert_eq!(state.latest_block_hash_fast(), Some(parent_hash));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(state.kura().blocks_count(), 1);
    assert_eq!(
        state
            .block_by_height(nonzero!(1_usize))
            .unwrap()
            .encode_wire()
            .unwrap(),
        parent_wire,
        "candidate execution cannot change its stored predecessor"
    );
}
#[test]
fn proposal_pop_defers_unavailable_ordinary_route_without_fault() {
    let state = State::new_with_nexus_for_testing(
        world_with_test_domains(),
        test_nexus_for_routes(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]),
        LiveQueryStore::start_test(),
    );
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
    let signed_hash = tx.as_ref().hash();
    let hash = tx.as_ref().hash_as_entrypoint();
    queue.push(tx, state.view()).expect("push tx");
    router.set_error(RoutingResolveError::UnknownLane {
        lane_id: LaneId::new(99),
    });
    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
    assert!(guards.is_empty());
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().coordinator_route()),
        Some(RoutingDecision::default())
    );
    assert_eq!(
        queue
            .routing_plan_hint(&hash)
            .map(|plan| plan.coordinator_route()),
        Some(RoutingDecision::default())
    );
    assert!(!queue.accepted_work_validation_faulted());
    let mut saw_rejected = false;
    while let Ok(event) = event_receiver.try_recv() {
        let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = event else {
            continue;
        };
        if event.hash != signed_hash {
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
    router.set(RoutingDecision::default());
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
    assert_eq!(guards.len(), 1);
    assert_eq!(guards[0].routing(), RoutingDecision::default());
}
#[test]
fn proposal_fee_drift_restores_fifo_and_retains_accepted_work() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let (authority, keypair) = gen_account_in("queue_fee_drift");
    let domain_id = DomainId::try_new("queue_fee_drift", "universal").expect("fee drift domain");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let universal_domain =
        Domain::new(DomainId::try_new("universal", "universal").expect("canonical XOR domain"))
            .build(&authority);
    let fee_asset: AssetDefinitionId =
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .expect("canonical configured XOR asset");
    let definition = AssetDefinition::numeric(
        fee_asset.clone(),
        "queue fee drift XOR".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let payer_asset_id = AssetId::new(fee_asset.clone(), authority.clone());
    let payer_asset = Asset::new(payer_asset_id.clone(), Quantity::from(10_u32));
    let world = World::with_assets(
        [domain, universal_domain],
        [account],
        [definition],
        [payer_asset],
        [],
    );
    let mut nexus = test_nexus_for_routes(&[(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]);
    nexus.fees.settlement_mode = iroha_config::parameters::actual::NexusFeeSettlementMode::Direct;
    nexus.fees.fee_asset_id = fee_asset.canonical_address();
    nexus.fees.base_fee = Quantity::from(1_u32);
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    // Preserve this fixture's actual fee policy from initial State/Kura construction.
    let state =
        State::new_with_pre_genesis_nexus_for_testing(world, nexus, LiveQueryStore::start_test());
    let fee_payment = iroha_data_model::transaction::FeePaymentIntent::authority(
        vec![iroha_data_model::transaction::FeeChargeLimit::new(
            iroha_data_model::transaction::FeeChargeKind::Nexus,
            fee_asset,
            Quantity::from(1_u32),
        )],
        None,
    );
    let signed = TransactionBuilder::new_with_time_source(
        state.network_id,
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
        state.network_id_ref(),
        Duration::from_millis(10),
        tx_limits,
        &iroha_config::parameters::actual::Crypto::default(),
        &time_source,
    )
    .expect("accept fee drift transaction");
    let hash = tx.hash_as_entrypoint();
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
fn expired_event_uses_the_authoritative_full_plan() {
    let expected = RoutingDecision::new(LaneId::new(5), DataSpaceId::new(13));
    let state = State::new_with_nexus_for_testing(
        world_with_test_domains(),
        test_nexus_for_routes(&[(expected.lane_id, expected.dataspace_id)]),
        LiveQueryStore::start_test(),
    );
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
    let signed_hash = tx.as_ref().hash();
    let hash = tx.as_ref().hash_as_entrypoint();
    queue.push(tx, state.view()).expect("push tx");
    while event_receiver.try_recv().is_ok() {}
    time_handle.advance(Duration::from_millis(11));
    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
    assert!(guards.is_empty());
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.routing_plan_hint(&hash), None);
    let mut saw_expired = false;
    while let Ok(event) = event_receiver.try_recv() {
        let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = event else {
            continue;
        };
        if event.hash != signed_hash {
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
    let mut state = State::new_with_nexus_for_testing(
        world_with_test_domains(),
        test_nexus_for_routes(&[(expected.lane_id, expected.dataspace_id)]),
        LiveQueryStore::start_test(),
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router = Arc::new(MutableRouter::new(expected));
    let mut queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router.clone(),
        &[(expected.lane_id, expected.dataspace_id)],
    );
    install_manifest_lane_authority_for_queue_test(&mut state, &queue, 0x71);
    let journal_dir = tempfile::tempdir().expect("journal directory");
    queue
        .install_plan_journal(
            journal_dir.path().join("routing-corruption.norito"),
            1024 * 1024,
            true,
        )
        .expect("install exact routing-ownership journal");
    let state = Arc::new(state);
    let (event_sender, mut event_receiver) = tokio::sync::broadcast::channel(8);
    queue.events_sender = event_sender;
    let queue = Arc::new(queue);
    let tx = accepted_tx_by_someone(&time_source);
    let signed_hash = tx.as_ref().hash();
    let hash = tx.as_ref().hash_as_entrypoint();
    queue.push(tx, state.view()).expect("push tx");
    while event_receiver.try_recv().is_ok() {}
    let stale_plan = RoutingPlan::single(stale);
    queue.routing_plans.insert(hash, stale_plan.clone());
    router.set_error(RoutingResolveError::UnknownLane {
        lane_id: LaneId::new(99),
    });
    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
    assert!(guards.is_empty());
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(queue.routing_plan_hint(&hash), Some(stale_plan));
    assert_eq!(
        queue
            .routing_plan_hint(&hash)
            .map(|plan| plan.coordinator_route()),
        Some(stale)
    );
    assert!(queue.accepted_work_validation_faulted());
    let mut saw_rejected = false;
    while let Ok(event) = event_receiver.try_recv() {
        let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = event else {
            continue;
        };
        if event.hash != signed_hash {
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
