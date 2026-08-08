const KUBO_IPNS_KEY_ALIAS: &str = "sorafs-gdag-integration";

fn real_kubo_ipns_service_view(
    source: &SourceSnapshot,
    source_dir: &Path,
    state_dir: &Path,
    api_url: &str,
    ipns_name: &str,
) -> SorafsGovernanceDagServiceView {
    let paths = [source_dir, state_dir];
    assert!(paths.iter().all(|path| {
        let path = path.to_string_lossy();
        !path.contains(['"', '\\', '\n', '\r'])
    }));
    let config = format!(
        r#"[sorafs.storage]
governance_dag_dir = "{}"
governance_dag_publisher_peer_id = "{TEST_PRODUCER_PEER_ID}"
governance_dag_signer_handle = "{TEST_PRODUCER_SIGNER_HANDLE}"
governance_dag_signer_revision = 1
governance_dag_signer_policy_digest_hex = "{}"
governance_dag_publisher_public_key_hex = "{}"

[sorafs.storage.governance_dag_service]
enabled = true
state_dir = "{}"
ipfs_api_url = "{}"
head_mode = "ipns"
ipns_name = "{}"
ipns_key_name = "{}"
ipfs_authenticator_handle = "{TEST_IPFS_AUTH_HANDLE}"
ipfs_authenticator_revision = 1
ipfs_authenticator_policy_digest_hex = "{}"
ipfs_request_auth_public_key_hex = "{}"
checkpoint_store_handle = "{TEST_CHECKPOINT_STORE_HANDLE}"
checkpoint_store_revision = 1
checkpoint_store_policy_digest_hex = "{}"
publisher_public_key_hex = "{}"
poll_interval_secs = 1
connect_timeout_ms = 5000
request_timeout_ms = 20000
dns_timeout_ms = 5000
max_request_bytes = {}
max_future_skew_secs = 60
allow_insecure_http = true
allow_private_ipfs_endpoint = true
allow_head_bootstrap = true
listen_addr = "127.0.0.1:0"
"#,
        source_dir.display(),
        "83".repeat(32),
        hex::encode(&source.head.head_signature.public_key),
        state_dir.display(),
        api_url,
        ipns_name,
        KUBO_IPNS_KEY_ALIAS,
        "81".repeat(32),
        hex::encode(test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE)),
        "82".repeat(32),
        hex::encode(&source.head.head_signature.public_key),
        BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1,
    );
    let config_path = state_dir
        .parent()
        .expect("integration state directory has parent")
        .join("governance-dag-service-ipns.toml");
    fs::write(&config_path, config).expect("write standalone IPNS G-DAG service config");
    load_service_config(&config_path).expect("parse standalone IPNS G-DAG service config")
}

async fn kubo_key_generate(endpoint: &PinnedEndpoint, alias: &str) -> String {
    let url = endpoint
        .ipfs_url(
            "api/v0/key/gen",
            &[("arg", alias), ("type", "ed25519"), ("ipns-base", "base36")],
        )
        .expect("construct Kubo key generation URL");
    let request = endpoint
        .request(Method::POST, url)
        .expect("construct Kubo key generation request");
    let response = endpoint
        .execute(request, "Kubo key generation request failed")
        .await
        .expect("send Kubo key generation request");
    assert!(response.status().is_success(), "Kubo key generation failed");
    let body = read_bounded_response(response, 64 * 1024)
        .await
        .expect("read Kubo key generation response");
    let value: JsonValue = json::from_slice(&body).expect("Kubo key response must be JSON");
    let name = value
        .get("Name")
        .and_then(JsonValue::as_str)
        .expect("Kubo key response has Name");
    assert_eq!(name, alias);
    validate_public_token(
        value
            .get("Id")
            .and_then(JsonValue::as_str)
            .expect("Kubo key response has Id"),
        "Kubo IPNS key id",
    )
    .expect("Kubo returns a canonical IPNS key id")
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
    let view =
        real_kubo_ipns_service_view(&source, &source_dir, &state_dir, &kubo.api_url, &ipns_name);

    let mut service = Service::from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_store.clone()),
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
    let (_, mirror_payload) =
        load_mirror_index_store(&service.config, &service.mirror_store)
            .expect("load committed mirror store");
    assert!(!mirror_payload.is_empty());
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

    let mirror_snapshot = service
        .mirror_store
        .load()
        .expect("load mirror snapshot before recovery test");
    let empty_mirror = encode_mirror_index_store_payload(&MirrorIndexStorePayloadV1::empty())
        .expect("encode empty mirror store payload");
    service
        .mirror_store
        .compare_and_swap(&mirror_snapshot, &empty_mirror)
        .expect("clear derived mirror to exercise deterministic recovery");
    service
        .reconcile_once()
        .await
        .expect("steady-state reconciliation rebuilds missing mirror");
    let (_, recovered_mirror) =
        load_mirror_index_store(&service.config, &service.mirror_store)
            .expect("load recovered mirror store");
    assert!(!recovered_mirror.is_empty());

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
    let restart_providers = test_runtime_providers(&view, checkpoint_store);
    let mut restarted = Service::from_view(view, restart_providers)
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

#[test]
fn publish_intent_progress_is_monotonic_within_one_generation() {
    let source = signed_source(2, 0x3b, 1_800_000_000);
    let mut prepared = intent_from_source(&source);
    for block in &mut prepared.blocks {
        block.ipfs_cid = None;
    }
    prepared.head_ipfs_cid = None;
    let mut progressed = prepared.clone();
    progressed.blocks[0].ipfs_cid = Some(
        canonical_ipfs_file_cid(&source.blocks[0].bytes)
            .expect("bounded source block has a deterministic IPFS file CID"),
    );
    assert!(publish_intent_is_monotonic_refinement(
        &prepared,
        &progressed
    ));

    let mut regressed = progressed.clone();
    regressed.blocks[0].ipfs_cid = None;
    assert!(!publish_intent_is_monotonic_refinement(
        &progressed,
        &regressed
    ));

    let mut equivocated = progressed.clone();
    equivocated.target_head_blake3[0] ^= 0x80;
    assert!(!publish_intent_is_monotonic_refinement(
        &progressed,
        &equivocated
    ));
}

#[test]
fn checkpoint_and_intent_sequence_validation_rejects_u64_exhaustion() {
    let source = signed_source(2, 0x3c, 1_800_000_000);

    let mut checkpoint = checkpoint_from_source(&source);
    checkpoint.mirror_blocks[0].sequence = u64::MAX;
    checkpoint.mirror_blocks[1].sequence = 0;
    let checkpoint_error = validate_checkpoint_body(&checkpoint)
        .expect_err("checkpoint sequence exhaustion must fail closed");
    assert!(matches!(
        checkpoint_error,
        GovernanceDagServiceError::State(_)
    ));

    let mut intent = intent_from_source(&source);
    intent.blocks[0].sequence = u64::MAX;
    intent.blocks[1].sequence = 0;
    let intent_error = validate_publish_intent(&intent)
        .expect_err("publish-intent sequence exhaustion must fail closed");
    assert!(matches!(intent_error, GovernanceDagServiceError::State(_)));
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
    let response = metrics_response(&state).await;
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

#[tokio::test]
async fn failed_object_repair_latches_mirror_drift_until_coherent_retry() {
    #[derive(Default)]
    struct RecoveryHttpInner {
        objects: BTreeMap<String, Vec<u8>>,
        required_cids: BTreeSet<String>,
        head: Option<Vec<u8>>,
        head_generation: u64,
        early_head_put: bool,
        reject_add: bool,
        add_count: u64,
    }

    type RecoveryHttpState = Arc<Mutex<RecoveryHttpInner>>;

    fn query_arg(raw: Option<&str>) -> Option<&str> {
        raw?.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "arg").then_some(value)
        })
    }

    async fn add(
        State(state): State<RecoveryHttpState>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        let Some(boundary) = headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("multipart/form-data; boundary="))
        else {
            return test_response(StatusCode::BAD_REQUEST, Body::empty());
        };
        let Some(payload_start) = body
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .and_then(|position| position.checked_add(4))
        else {
            return test_response(StatusCode::BAD_REQUEST, Body::empty());
        };
        let suffix = format!("\r\n--{boundary}--\r\n");
        if !body.ends_with(suffix.as_bytes()) || payload_start > body.len() - suffix.len() {
            return test_response(StatusCode::BAD_REQUEST, Body::empty());
        }
        let payload = body[payload_start..body.len() - suffix.len()].to_vec();
        let Some(cid) = canonical_ipfs_file_cid(&payload) else {
            return test_response(StatusCode::BAD_REQUEST, Body::empty());
        };
        let mut state = state.lock().await;
        if state.reject_add {
            return test_response(StatusCode::SERVICE_UNAVAILABLE, Body::empty());
        }
        state.objects.insert(cid.clone(), payload);
        state.add_count = state.add_count.saturating_add(1);
        test_response(StatusCode::OK, format!(r#"{{"Hash":"{cid}"}}"#))
    }

    async fn pin_add() -> Response {
        test_response(StatusCode::OK, "{}")
    }

    async fn pin_ls(
        State(state): State<RecoveryHttpState>,
        axum::extract::RawQuery(raw): axum::extract::RawQuery,
    ) -> Response {
        let Some(cid) = query_arg(raw.as_deref()) else {
            return test_response(StatusCode::BAD_REQUEST, Body::empty());
        };
        let present = state.lock().await.objects.contains_key(cid);
        let body = if present {
            format!(r#"{{"Keys":{{"{cid}":{{}}}}}}"#)
        } else {
            r#"{"Keys":{}}"#.to_owned()
        };
        test_response(StatusCode::OK, body)
    }

    async fn cat(
        State(state): State<RecoveryHttpState>,
        axum::extract::RawQuery(raw): axum::extract::RawQuery,
    ) -> Response {
        let Some(cid) = query_arg(raw.as_deref()) else {
            return test_response(StatusCode::BAD_REQUEST, Body::empty());
        };
        state.lock().await.objects.get(cid).cloned().map_or_else(
            || test_response(StatusCode::NOT_FOUND, Body::empty()),
            |bytes| test_response(StatusCode::OK, bytes),
        )
    }

    async fn head_get(State(state): State<RecoveryHttpState>) -> Response {
        let state = state.lock().await;
        let Some(bytes) = &state.head else {
            return test_response(StatusCode::NOT_FOUND, Body::empty());
        };
        let mut response = test_response(StatusCode::OK, bytes.clone());
        response.headers_mut().insert(
            header::ETAG,
            HeaderValue::from_str(&format!("\"{}\"", state.head_generation))
                .expect("canonical recovery ETag"),
        );
        response
    }

    async fn head_put(
        State(state): State<RecoveryHttpState>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        let mut state = state.lock().await;
        if !state
            .required_cids
            .iter()
            .all(|cid| state.objects.contains_key(cid))
        {
            state.early_head_put = true;
            return test_response(StatusCode::INTERNAL_SERVER_ERROR, Body::empty());
        }
        if state.head.is_some()
            || headers.get(header::IF_NONE_MATCH) != Some(&HeaderValue::from_static("*"))
        {
            return test_response(StatusCode::PRECONDITION_FAILED, Body::empty());
        }
        state.head = Some(body.to_vec());
        state.head_generation = state.head_generation.saturating_add(1);
        test_response(StatusCode::NO_CONTENT, Body::empty())
    }

    let http_state = RecoveryHttpState::default();
    let router = Router::new()
        .route("/api/v0/add", post(add))
        .route("/api/v0/pin/add", post(pin_add))
        .route("/api/v0/pin/ls", post(pin_ls))
        .route("/api/v0/cat", post(cat))
        .route("/head", get(head_get).put(head_put))
        .with_state(Arc::clone(&http_state));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind recovery publication fixture");
    let address = listener.local_addr().expect("recovery fixture address");
    let http_task = tokio::spawn(async move {
        let _ = axum::serve(listener, router.into_make_service()).await;
    });

    let work = secure_temp_dir();
    let source_dir = work.path().join("source");
    let state_dir = work.path().join("state");
    let mut source = signed_source(2, 0x74, current_unix_timestamp_seconds().saturating_sub(5));
    materialize_source_snapshot(&source_dir, &mut source);
    let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    seed_producer_checkpoint(&checkpoint_provider, &source_dir, &source);
    let intent = intent_from_source(&source);
    let required_cids = intent
        .blocks
        .iter()
        .filter_map(|block| block.ipfs_cid.clone())
        .chain(intent.head_ipfs_cid.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(required_cids.len(), source.blocks.len() + 1);
    save_publish_intent(
        &test_checkpoint_store(Arc::clone(&checkpoint_provider)),
        None,
        &intent,
    )
    .expect("seal a crash-resumed intent with every CID already filled");
    http_state.lock().await.required_cids = required_cids.clone();

    let base_url = format!("http://{address}");
    let signed_head_url = format!("{base_url}/head");
    let view = real_kubo_service_view(
        &source,
        &source_dir,
        &state_dir,
        &base_url,
        &signed_head_url,
    );
    let mut service = Service::from_view(
        view.clone(),
        test_runtime_providers(&view, Arc::clone(&checkpoint_provider)),
    )
    .await
    .expect("construct crash-recovery service");

    http_state.lock().await.reject_add = true;
    service
        .reconcile_once()
        .await
        .expect_err("failed object repair must withdraw mirror readiness");
    {
        let api = service.api.0.read().await;
        assert!(!api.ready);
        assert_eq!(
            api.metrics.mirror_drift, 1,
            "a failed reconciliation must latch observable mirror drift"
        );
    }
    http_state.lock().await.reject_add = false;
    service
        .reconcile_once()
        .await
        .expect("repair every prefilled object before the public-head CAS");
    {
        let api = service.api.0.read().await;
        assert!(api.ready);
        assert_eq!(
            api.metrics.mirror_drift, 0,
            "only a checkpoint-coherent successful reconciliation clears mirror drift"
        );
    }

    let state = http_state.lock().await;
    assert!(
        !state.early_head_put,
        "public head crossed CAS before repair"
    );
    assert_eq!(state.add_count as usize, required_cids.len());
    assert!(
        required_cids
            .iter()
            .all(|cid| state.objects.contains_key(cid))
    );
    assert_eq!(state.head.as_deref(), Some(source.head_bytes.as_slice()));
    drop(state);
    assert!(service.checkpoint.is_some());
    assert!(service.intent.is_none());
    http_task.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires SORAFS_RUN_KUBO_INTEGRATION=1 and a local Kubo binary"]
async fn real_kubo_publication_signed_head_restart_and_tamper_lane() {
    let kubo = KuboHarness::start().await;
    let endpoint = kubo.endpoint();
    assert_kubo_has_no_swarm_peers(&endpoint).await;
    let (head_endpoint, head_state, head_task) =
        spawn_signed_head(SignedHeadInner::default()).await;
    let signed_head_url = head_endpoint.url.to_string();

    let mut over_chunk_conformance = None;
    for (label, size) in [
        ("below-chunk", IPFS_UNIXFS_CHUNK_BYTES - 1),
        ("at-chunk", IPFS_UNIXFS_CHUNK_BYTES),
        ("over-chunk", IPFS_UNIXFS_CHUNK_BYTES + 1),
        ("max-object", GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1),
    ] {
        let payload = ipip_499_chacha20_bytes(label.as_bytes(), size);
        let expected_cid = canonical_ipfs_file_cid(&payload)
            .expect("Kubo conformance object fits the fixed UnixFS profile");
        let cid = ipfs_add_verified(
            &endpoint,
            &format!("fixed-unixfs-{label}.to"),
            &payload,
            payload.len() as u64,
            64 * 1024,
        )
        .await
        .unwrap_or_else(|err| panic!("real Kubo rejected {label} conformance vector: {err}"));
        assert_eq!(
            cid, expected_cid,
            "local UnixFS derivation diverged from Kubo for {label}"
        );
        if label == "over-chunk" {
            over_chunk_conformance = Some((payload, cid));
        }
    }
    let (direct_payload, direct_cid) =
        over_chunk_conformance.expect("the over-chunk conformance case ran");
    assert!(is_canonical_cid_v1(&direct_cid));
    assert_eq!(
        ipfs_cat(
            &endpoint,
            &direct_cid,
            direct_payload.len() as u64,
            64 * 1024
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
            64 * 1024,
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
    let view = real_kubo_service_view(
        &source,
        &source_dir,
        &state_dir,
        &kubo.api_url,
        &signed_head_url,
    );

    let mut service = Service::from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_store.clone()),
    )
    .await
    .expect("initialize G-DAG service against real Kubo");
    service
        .reconcile_once()
        .await
        .expect("publish verified source through real Kubo and signed-head CAS");
    let checkpoint = service
        .checkpoint
        .clone()
        .expect("first reconciliation persists checkpoint");
    assert_eq!(checkpoint.block_count, source.blocks.len() as u64);
    assert_eq!(checkpoint.mirror_blocks.len(), source.blocks.len());
    assert!(state_dir.join(MIRROR_INDEX_STORE_NAME).is_dir());
    assert!(
        !state_dir.join("mirror-index.json").exists(),
        "the first-release service must not dual-write the retired mirror file"
    );
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
    let public = service
        .fetch_public_head()
        .await
        .expect("fetch published signed HTTP head");
    assert!(matches!(
        &public,
        PublicHead::Present { bytes, token }
            if bytes == &source.head_bytes && strong_http_entity_tag(
                &HeaderValue::from_str(token).expect("signed-head ETag remains a header value")
            ).as_deref() == Some(token.as_str())
    ));

    let (mirror_snapshot, mirror_payload) =
        load_mirror_index_store(&service.config, &service.mirror_store)
            .expect("load published mirror payload");
    assert_eq!(mirror_payload.checkpoint_generation, checkpoint.generation);
    compare_and_swap_mirror_index_store(
        &service.config,
        &service.mirror_store,
        &mirror_snapshot,
        &MirrorIndexStorePayloadV1::empty(),
    )
    .expect("represent a hard-cut deployment without a local mirror payload");
    service
        .reconcile_once()
        .await
        .expect("steady-state reconciliation rebuilds an empty mirror store");
    let (_, recovered_payload) = load_mirror_index_store(&service.config, &service.mirror_store)
        .expect("load recovered mirror payload");
    assert_eq!(
        recovered_payload.checkpoint_generation,
        checkpoint.generation
    );
    assert_eq!(recovered_payload.mirror_blake3, checkpoint.mirror_blake3);

    kubo_unpin(&service.ipfs, &checkpoint.head_ipfs_cid).await;
    service
        .reconcile_once()
        .await
        .expect("steady state deterministically repairs a missing real Kubo head pin");
    ipfs_verify_pin(&service.ipfs, &checkpoint.head_ipfs_cid, 1024 * 1024)
        .await
        .expect("steady-state repair restores the recursive head pin");

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
    let mut restarted = Service::from_view(
        view.clone(),
        test_runtime_providers(&view, checkpoint_store),
    )
    .await
    .expect("restart G-DAG service from durable state");
    restarted
        .reconcile_once()
        .await
        .expect("restart verifies checkpoint, signed head, pins, and readback");
    assert_eq!(
        restarted
            .checkpoint
            .as_ref()
            .expect("restart loaded checkpoint")
            .generation,
        checkpoint.generation
    );
    assert!(restarted.api.0.read().await.ready);

    let attacker_bytes = b"concurrent-authorized-but-unexpected-signed-head";
    {
        let mut state = head_state.0.lock().await;
        state.bytes = Some(attacker_bytes.to_vec());
        state.etag = "\"attacker\"".to_owned();
    }
    let moved = restarted
        .reconcile_once()
        .await
        .expect_err("checkpoint reconciliation must reject unexpected signed-head movement");
    assert!(matches!(moved, GovernanceDagServiceError::Conflict(_)));

    {
        let mut state = head_state.0.lock().await;
        state.bytes = Some(source.head_bytes.clone());
        state.etag = "\"restored\"".to_owned();
    }
    restarted
        .reconcile_once()
        .await
        .expect("restored signed head returns service to steady state");

    eprintln!(
        "real Kubo G-DAG lane passed: direct_cid={direct_cid} head_cid={}",
        checkpoint.head_ipfs_cid
    );
    drop(restarted);
    head_task.abort();
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
