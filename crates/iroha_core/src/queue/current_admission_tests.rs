// Current executable admission must precede any queue, fee, or journal custody.
fn current_admission_queue_fixture() -> (
    State,
    Queue,
    TimeSource,
    tempfile::TempDir,
    std::path::PathBuf,
) {
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    install_single_validator_topology_for_queue_test(&mut state, 0xB7);
    let (_clock, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: DataSpaceId::UNIVERSAL,
        }),
        &[],
    );
    let directory = tempfile::tempdir().expect("current admission journal directory");
    let path = directory.path().join("current-admission.norito");
    queue
        .install_plan_journal(&path, 1024 * 1024, true)
        .expect("install journal");
    (state, queue, time_source, directory, path)
}

#[test]
fn current_admission_rejects_unsupported_intent_across_direct_queue_boundaries() {
    let (mut state, queue, time_source, _directory, path) = current_admission_queue_fixture();
    let unsupported = accepted_queue_plan_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &unsupported);
    let plan = queue
        .route_plan_with_state(&unsupported, &state)
        .expect("single route");
    let before = std::fs::read(&path).expect("journal before rejected attempts");
    for boundary in 0..5 {
        let result = match boundary {
            0 => queue.push(unsupported.clone(), state.view()).map(|_| ()),
            1 => queue
                .push_with_lane_with_state(unsupported.clone(), &state)
                .map(|_| ()),
            2 => queue
                .push_with_gossip_payload_with_state_and_routing_plan(
                    unsupported.clone(),
                    &state,
                    plan.clone(),
                    None,
                )
                .map(|_| ()),
            3 => queue
                .push_with_lane_with_state_and_routing_plan_strict_durable(
                    unsupported.clone(),
                    &state,
                    plan.clone(),
                )
                .map(|_| ()),
            _ => queue
                .push_batch_with_lane_with_state_and_routing_plans(
                    vec![(unsupported.clone(), plan.clone())],
                    &state,
                )
                .map(|_| ()),
        };
        let failure = result.expect_err("unsupported signed intent must be rejected");
        assert!(matches!(
            failure.err,
            Error::UnsupportedTransactionAdmission { .. }
        ));
        assert_eq!(failure.tx.entrypoint(), unsupported.entrypoint());
        assert_eq!(queue.active_len(), 0);
        assert!(queue.txs.is_empty());
        assert!(queue.durable_plan_claims.is_empty());
        assert!(
            queue
                .fee_admission_reservations
                .lock()
                .live_by_entrypoint
                .is_empty()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    let ordinary = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &ordinary);
    let ordinary_plan = queue.route_plan_with_state(&ordinary, &state).unwrap();
    queue
        .push_with_lane_with_state_and_routing_plan_strict_durable(ordinary, &state, ordinary_plan)
        .expect("supported Ordinary work retains real durable admission");
    assert_eq!(queue.active_len(), 1);
    assert_ne!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn current_admission_rejects_actual_multiroute_before_queue_custody() {
    let (_clock, time_source) = TimeSource::new_mock(Duration::default());
    let fixture = native_amx_participant_drift_fixture(&time_source);
    let queue = Queue::test(config_factory(), &time_source);
    let actual = queue
        .route_plan_with_state(&fixture.tx, &fixture.state)
        .expect("real multi-route resolves");
    assert_eq!(actual, fixture.current_plan);
    assert!(!matches!(actual, RoutingPlan::Single(_)));
    assert_eq!(
        fixture.tx.entrypoint().admission_intent(),
        TransactionAdmissionIntent::Ordinary
    );
    for boundary in 0..3 {
        let failure = match boundary {
            0 => queue
                .push(fixture.tx.clone(), fixture.state.view())
                .map(|_| ()),
            1 => queue
                .push_with_lane_with_state_and_routing_plan(
                    fixture.tx.clone(),
                    &fixture.state,
                    actual.clone(),
                )
                .map(|_| ()),
            _ => queue
                .push_batch_with_lane_with_state_and_routing_plans(
                    vec![(fixture.tx.clone(), actual.clone())],
                    &fixture.state,
                )
                .map(|_| ()),
        }
        .expect_err("resolved multi-route work has no current execution owner");
        assert!(matches!(
            failure.err,
            Error::UnsupportedTransactionAdmission { .. }
        ));
        assert_eq!(queue.active_len(), 0);
        assert!(queue.durable_plan_claims.is_empty());
        assert!(
            queue
                .fee_admission_reservations
                .lock()
                .live_by_entrypoint
                .is_empty()
        );
    }
}

#[test]
fn current_admission_replay_rejects_unsupported_record_without_publishing_or_rewriting() {
    let (mut state, queue, time_source, _directory, path) = current_admission_queue_fixture();
    let unsupported = accepted_queue_plan_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &unsupported);
    let plan = queue.route_plan_with_state(&unsupported, &state).unwrap();
    let context = queue
        .plan_admission_context_with_state(&state, &plan)
        .unwrap();
    // Seed an on-disk incompatible record directly; the live API must never create it.
    queue
        .record_plan_journal_put_durable(
            &unsupported,
            &plan,
            &context,
            queue.queue_plan_admission_timestamp_ms(),
            None,
            None,
            true,
        )
        .expect("write incompatible disk fixture");
    let before = std::fs::read(&path).unwrap();
    drop(queue);
    let replay = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: DataSpaceId::UNIVERSAL,
        }),
        &[],
    );
    assert_eq!(
        replay
            .install_plan_journal(&path, 1024 * 1024, true)
            .unwrap(),
        1
    );
    let error = replay
        .replay_plan_journal(&state)
        .expect_err("unsupported custody must fail startup explicitly");
    assert!(
        error
            .to_string()
            .contains("unsupported_transaction_admission")
    );
    assert_eq!(replay.active_len(), 0);
    assert!(replay.txs.is_empty());
    assert!(replay.durable_plan_claims.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn current_payload_selects_full_block_gas_call_with_idle_catalog_route() {
    let busy_route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    let mut nexus = test_nexus_for_routes(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (busy_route.lane_id, busy_route.dataspace_id),
    ]);
    nexus.routing_policy.default_lane = busy_route.lane_id;
    nexus.routing_policy.default_dataspace = busy_route.dataspace_id;
    let NexusRoutingFixture {
        mut state,
        authority_id,
        authority_keypair,
    } = nexus_routing_fixture_with_nexus(nexus);
    install_single_validator_topology_for_queue_test(&mut state, 0xB8);
    assert_eq!(state.consensus_lane_routes_at_height(1).len(), 2);
    let block_gas = crate::state::gas_limit_from_parameters(state.world_view().parameters());
    let (_clock, time_source) = TimeSource::new_mock(Duration::ZERO);
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let directory = tempfile::tempdir().expect("current payload journal directory");
    let path = directory.path().join("ordinary-full-block-gas.norito");
    queue
        .install_plan_journal(&path, 1024 * 1024, true)
        .expect("install Ordinary admission journal");
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        state.network_id_ref(),
        &authority_id,
        1,
        busy_route.dataspace_id,
    )
    .expect("derive exact-network contract address");
    let signed = TransactionBuilder::new_with_time_source(
        *state.network_id_ref(),
        authority_id,
        &time_source,
        iroha_data_model::transaction::FeePaymentIntent::authority(
            Vec::new(),
            NonZeroU64::new(block_gas),
        ),
    )
    .with_admission_intent(TransactionAdmissionIntent::Ordinary)
    .with_executable(Executable::ContractCall(
        iroha_data_model::transaction::executable::ContractInvocation {
            contract_address,
            expected_code_hash: Hash::new(b"current-full-block-gas-call"),
            entrypoint: "configure".to_owned(),
            arguments: None,
        },
    ))
    .sign(authority_keypair.private_key());
    let accepted = AcceptedTransaction::accept_with_time_source(
        signed,
        state.network_id_ref(),
        Duration::from_millis(10),
        TransactionParameters::default(),
        &iroha_config::parameters::actual::Crypto::default(),
        &time_source,
    )
    .expect("validate the caller-signed Ordinary contract call");
    assert_eq!(Queue::compute_proposal_gas_cost(&accepted), Ok(block_gas));
    let entrypoint = accepted.entrypoint().clone();
    let hash = accepted.hash_as_entrypoint();
    let max_bytes = accepted.encoded_len();
    let plan = queue.route_plan_with_state(&accepted, &state).unwrap();
    assert_eq!(plan, RoutingPlan::single(busy_route));
    let context = queue
        .plan_admission_context_with_state(&state, &plan)
        .unwrap();
    assert_eq!(context.route_incarnations.len(), 1);
    assert_eq!(context.route_incarnations[0].validator_count, 4);
    queue
        .push_with_lane_with_state_and_routing_plan_strict_durable(accepted, &state, plan.clone())
        .expect("durably admit supported full-block-gas work");
    let journal_before = std::fs::read(&path).unwrap();
    assert_eq!(queue.fifo_snapshot_for_test(), vec![hash]);

    // Current proposal building peeks; neither an idle route nor rebuilding a
    // proposal may divide this input's gas budget or take away its durable owner.
    for _ in 0..2 {
        let selected = crate::sumeragi::payload::select(&state, &queue, max_bytes);
        assert_eq!(
            selected.len(),
            1,
            "the idle route must not split the gas budget"
        );
        assert_eq!(selected[0].0.entrypoint(), &entrypoint);
        assert_eq!(selected[0].1, plan);
        assert_eq!(
            Queue::compute_proposal_gas_cost(&selected[0].0),
            Ok(block_gas)
        );
        assert_eq!(queue.fifo_snapshot_for_test(), vec![hash]);
        assert_eq!(queue.queued_len(), 1);
        assert!(queue.live_lane_reservations().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), journal_before);
    }
}

#[test]
fn current_ordinary_fifo_survives_committed_height_and_replay() {
    let handle = crate::sumeragi::threads::sumeragi_thread_builder("ordinary-height-custody")
        .spawn(current_ordinary_fifo_survives_committed_height_and_replay_on_consensus_stack)
        .expect("spawn actual genesis and queue recovery on the production consensus stack");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[inline(never)]
fn current_ordinary_fifo_survives_committed_height_and_replay_on_consensus_stack() {
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

    let (authority, key) = gen_account_in("ordinary-height-custody");
    let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 10_000))
        .expect("real four-validator genesis");
    assert_eq!(chain.height(), 1);
    let state = Arc::clone(chain.state());
    let (_clock, time) = TimeSource::new_mock(Duration::from_millis(20_001));
    let directory = tempfile::tempdir().expect("ordinary height custody journal");
    let path = directory.path().join("ordinary-height-custody.norito");
    let queue = Arc::new(Queue::test(config_factory(), &time));
    queue
        .install_plan_journal(&path, 1024 * 1024, true)
        .unwrap();
    let mut hashes = Vec::new();
    let mut signed_inputs = Vec::new();
    for index in 0..2_u64 {
        let signed = chain.sign(
            &key,
            [InstructionBox::from(Log::new(
                Level::INFO,
                format!("pending ordinary input {index}"),
            ))],
            20_000 + index,
        );
        let accepted = AcceptedTransaction::accept_with_time_source(
            signed,
            state.network_id_ref(),
            Duration::from_millis(10),
            TransactionParameters::default(),
            &iroha_config::parameters::actual::Crypto::default(),
            &time,
        )
        .expect("exact signed Ordinary input");
        assert_eq!(
            accepted.entrypoint().admission_intent(),
            TransactionAdmissionIntent::Ordinary
        );
        let plan = queue.route_plan_with_state(&accepted, &state).unwrap();
        assert!(matches!(plan, RoutingPlan::Single(_)));
        let context = queue
            .plan_admission_context_with_state(&state, &plan)
            .unwrap();
        assert_eq!((context.authority_height, context.proposal_height), (1, 2));
        assert_eq!(context.route_incarnations[0].validator_count, 4);
        hashes.push(accepted.hash_as_entrypoint());
        signed_inputs.push(norito::to_bytes(accepted.entrypoint()).unwrap());
        queue
            .push_with_lane_with_state_and_routing_plan_strict_durable(accepted, &state, plan)
            .expect("materialize the exact Ordinary owner before height advancement");
    }
    let journal = std::fs::read(&path).unwrap();
    assert_eq!(queue.fifo_snapshot_for_test(), hashes);

    // Advance the actual committed frontier with unrelated nonempty work. The
    // pending inputs have no height-adapter or f+1 certificate prerequisite.
    let advance = chain.sign(
        &key,
        [InstructionBox::from(Log::new(
            Level::INFO,
            "advance committed frontier".to_owned(),
        ))],
        20_002,
    );
    assert_eq!(chain.commit(vec![advance]), [true]);
    assert_eq!(chain.height(), 2);
    assert_eq!(
        state
            .view()
            .latest_block()
            .unwrap()
            .network_entrypoint_count(),
        1
    );
    let assert_retained = |queue: &Arc<Queue>| {
        assert_eq!(queue.fifo_snapshot_for_test(), hashes);
        assert_eq!((queue.active_len(), queue.queued_len()), (2, 2));
        for _ in 0..2 {
            let selected = crate::sumeragi::payload::select(&state, queue, 1024 * 1024);
            assert_eq!(selected.len(), 2);
            assert_eq!(
                selected
                    .iter()
                    .map(|(tx, _)| tx.hash_as_entrypoint())
                    .collect::<Vec<_>>(),
                hashes
            );
            assert_eq!(
                selected
                    .iter()
                    .map(|(tx, _)| norito::to_bytes(tx.entrypoint()).unwrap())
                    .collect::<Vec<_>>(),
                signed_inputs
            );
            for (_, plan) in &selected {
                let current = queue
                    .plan_admission_context_with_state(&state, plan)
                    .unwrap();
                assert_eq!((current.authority_height, current.proposal_height), (2, 3));
            }
            assert_eq!(queue.fifo_snapshot_for_test(), hashes);
            assert_eq!(std::fs::read(&path).unwrap(), journal);
            assert!(queue.live_lane_reservations().is_empty());
        }
    };
    assert_retained(&queue);
    drop(queue);
    let replay = Arc::new(Queue::test(config_factory(), &time));
    assert_eq!(
        replay
            .install_plan_journal(&path, 1024 * 1024, true)
            .unwrap(),
        2
    );
    let summary = replay
        .replay_plan_journal(&state)
        .expect("reopen exact Ordinary custody at the committed successor");
    assert_eq!(summary.replayed, 2);
    assert_retained(&replay);
}
