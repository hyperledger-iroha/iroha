// Durable restart, metrics, live Kubo, and remote-head regressions.

#[test]
fn durable_restart_state_preserves_every_publish_phase() {
    let source = signed_source(2, 0x3b, 1_800_000_000);
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let store = test_checkpoint_store(provider);
    let mut intent = intent_from_source(&source);
    for block in &mut intent.blocks {
        block.ipfs_cid = None;
    }
    intent.head_ipfs_cid = None;
    let mut intent_revision =
        save_publish_intent(&store, None, &intent).expect("persist prepared intent");
    assert_eq!(
        load_publish_intent(&store)
            .expect("reload prepared intent")
            .0
            .expect("prepared intent exists")
            .blocks
            .iter()
            .filter(|block| block.ipfs_cid.is_some())
            .count(),
        0
    );

    intent.blocks[0].ipfs_cid = Some(TEST_CID_BLOCK.to_owned());
    intent_revision =
        save_publish_intent(&store, Some(intent_revision), &intent).expect("persist partial pins");
    assert_eq!(
        load_publish_intent(&store)
            .expect("reload partial pins")
            .0
            .expect("partial intent exists")
            .blocks[0]
            .ipfs_cid
            .as_deref(),
        Some(TEST_CID_BLOCK)
    );

    intent.blocks[1].ipfs_cid = Some(TEST_CID_PAYLOAD.to_owned());
    intent.head_ipfs_cid = Some(TEST_CID_HEAD.to_owned());
    intent_revision =
        save_publish_intent(&store, Some(intent_revision), &intent).expect("persist head pin");
    let loaded = load_publish_intent(&store)
        .expect("reload head pin")
        .0
        .expect("head intent exists");
    assert_eq!(loaded.head_ipfs_cid.as_deref(), Some(TEST_CID_HEAD));

    let target = PublicHead::Present {
        bytes: intent.target_head_bytes.clone(),
        token: "\"target\"".to_owned(),
    };
    assert_eq!(
        public_head_digest(&target),
        Some(intent.target_head_blake3),
        "restart recognizes a public head already at the durable target"
    );

    let checkpoint = checkpoint_from_source(&source);
    save_checkpoint(&store, None, &checkpoint).expect("persist checkpoint before cleanup");
    assert!(
        load_checkpoint(&store)
            .expect("reload checkpoint")
            .0
            .is_some()
    );
    assert!(
        load_publish_intent(&store)
            .expect("reload stale completed intent")
            .0
            .is_some()
    );
    delete_publish_intent(&store, Some(intent_revision)).expect("restart removes completed intent");
    assert!(
        load_publish_intent(&store)
            .expect("intent remains absent")
            .0
            .is_none()
    );
}

#[tokio::test]
async fn metrics_expose_exact_values_and_payload_kind_counts() {
    let mut block = JsonMap::new();
    block.insert("payload_kind".into(), JsonValue::from("deal_settlement"));
    let mut mirror = JsonMap::new();
    mirror.insert(
        "blocks".into(),
        JsonValue::Array(vec![
            JsonValue::Object(block.clone()),
            JsonValue::Object(block),
        ]),
    );
    let state = ApiState(Arc::new(RwLock::new(ApiSnapshot {
        mirror: Some(JsonValue::Object(mirror)),
        metrics: ServiceMetrics {
            publish_success_total: 2,
            publish_failure_total: 3,
            published_bytes_total: 5,
            last_publish_timestamp_seconds: 7,
            backlog: 11,
            head_age_seconds: 13,
            ipfs_pin_lag_seconds: 17,
            ipns_update_success_total: 19,
            ipns_update_failure_total: 23,
            last_ipns_update_timestamp_seconds: 29,
            validation_failure_total: 31,
            mirror_drift: 37,
        },
        ..ApiSnapshot::default()
    })));
    let response = metrics_handler(State(state)).await;
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read metrics body");
    let body = std::str::from_utf8(&body).expect("metrics are UTF-8");
    for expected in [
        "result=\"success\"} 2",
        "result=\"failure\"} 3",
        "published_bytes_total{sink=\"ipfs\"} 5",
        "last_ipns_update_timestamp_seconds 29",
        "validation_failure_total 31",
        "mirror_drift 37",
        "blocks{payload_kind=\"deal_settlement\"} 2",
    ] {
        assert!(body.contains(expected), "missing metric row: {expected}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires SORAFS_RUN_KUBO_INTEGRATION=1 and a local Kubo binary"]
async fn real_kubo_publication_ipns_restart_and_tamper_lane() {
    let kubo = KuboHarness::start().await;
    let endpoint = kubo.endpoint();
    assert_kubo_has_no_swarm_peers(&endpoint).await;
    let ipns_name = kubo_key_generate(&endpoint, KUBO_IPNS_KEY_ALIAS).await;

    let direct_payload = b"sorafs-governance-dag-real-kubo-integration-v1";
    let direct_cid = ipfs_add_verified(
        &endpoint,
        "direct-integration-object.to",
        direct_payload,
        1024 * 1024,
        1024 * 1024,
    )
    .await
    .expect("real Kubo add/pin/ls/cat roundtrip");
    assert!(is_canonical_cid_v1(&direct_cid));
    assert_eq!(
        ipfs_cat(
            &endpoint,
            &direct_cid,
            direct_payload.len() as u64,
            1024 * 1024
        )
        .await
        .expect("cat direct Kubo object"),
        direct_payload
    );
    assert!(
        ipfs_cat(
            &endpoint,
            &direct_cid,
            direct_payload.len() as u64 - 1,
            1024 * 1024,
        )
        .await
        .is_err(),
        "bounded cat must reject a real response larger than expected"
    );
    kubo_unpin(&endpoint, &direct_cid).await;
    assert!(
        ipfs_verify_pin(&endpoint, &direct_cid, 1024 * 1024)
            .await
            .is_err(),
        "real Kubo pin/ls must expose a removed recursive pin"
    );
    ipfs_pin(&endpoint, &direct_cid, 1024 * 1024)
        .await
        .expect("restore direct object pin");
    assert!(
        ipfs_cat(&endpoint, TEST_CID_ATTACKER, 1024, 1024)
            .await
            .is_err(),
        "unknown content-addressed bytes must fail closed"
    );

    let work = secure_temp_dir();
    let source_dir = work.path().join("source");
    let state_dir = work.path().join("state");
    let checkpoint_store = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));

    let first_timestamp = current_unix_timestamp_seconds().saturating_sub(5);
    let mut source = signed_source(3, 0x72, first_timestamp);
    materialize_source_snapshot(&source_dir, &mut source);
    seed_producer_checkpoint(&checkpoint_store, &source_dir, &source);
    let view = real_kubo_service_view(&source, &source_dir, &state_dir, &kubo.api_url, &ipns_name);

    let mut service = Service::from_view(
        view.clone(),
        test_runtime_providers(checkpoint_store.clone()),
    )
    .await
    .expect("initialize G-DAG service against real Kubo");
    service
        .reconcile_once()
        .await
        .expect("publish verified source through real Kubo and IPNS");
    let checkpoint = service
        .checkpoint
        .clone()
        .expect("first reconciliation persists checkpoint");
    assert_eq!(checkpoint.block_count, source.blocks.len() as u64);
    assert_eq!(checkpoint.mirror_blocks.len(), source.blocks.len());
    assert!(state_dir.join(MIRROR_INDEX_FILE).is_file());
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::PublishIntent)
            .expect("read integration sealed intent")
            .is_none()
    );
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::Checkpoint)
            .expect("read integration sealed checkpoint")
            .is_some()
    );
    for (published, block) in checkpoint.mirror_blocks.iter().zip(&source.blocks) {
        ipfs_verify_pin(&service.ipfs, &published.ipfs_cid, 1024 * 1024)
            .await
            .expect("real Kubo retains recursive block pin");
        assert_eq!(
            ipfs_cat(
                &service.ipfs,
                &published.ipfs_cid,
                block.bytes.len() as u64,
                1024 * 1024,
            )
            .await
            .expect("read real Kubo block"),
            block.bytes
        );
    }
    let public = resolve_ipns_head(&service.ipfs, &ipns_name, 1024 * 1024)
        .await
        .expect("resolve published IPNS head");
    assert!(matches!(
        &public,
        PublicHead::Present { bytes, token }
            if bytes == &source.head_bytes && token == &checkpoint.head_ipfs_cid
    ));

    fs::remove_file(state_dir.join(MIRROR_INDEX_FILE))
        .expect("remove mirror to exercise deterministic recovery");
    service
        .reconcile_once()
        .await
        .expect("steady-state reconciliation rebuilds missing mirror");
    assert!(state_dir.join(MIRROR_INDEX_FILE).is_file());

    kubo_unpin(&service.ipfs, &checkpoint.head_ipfs_cid).await;
    let missing_pin = service
        .reconcile_once()
        .await
        .expect_err("steady state must reject a missing real Kubo head pin");
    assert!(matches!(missing_pin, GovernanceDagServiceError::Network(_)));
    ipfs_pin(&service.ipfs, &checkpoint.head_ipfs_cid, 1024 * 1024)
        .await
        .expect("restore real Kubo head pin");
    service
        .reconcile_once()
        .await
        .expect("steady state recovers after head repin");

    let checkpoint_record = checkpoint_store
        .load(GovernanceDagSealedStateSlot::Checkpoint)
        .expect("read sealed checkpoint")
        .expect("sealed checkpoint exists");
    {
        let mut inner = checkpoint_store
            .inner
            .lock()
            .expect("lock integration store");
        let record = inner.checkpoint.as_mut().expect("checkpoint record");
        let tamper_position = record.payload.len() / 2;
        record.payload[tamper_position] ^= 0x80;
    }
    let checkpoint_error = service
        .reconcile_once()
        .await
        .expect_err("authenticated checkpoint tamper must fail closed");
    assert!(matches!(
        checkpoint_error,
        GovernanceDagServiceError::State(_)
    ));
    {
        let mut inner = checkpoint_store
            .inner
            .lock()
            .expect("lock integration store");
        inner.checkpoint = Some(checkpoint_record);
    }
    service
        .reconcile_once()
        .await
        .expect("restored authenticated checkpoint reconciles");

    drop(service);
    let mut restarted = Service::from_view(view, test_runtime_providers(checkpoint_store))
        .await
        .expect("restart G-DAG service from durable state");
    restarted
        .reconcile_once()
        .await
        .expect("restart verifies checkpoint, IPNS head, pins, and readback");
    assert_eq!(
        restarted
            .checkpoint
            .as_ref()
            .expect("restart loaded checkpoint")
            .generation,
        checkpoint.generation
    );
    assert!(restarted.api.0.read().await.ready);

    let attacker_bytes = b"concurrent-authorized-but-unexpected-ipns-head";
    let attacker_cid = ipfs_add_verified(
        &restarted.ipfs,
        "attacker-head.to",
        attacker_bytes,
        1024 * 1024,
        1024 * 1024,
    )
    .await
    .expect("publish adversarial head bytes to real Kubo");
    let current = resolve_ipns_head(&restarted.ipfs, &ipns_name, 1024 * 1024)
        .await
        .expect("read current IPNS head before adversarial movement");
    publish_ipns_head(
        &restarted.ipfs,
        IpnsHeadPublishRequest {
            name: &ipns_name,
            key_name: KUBO_IPNS_KEY_ALIAS,
            head_cid: &attacker_cid,
            bytes: attacker_bytes,
            initial: &current,
            allow_bootstrap: false,
            max_response_bytes: 1024 * 1024,
        },
    )
    .await
    .expect("move test IPNS name with its isolated key");
    let moved = restarted
        .reconcile_once()
        .await
        .expect_err("checkpoint reconciliation must reject unexpected IPNS movement");
    assert!(matches!(moved, GovernanceDagServiceError::Conflict(_)));

    let attacker = resolve_ipns_head(&restarted.ipfs, &ipns_name, 1024 * 1024)
        .await
        .expect("resolve adversarial IPNS value");
    publish_ipns_head(
        &restarted.ipfs,
        IpnsHeadPublishRequest {
            name: &ipns_name,
            key_name: KUBO_IPNS_KEY_ALIAS,
            head_cid: &checkpoint.head_ipfs_cid,
            bytes: &source.head_bytes,
            initial: &attacker,
            allow_bootstrap: false,
            max_response_bytes: 1024 * 1024,
        },
    )
    .await
    .expect("restore checkpointed IPNS value");
    restarted
        .reconcile_once()
        .await
        .expect("restored IPNS head returns service to steady state");

    eprintln!(
        "real Kubo G-DAG lane passed: direct_cid={direct_cid} head_cid={} ipns_name={ipns_name}",
        checkpoint.head_ipfs_cid
    );
    drop(restarted);
    kubo.shutdown();
}

#[test]
fn remote_head_validates_complete_prefix_and_rejects_checkpoint_tamper() {
    let source = signed_source(2, 0x39, current_unix_timestamp_seconds().saturating_sub(1));
    let dir = secure_temp_dir();
    let config = test_runtime_config(&source, dir.path());
    validate_remote_head(&source.head_bytes, &source, &config)
        .expect("canonical public head binds the complete source prefix");

    let signer = TestSigner::new(0x39);
    let mut tampered = source.head.clone();
    tampered.checkpoint_cid = Some(source.blocks[0].block.block_cid.clone());
    tampered.head_signature = signer.sign(
        &tampered
            .signature_payload_bytes()
            .expect("encode checkpoint-tampered head"),
    );
    let tampered_bytes = norito::to_bytes(&tampered).expect("encode checkpoint-tampered head");
    assert!(
        validate_remote_head(&tampered_bytes, &source, &config).is_err(),
        "a validly signed head with a noncanonical checkpoint must fail"
    );
}

#[test]
fn remote_head_rejects_future_timestamp() {
    let now = current_unix_timestamp_seconds();
    let signer = TestSigner::new(0x3c);
    let mut source = signed_source(1, 0x3c, now);
    source.head.generated_at = now + 120;
    source.head.head_signature = signer.sign(
        &source
            .head
            .signature_payload_bytes()
            .expect("encode future head"),
    );
    source.head_bytes = norito::to_bytes(&source.head).expect("encode future head");
    let dir = secure_temp_dir();
    let config = test_runtime_config(&source, dir.path());
    assert!(validate_remote_head(&source.head_bytes, &source, &config).is_err());
}
