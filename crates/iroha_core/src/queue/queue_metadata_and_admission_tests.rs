#[test]
fn guard_return_capacity_invariant_is_batch_atomic() {
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut cfg = config_factory();
    cfg.capacity = nonzero!(2_usize);
    cfg.capacity_per_user = nonzero!(2_usize);
    let queue = Arc::new(Queue::test(cfg, &time_source));
    for _ in 0..2 {
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("fill queue");
    }
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2_usize));
    let guarded_hashes = guards
        .iter()
        .map(|guard| guard.tx.hash())
        .collect::<BTreeSet<_>>();

    // Corrupt the private index with a tracked foreign entry to exercise the fail-closed
    // capacity preflight. No returned guard may be partially released or appended.
    let foreign = accepted_tx_by_someone(&time_source);
    let foreign_hash = foreign.as_ref().hash();
    queue.txs.insert(
        foreign_hash,
        Arc::new(CheckedTransaction::new_unchecked(foreign)),
    );
    queue.tx_enqueued_at_ms.insert(foreign_hash, 0);
    assert!(queue.push_queued_hash(foreign_hash, 0));

    let err = queue
        .return_transaction_guards(&mut guards, &state)
        .expect_err("corrupt live hash index must fail closed");
    assert!(matches!(
        err,
        TransactionGuardReturnError::HashIndexCapacity {
            queued: 1,
            returning: 2,
            capacity: 2
        }
    ));
    assert_eq!(guards.len(), 2);
    assert!(guards.iter().all(|guard| !guard.released));
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 2);
    assert_eq!(
        guarded_hashes
            .iter()
            .filter(|hash| queue.queued_tx_enqueued_at_ms.contains_key(hash))
            .count(),
        0,
        "capacity failure must not append a partial returned batch"
    );

    assert_eq!(queue.pop_queued_hash(), Some(foreign_hash));
    queue.txs.remove(&foreign_hash);
    queue.tx_enqueued_at_ms.remove(&foreign_hash);
    let report = queue
        .return_transaction_guards(&mut guards, &state)
        .expect("return after repairing index");
    assert_eq!(report.returned, 2);
    assert_eq!(queue.queued_len(), 2);
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 0);
}

#[test]
fn guard_return_missing_transaction_is_explicit_and_does_not_release_batch() {
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("push tx");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(1_usize));
    let hash = guards[0].tx.hash();
    let (_, tracked) = queue.txs.remove(&hash).expect("remove tracked entry");

    let err = queue
        .return_transaction_guards(&mut guards, &state)
        .expect_err("unowned missing transaction must fail explicitly");
    assert_eq!(
        err,
        TransactionGuardReturnError::MissingTrackedTransactions { hashes: vec![hash] }
    );
    assert_eq!(guards.len(), 1);
    assert!(!guards[0].released);
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 1);
    assert_eq!(queue.queued_len(), 0);

    queue.txs.insert(hash, tracked);
    let report = queue
        .return_transaction_guards(&mut guards, &state)
        .expect("return after restoring invariant");
    assert_eq!(report.returned, 1);
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 0);
}

#[test]
fn queue_metadata_cleared_on_commit_and_clear_all() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    queue.push(tx, state.view()).expect("push tx");
    let _ = queue.gossip_batch(1, &state.view());

    let removed = queue.remove_committed_hashes(std::iter::once(hash), None);
    assert_eq!(removed, 1);
    assert!(queue.tx_encoded_len.is_empty());
    assert!(queue.tx_gas_cost.is_empty());

    let tx = accepted_tx_by_someone(&time_source);
    queue.push(tx, state.view()).expect("push tx");
    assert!(!queue.tx_encoded_len.is_empty());
    let _ = queue.gossip_batch(1, &state.view());
    queue.clear_all();
    assert!(queue.tx_encoded_len.is_empty());
    assert!(queue.tx_gas_cost.is_empty());
}

#[test]
fn queue_reuses_gossip_payload_without_side_cache() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let entrypoint = tx.entrypoint().clone();
    let entrypoint_hash = tx.hash_as_entrypoint();
    let payload = tx.entrypoint_bytes();
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
    let tx = AcceptedTransaction::accept_gossip_entrypoint_with_payload(
        entrypoint,
        Arc::clone(&payload),
        entrypoint_hash,
        &ChainId::from("00000000-0000-0000-0000-000000000000"),
        Duration::from_millis(10),
        tx_limits,
        &crypto_cfg,
    )
    .expect("accept gossip entrypoint with cached payload");
    let hash = tx.as_ref().hash();
    assert!(
        Arc::ptr_eq(&tx.entrypoint_bytes(), &payload),
        "accepted gossip transaction should reuse inbound entrypoint bytes"
    );

    queue
        .push_with_gossip_payload(tx, state.view(), Some(Arc::clone(&payload)))
        .expect("push tx with payload");

    let encoded_len = queue
        .tx_encoded_len
        .get(&hash)
        .map(|entry| *entry.value())
        .expect("encoded len stored");
    assert_eq!(encoded_len, payload.len());

    let batch = queue.gossip_batch_with_state(1, &state);
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].payload.as_slice(), payload.as_slice());
    assert!(
        Arc::ptr_eq(&batch[0].payload, &payload),
        "gossip should reuse inbound entrypoint bytes"
    );
}

#[test]
fn queue_reuses_gossip_payload_without_side_cache_in_shared_view() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let payload = tx.entrypoint_bytes();
    let state_view = state.view();

    queue
        .push_with_gossip_payload_in_view(tx, &state_view, Some(Arc::clone(&payload)))
        .expect("push tx with payload through shared view");

    let batch = queue.gossip_batch(1, &state_view);
    assert_eq!(batch.len(), 1);
    assert!(Arc::ptr_eq(&batch[0].payload, &payload));
}

#[test]
fn queue_reuses_gossip_payload_without_side_cache_with_state() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let payload = tx.entrypoint_bytes();

    queue
        .push_with_gossip_payload_with_state(tx, &state, Some(Arc::clone(&payload)))
        .expect("push tx with payload through state");

    let batch = queue.gossip_batch_with_state(1, &state);
    assert_eq!(batch.len(), 1);
    assert!(Arc::ptr_eq(&batch[0].payload, &payload));
}

#[test]
fn queue_generated_gossip_payload_uses_framed_entrypoint_wire() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let cached_payload = tx.entrypoint_bytes();
    let expected_payload = ncore::to_bytes(tx.entrypoint()).expect("encode transaction entrypoint");

    queue.push(tx, state.view()).expect("push tx");

    let batch = queue.gossip_batch(1, &state.view());
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].payload.as_slice(), expected_payload.as_slice());
    assert!(
        Arc::ptr_eq(&batch[0].payload, &cached_payload),
        "queue gossip should reuse accepted transaction entrypoint bytes"
    );
}

#[test]
fn sealed_commitment_uses_local_queue_residence_ttl() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::from_secs(3600));
    let queue = Queue::test(
        Config {
            transaction_time_to_live: Duration::from_secs(1),
            expired_cull_interval: Duration::from_secs(1),
            ..config_factory()
        },
        &time_source,
    );
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let chain_id = ChainId::from("sealed-queue-expiry");
    let (authority, keypair) = gen_account_in("wonderland");
    let inner_tx = TransactionBuilder::new_with_time_source(
        chain_id.clone(),
        authority.clone(),
        &time_source,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "sealed queue expiry inner".to_owned(),
    )])
    .sign(keypair.private_key());
    let salt = [0xD4; 32];
    let reveal_deadline_height = 10;
    let commitment_hash =
        compute_sealed_transaction_commitment(&chain_id, &inner_tx, salt, reveal_deadline_height);
    let payload = SealedTransactionCommitmentPayload::new(
        chain_id,
        authority,
        commitment_hash,
        2,
        reveal_deadline_height,
        None,
    );
    let commitment = SignedSealedTransactionCommitment::sign(payload, keypair.private_key());
    let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
        TransactionEntrypoint::SealedCommitment(commitment),
    ));

    assert!(
        !queue.is_expired(&accepted),
        "a commitment has no queue residence before admission"
    );
    queue
        .push(accepted, state.view())
        .expect("sealed commitment admission");
    time_handle.advance(Duration::from_secs(2));
    assert_eq!(
        queue.cull_expired_entries_if_due(),
        1,
        "a commitment that never reaches a block must not become a permanent queue entry"
    );
    assert_eq!(queue.active_len(), 0);
}

#[test]
fn push_in_view_accepts_multiple_transactions_with_shared_snapshot() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let state_view = state.view();
    queue
        .push_in_view(accepted_tx_by_someone(&time_source), &state_view)
        .expect("first push");
    queue
        .push_in_view(accepted_tx_by_someone(&time_source), &state_view)
        .expect("second push");

    assert_eq!(queue.queued_len(), 2);
}

#[test]
fn push_with_lane_with_state_accepts_multiple_transactions() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    queue
        .push_with_lane_with_state(accepted_tx_by_someone(&time_source), &state)
        .expect("first push");
    queue
        .push_with_lane_with_state(accepted_tx_by_someone(&time_source), &state)
        .expect("second push");

    assert_eq!(queue.queued_len(), 2);
}

#[test]
fn push_with_lane_with_state_rejects_committed_transaction() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let tx_hash = tx.as_ref().hash();
    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(tx_hash, nonzero!(1_usize));
        transactions
            .commit()
            .expect("transactions block should commit");
    }

    let err = queue
        .push_with_lane_with_state(tx, &state)
        .expect_err("committed transaction must be rejected");
    assert!(matches!(err.err, Error::InBlockchain));
    assert_eq!(err.tx.as_ref().as_ref().hash(), tx_hash);
}

#[test]
fn push_with_lane_with_state_rejects_unresolved_route() {
    struct UnresolvedRouter;

    impl LaneRouter for UnresolvedRouter {
        fn route(&self, _tx: &dyn TransactionRoutingView) -> RoutingDecision {
            RoutingDecision::new(LaneId::new(99), DataSpaceId::new(77))
        }
    }

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test_with_router(config_factory(), &time_source, Arc::new(UnresolvedRouter));
    let tx = accepted_tx_by_someone(&time_source);

    let err = queue
        .push_with_lane_with_state(tx, &state)
        .expect_err("unresolved route must be rejected");
    assert!(matches!(err.err, Error::UnresolvedRoute { .. }));
    if let Error::UnresolvedRoute { reason } = &err.err {
        assert!(
            reason.contains("lane"),
            "route rejection reason should include lane lookup failure"
        );
    }
}

#[test]
fn push_with_lane_with_state_rejects_confidential_policy_before_enqueue() {
    let (authority_id, authority_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let account = Account::new(authority_id.clone()).build(&authority_id);
    let asset_def_id = AssetDefinitionId::derive_from_components(
        domain_id,
        "zkqueuepolicy".parse().expect("asset name"),
    );
    let asset_definition = AssetDefinition::numeric(
        asset_def_id.clone(),
        "zkqueuepolicy".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .confidential_policy(
        iroha_data_model::asset::definition::AssetConfidentialPolicy::convertible(),
    )
    .build(&authority_id);
    let mut world = World::with([domain], [account], [asset_definition]);
    let mut zk_state = crate::state::ZkAssetState::default();
    zk_state.mode = iroha_data_model::isi::zk::ZkAssetMode::Hybrid;
    zk_state.allow_shield = false;
    zk_state.allow_unshield = true;
    world.zk_assets.insert(asset_def_id.clone(), zk_state);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_with(
        authority_id.clone(),
        &authority_keypair,
        &time_source,
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Shield::new(
                asset_def_id,
                authority_id,
                10_u128,
                [3; 32],
                iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
            ),
        )],
        Metadata::default(),
    );

    let err = queue
        .push_with_lane_with_state(tx, &state)
        .expect_err("disabled shield must be rejected before enqueue");

    assert!(matches!(
        err.err,
        Error::ConfidentialPolicyAdmissionRejected { .. }
    ));
    if let Error::ConfidentialPolicyAdmissionRejected { detail, reason } = &err.err {
        assert_eq!(detail, "shield not permitted by policy");
        assert!(matches!(
            reason,
            TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message))
                if message == "shield not permitted by policy"
        ));
    }
    assert_eq!(queue.queued_len(), 0);
}

#[test]
fn contains_pending_hash_ignores_committed_entries() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    queue.push(tx, state.view()).expect("push tx");
    assert!(queue.contains_pending_hash(hash, &state));

    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(hash, nonzero!(1_usize));
        transactions
            .commit()
            .expect("transactions block should commit");
    }

    assert!(!queue.contains_pending_hash(hash, &state));
}

#[test]
fn gossip_batch_with_state_removes_committed_entries() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    queue.push(tx, state.view()).expect("push tx");
    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(hash, nonzero!(1_usize));
        transactions
            .commit()
            .expect("transactions block should commit");
    }

    let batch = queue.gossip_batch_with_state(1, &state);
    assert!(
        batch.is_empty(),
        "committed transaction must not be selected for gossip"
    );
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    assert!(!queue.current_backpressure().is_saturated());
}

#[tokio::test]
async fn push_rejects_without_governance_manifest() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let metrics = Arc::new(Metrics::default());
    #[cfg(feature = "telemetry")]
    let state = Arc::new(State::with_telemetry(
        world_with_test_domains(),
        kura.clone(),
        query_handle.clone(),
        StateTelemetry::new(metrics.clone(), true),
    ));
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let mut statuses = BTreeMap::new();
    statuses.insert(
        LaneId::SINGLE,
        LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "default".to_string(),
            dataspace: DataSpaceId::UNIVERSAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: None,
            governance_rules: None,
            privacy_commitments: Vec::new(),
        },
    );
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
    queue.install_lane_manifests(&manifests);

    let result = queue.push(accepted_tx_by_someone(&time_source), state.view());
    assert!(matches!(
        result,
        Err(Failure {
            err: Error::Governance(_),
            ..
        })
    ));
    #[cfg(feature = "telemetry")]
    assert_eq!(
        metrics
            .governance_manifest_admission_total
            .with_label_values(&["missing_manifest"])
            .get(),
        1
    );
}

#[tokio::test]
async fn uaid_without_dataspace_binding_is_rejected() {
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::missing-binding"));
    let dataspace = DataSpaceId::new(7);
    let (world, account_id, key_pair) = world_with_uaid_account(uaid, dataspace, false);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let metrics = Arc::new(Metrics::default());
    #[cfg(feature = "telemetry")]
    let state = {
        let mut state = State::with_telemetry(
            world,
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        );
        install_test_nexus_routes(&mut state, &[(LaneId::SINGLE, dataspace)]);
        Arc::new(state)
    };
    #[cfg(not(feature = "telemetry"))]
    let state = {
        let mut state = State::new(world, kura, query_handle);
        install_test_nexus_routes(&mut state, &[(LaneId::SINGLE, dataspace)]);
        Arc::new(state)
    };
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: LaneId::SINGLE,
        dataspace,
    });
    let queue = Arc::new(Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router.clone(),
        &[(LaneId::SINGLE, dataspace)],
    ));

    let mut statuses = BTreeMap::new();
    statuses.insert(
        LaneId::SINGLE,
        LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "uaid-enforcement".to_string(),
            dataspace,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: None,
            manifest_path: None,
            governance_rules: None,
            privacy_commitments: Vec::new(),
        },
    );
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
    queue.install_lane_manifests(&manifests);

    let result = queue.push(
        accepted_tx_by(account_id.clone(), &key_pair, &time_source),
        state.view(),
    );
    match result {
        Err(Failure {
            err: Error::LaneComplianceDenied { reason, .. },
            ..
        }) => assert!(
            reason.contains("not bound to dataspace"),
            "expected missing dataspace binding rejection, got {reason}"
        ),
        other => panic!("expected missing dataspace binding rejection, got {other:?}"),
    }
}

#[tokio::test]
async fn space_directory_manifest_publish_bypasses_uaid_binding_admission() {
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::manifest-publish"));
    let manifest_dataspace = DataSpaceId::new(10);
    let (world, account_id, key_pair) = world_with_uaid_account(uaid, manifest_dataspace, false);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world, kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: DataSpaceId::UNIVERSAL,
        }),
    );
    let manifest = AssetPermissionManifest {
        version: ManifestVersion::default(),
        uaid,
        dataspace: manifest_dataspace,
        issued_ms: 1,
        activation_epoch: 0,
        expiry_epoch: None,
        entries: Vec::new(),
    };
    let tx = accepted_tx_with(
        account_id,
        &key_pair,
        &time_source,
        vec![
            iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest { manifest }
                .into(),
        ],
        Metadata::default(),
    );

    queue
        .push(tx, state.view())
        .expect("manifest publication creates the UAID dataspace binding");
}

#[tokio::test]
async fn uaid_binding_allows_lane_identity_extraction() {
    let dataspace = DataSpaceId::new(11);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::bound"));
    let (world, account_id, key_pair) = world_with_uaid_account(uaid, dataspace, true);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let metrics = Arc::new(Metrics::default());
    #[cfg(feature = "telemetry")]
    let state = {
        let mut state = State::with_telemetry(
            world,
            kura.clone(),
            query_handle.clone(),
            StateTelemetry::new(metrics.clone(), true),
        );
        install_test_nexus_routes(&mut state, &[(LaneId::SINGLE, dataspace)]);
        Arc::new(state)
    };
    #[cfg(not(feature = "telemetry"))]
    let state = {
        let mut state = State::new(world, kura, query_handle);
        install_test_nexus_routes(&mut state, &[(LaneId::SINGLE, dataspace)]);
        Arc::new(state)
    };
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: LaneId::SINGLE,
        dataspace,
    });
    let queue = Arc::new(Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router.clone(),
        &[(LaneId::SINGLE, dataspace)],
    ));

    let mut statuses = BTreeMap::new();
    statuses.insert(
        LaneId::SINGLE,
        LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "uaid-binding".to_string(),
            dataspace,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: None,
            manifest_path: None,
            governance_rules: None,
            privacy_commitments: Vec::new(),
        },
    );
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(statuses));
    queue.install_lane_manifests(&manifests);

    queue
        .push(
            accepted_tx_by(account_id.clone(), &key_pair, &time_source),
            state.view(),
        )
        .expect("UAID with active dataspace binding should be admitted");
}

#[tokio::test]
async fn uaid_routing_rejects_foreign_dataspace_without_binding() {
    let bound = DataSpaceId::new(42);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::rebind"));
    let (world, account_id, key_pair) = world_with_uaid_account(uaid, bound, true);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world, kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let target = DataSpaceId::UNIVERSAL;
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: target,
        }),
    );

    let result = queue.push(
        accepted_tx_by(account_id.clone(), &key_pair, &time_source),
        state.view(),
    );
    match result {
        Err(Failure {
            err: Error::LaneComplianceDenied { reason, .. },
            ..
        }) => assert!(
            reason.contains("not bound to dataspace"),
            "expected missing dataspace binding rejection, got {reason}"
        ),
        other => panic!("expected missing dataspace binding rejection, got {other:?}"),
    }
}

#[tokio::test]
async fn uaid_binding_allows_matching_dataspace() {
    let dataspace = DataSpaceId::new(24);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::aligned"));
    let (world, account_id, key_pair) = world_with_uaid_account(uaid, dataspace, true);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world, kura, query_handle);
    install_test_nexus_routes(&mut state, &[(LaneId::SINGLE, dataspace)]);
    let state = Arc::new(state);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace,
        }),
        &[(LaneId::SINGLE, dataspace)],
    );

    queue
        .push(
            accepted_tx_by(account_id.clone(), &key_pair, &time_source),
            state.view(),
        )
        .expect("UAID bound to dataspace should be admitted");
}

#[tokio::test]
async fn uaid_with_inactive_target_dataspace_manifest_is_rejected() {
    let dataspace = DataSpaceId::new(24);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::inactive-manifest"));
    let (mut world, account_id, key_pair) = world_with_uaid_account(uaid, dataspace, true);
    let mut set = world
        .space_directory_manifests
        .view()
        .get(&uaid)
        .cloned()
        .expect("manifest set must exist");
    let record = set
        .get(&dataspace)
        .cloned()
        .expect("manifest record must exist");
    let mut inactive = record;
    inactive.lifecycle.mark_expired(2);
    set.upsert(inactive);
    world.space_directory_manifests.insert(uaid, set);

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world, kura, query_handle);
    install_test_nexus_routes(&mut state, &[(LaneId::SINGLE, dataspace)]);
    let state = Arc::new(state);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace,
        }),
        &[(LaneId::SINGLE, dataspace)],
    );

    let result = queue.push(
        accepted_tx_by(account_id.clone(), &key_pair, &time_source),
        state.view(),
    );

    match result {
        Err(Failure {
            err: Error::LaneComplianceDenied { .. },
            ..
        }) => {}
        other => panic!("expected inactive manifest rejection, got {other:?}"),
    }
}
