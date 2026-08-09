// Native AMX reservation coverage remains in the parent queue::tests module.

#[test]
fn native_amx_participant_lane_cannot_reserve_or_execute_full_transaction() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let coordinator = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let participant = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    let routes = [
        (coordinator.lane_id, coordinator.dataspace_id),
        (participant.lane_id, participant.dataspace_id),
    ];
    let (lane_catalog, dataspace_catalog) = Queue::test_catalogs_for_routes(&routes);
    let kura_dir = tempdir().expect("authenticated queue Kura root");
    let lane_geometry = LaneGeometry::from_catalog(&lane_catalog);
    let kura_config = KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(kura_dir.path().join("kura")),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity:
            iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: iroha_config::kura::FsyncMode::Batched,
        fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention:
            iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention:
            iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
    };
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config, &lane_geometry, &lane_catalog)
            .expect("open authenticated two-lane reservation Kura");
    // Exercise the production startup sequence here. `State::new` is the unit-test
    // convenience constructor and eagerly installs a marker for its initial single-lane
    // catalog; that marker necessarily precedes (and therefore conflicts with) the
    // authenticated two-lane configured-primary anchor established below.
    let mut state = State::try_new(
        world_with_test_domains(),
        kura,
        LiveQueryStore::start_test(),
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
    .expect("open reservation-test State without replacing authenticated Kura markers");
    state
        .prepare_configured_primary_geometry_anchor(&lane_catalog)
        .expect("anchor authenticated reservation-test primary");
    state
        .restore_kura_lane_segments_before_startup_replay()
        .expect("restore reservation-test startup cursor");
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    nexus.lane_catalog = (*lane_catalog).clone();
    nexus.configured_lane_catalog = nexus.lane_catalog.clone();
    nexus.lane_config = lane_geometry;
    nexus.dataspace_catalog = (*dataspace_catalog).clone();
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    state
        .set_nexus_from_config(nexus)
        .expect("install two-lane reservation test Nexus");
    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        state.nexus_snapshot().routing_policy,
        (*dataspace_catalog).clone(),
        (*lane_catalog).clone(),
    ));
    let queue = Arc::new(Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router,
        &routes,
    ));
    install_manifest_lane_authority_for_queue_test(&mut state, queue.as_ref(), 0xC1);
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    queue
        .install_plan_journal(
            dir.path().join("native-amx-queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install Native AMX queue-plan journal");
    let (authority, authority_keypair) = gen_account_in("wonderland");
    let transaction = accepted_tx_with(
        authority.clone(),
        &authority_keypair,
        &time_source,
        vec![
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("nativeamxcoordinator", "universal")
                    .expect("coordinator domain id"),
            ))),
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("nativeamxparticipant", "test-dataspace-7")
                    .expect("participant domain id"),
            ))),
        ],
        Metadata::default(),
    );
    register_test_authority(&mut state, &authority);
    let plan = queue
        .route_plan_for_gossip_with_state(&transaction, &state)
        .expect("derive exact current Native AMX reservation plan");
    let RoutingPlan::NativeAmx(native_plan) = &plan else {
        panic!("mixed-dataspace reservation transaction must use Native AMX");
    };
    assert_eq!(native_plan.coordinator.route, coordinator);
    assert!(
        native_plan
            .participants
            .iter()
            .any(|leg| leg.route == participant),
        "Native AMX reservation plan must retain the participant lane"
    );
    let admission_context = queue
        .plan_admission_context_with_state(&state, &plan)
        .expect("capture Native AMX admission context");
    let admission_binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
        state.chain_id_ref(),
        transaction.entrypoint(),
        &plan,
        admission_context,
        queue.queue_plan_admission_timestamp_ms(),
    )
    .expect("build Native AMX global admission binding");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            transaction,
            &state,
            plan.clone(),
            &admission_binding,
        )
        .expect("durably enqueue globally bound Native AMX transaction");

    let participant_scope = LaneQueueReservationScopeV1 {
        lane_id: participant.lane_id,
        dataspace_id: participant.dataspace_id,
        lane_incarnation: state
            .lane_incarnation_at_height(participant.lane_id, 1)
            .expect("participant lane incarnation"),
        proposal_height: 1,
        lane_block_height: 1,
        lane_block_view: 0,
        reservation_owner_hash: Hash::new(b"participant-owner"),
        proposal_identity_hash: Hash::new(b"participant-proposal"),
    };
    let coordinator_scope = LaneQueueReservationScopeV1 {
        lane_id: coordinator.lane_id,
        dataspace_id: coordinator.dataspace_id,
        lane_incarnation: state
            .lane_incarnation_at_height(coordinator.lane_id, 1)
            .expect("coordinator lane incarnation"),
        ..participant_scope
    };
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&admission_binding)
            .expect("read absent Native AMX admission registry"),
        QueuePlanAdmissionRegistryMatch::Absent
    );
    assert!(
        queue
            .reserve_transactions_for_lane(&state, coordinator_scope, nonzero!(1_usize))
            .expect("uncertified Native AMX selection must safely retain FIFO ownership")
            .is_empty(),
        "a durable local binding is not autonomous ownership authority before the global \
             carrier commits its exact registry marker"
    );
    assert_eq!(queue.queued_len(), 1);

    install_queue_plan_registry_value_for_test(&state, &admission_binding);
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&admission_binding)
            .expect("read exact Native AMX admission registry"),
        QueuePlanAdmissionRegistryMatch::Exact
    );
    assert!(
        queue
            .reserve_transactions_for_lane(&state, participant_scope, nonzero!(1_usize))
            .expect("participant selection must safely return no full transaction")
            .is_empty()
    );
    assert_eq!(queue.queued_len(), 1);

    assert!(
        queue
            .reserve_transactions_for_lane_bounded(
                &state,
                AutonomousLaneReservationSelectionAuthorization::single_validator_for_test(
                    coordinator_scope,
                ),
                LaneQueueReservationSelectionLimits {
                    max_transactions: nonzero!(1_usize),
                    max_scan: nonzero!(1_usize),
                    max_encoded_bytes: NonZeroU64::new(u64::MAX).expect("non-zero byte bound"),
                    max_gas: NonZeroU64::new(u64::MAX).expect("non-zero gas bound"),
                },
                &BTreeSet::new(),
                LaneQueueReservationRoutingMode::SingleRouteOnly,
            )
            .expect("single-route mode excludes Native AMX")
            .is_empty()
    );
    assert_eq!(queue.queued_len(), 1);
    let reserved = queue
        .reserve_transactions_for_lane(&state, coordinator_scope, nonzero!(1_usize))
        .expect("coordinator reserves Native AMX transaction");
    assert_eq!(reserved.len(), 1);
    assert_eq!(reserved[0].routing_plan(), &plan);
    assert_eq!(
        reserved[0].key().coordinator_leg.role,
        RouteLegRole::Coordinator
    );
    assert_eq!(reserved[0].key().lane_id, coordinator.lane_id);
}
