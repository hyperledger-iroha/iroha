#[tokio::test(flavor = "current_thread")]
#[allow(clippy::too_many_lines)]
async fn gossip_accepts_restricted_route_match() {
    let temp_dir = tempdir().expect("temp dir");
    let kura_cfg = KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(temp_dir.path().to_path_buf()),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
        block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
    };
    let (kura, _) = Kura::new(&kura_cfg, &LaneGeometry::default()).expect("init kura");
    let live_query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::new(), kura, live_query));
    let restricted_dataspace = DataSpaceId::new(7);
    let restricted_lane = LaneId::new(1);
    let lane_catalog = LaneCatalog::new(
        NonZeroU32::new(2).expect("nonzero lanes"),
        vec![
            iroha_data_model::nexus::LaneConfig {
                id: LaneId::SINGLE,
                alias: "public".to_string(),
                ..iroha_data_model::nexus::LaneConfig::default()
            },
            iroha_data_model::nexus::LaneConfig {
                id: restricted_lane,
                dataspace_id: restricted_dataspace,
                alias: "restricted".to_string(),
                visibility: LaneVisibility::Restricted,
                ..iroha_data_model::nexus::LaneConfig::default()
            },
        ],
    )
    .expect("lane catalog");
    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: restricted_dataspace,
            alias: "restricted".to_string(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    {
        let mut nexus = state.nexus.write();
        nexus.enabled = true;
        nexus.autoscale.enabled = false;
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.lane_catalog = lane_catalog.clone();
        nexus.lane_config = LaneGeometry::from_catalog(&lane_catalog);
        nexus.dataspace_catalog = dataspace_catalog;
        nexus.routing_policy.default_lane = restricted_lane;
        nexus.routing_policy.default_dataspace = restricted_dataspace;
    }
    assert!(state.is_lane_active_for_authority(restricted_lane));
    let queue = Arc::new(Queue::test(
        QueueConfig::default(),
        &TimeSource::new_system(),
    ));
    let now = Instant::now();
    let gossiper = TransactionGossiper {
        gossip_period: Duration::from_millis(50),
        gossip_size: NonZeroU32::new(1).expect("nonzero size"),
        gossip_resend_ticks: defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS,
        gossip_tick: 0,
        gossip_deferred: vec![
            Vec::new();
            defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS.get() as usize
        ],
        peer_recently_sent: BTreeMap::new(),
        peer_recent_ring: vec![Vec::new(); GOSSIP_PEER_RECENT_SUPPRESSION_TTL_TICKS],
        last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
        last_drop_at: None,
        network: IrohaNetwork::closed_for_tests(),
        queue: Arc::clone(&queue),
        state: Arc::clone(&state),
        tx_frame_cap: 1024,
        dataspace_cfg: DataspaceGossip::default(),
        public_seed: GossipTargetSeed::new(0xBEEF_0001, Duration::from_secs(1), now),
        restricted_seed: GossipTargetSeed::new(0xBEEF_0002, Duration::from_secs(1), now),
    };
    let (signed, _) = build_transaction("restricted-route");
    let route = GossipRoute {
        lane_id: restricted_lane,
        dataspace_id: restricted_dataspace,
    };
    assert_eq!(
        dataspace_plane(&state.nexus.read().lane_config, route.dataspace_id),
        Some(GossipPlane::Restricted)
    );
    assert_eq!(
        queue
            .route_plan_for_gossip_with_state(
                &AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone())),
                state.as_ref(),
            )
            .expect("restricted route should resolve locally"),
        plan_for_route(route)
    );
    gossiper.handle_transaction_gossip(Arc::new(TransactionGossip {
        txs: vec![signed.into()],
        routes: vec![route],
        plans: vec![plan_for_route(route)],
        plane: GossipPlane::Restricted,
    }));
    assert_eq!(queue.queued_len(), 1);
}
