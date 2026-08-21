fn install_unavailable_local_read_runtime(
    app: &mut SharedAppState,
    local_peer_id: Option<String>,
    message: &'static str,
) {
    Arc::get_mut(app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime::unavailable(
        local_peer_id,
        message,
    )));
}
#[tokio::test]
async fn soracloud_public_split_app_routes_hosted_live_and_ordered_vault_updates_on_one_node() {
    use tower::ServiceExt as _;
    let TravelSplitTopologyFixture {
        world,
        snapshot,
        temp,
        live_peer_id,
        upstream_task,
    } = travel_split_topology_fixture(TravelSplitVaultMode::OrderedMailbox).await;
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestMailboxRuntime {
        snapshot,
        state_dir: temp.path().to_path_buf(),
        local_peer_id: Some(live_peer_id.to_string()),
        result: Ok(
            iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionResult {
                state_mutations: Vec::new(),
                outbound_mailbox_messages: Vec::new(),
                response_bytes: br#"{"status":"queued"}"#.to_vec(),
                content_type: Some("application/json".to_owned()),
                runtime_state: None,
                runtime_receipt: iroha_data_model::soracloud::SoraRuntimeReceiptV1 {
                    schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                    receipt_id: Hash::new(b"travel-vault-preferences-receipt"),
                    service_name: "travel_ops_vault".parse().expect("service"),
                    service_version: "2026.04.0".to_owned(),
                    handler_name: "preferences_put".parse().expect("handler"),
                    handler_class:
                        iroha_data_model::soracloud::SoraServiceHandlerClassV1::PrivateUpdate,
                    request_commitment: Hash::new(b"travel-vault-preferences-request"),
                    result_commitment: Hash::new(b"travel-vault-preferences-result"),
                    certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                    emitted_sequence: 1,
                    mailbox_message_id: Some(Hash::new(b"travel-vault-preferences-message")),
                    journal_artifact_hash: None,
                    checkpoint_artifact_hash: None,
                    placement_id: None,
                    selected_validator_account_id: None,
                    selected_peer_id: None,
                },
            },
        ),
        captured_requests: Arc::clone(&captured_requests),
    };
    let mut app = mk_app_state_for_tests_with_world(world);
    let app_mut = Arc::get_mut(&mut app).expect("unique app state");
    app_mut.local_peer_id = Some(live_peer_id);
    app_mut.soracloud_runtime = Some(Arc::new(runtime));
    let router = axum::Router::new()
        .fallback(any(handler_soracloud_public_local_read))
        .with_state(app);
    let live_response = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v1/search")
                .header(axum::http::header::HOST, "travel.sora")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("live request"),
        )
        .await
        .expect("live response");
    assert_eq!(live_response.status(), StatusCode::OK);
    let live_body = torii_body_bytes(live_response, "live body").await;
    assert_eq!(live_body.as_ref(), br#"{"source":"live"}"#);
    assert!(
        captured_requests.lock().expect("capture lock").is_empty(),
        "hosted live routes must bypass ordered mailbox execution"
    );
    let vault_payload = br#"{"home_airport":"BNE","cabin_preference":"business"}"#;
    let vault_response = router
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/api/v1/user/preferences")
                .header(axum::http::header::HOST, "travel.sora")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .extension(crate::loopback_connect_info())
                .body(Body::from(vault_payload.to_vec()))
                .expect("vault request"),
        )
        .await
        .expect("vault response");
    assert_eq!(vault_response.status(), StatusCode::OK);
    let vault_body = torii_body_bytes(vault_response, "vault body").await;
    assert_eq!(vault_body.as_ref(), br#"{"status":"queued"}"#);
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].deployment.service_name.as_ref(),
        "travel_ops_vault"
    );
    assert_eq!(
        captured[0]
            .handler
            .as_ref()
            .expect("handler")
            .handler_name
            .as_ref(),
        "preferences_put"
    );
    assert_eq!(
        captured[0].handler.as_ref().expect("handler").class,
        iroha_data_model::soracloud::SoraServiceHandlerClassV1::PrivateUpdate
    );
    assert_eq!(
        captured[0].mailbox_message.payload_bytes.as_slice(),
        vault_payload
    );
    assert_eq!(
        captured[0].mailbox_message.to_handler.as_ref(),
        "preferences_put"
    );
    assert_eq!(captured[0].authoritative_pending_mailbox_messages, 1);
    upstream_task.abort();
}
#[tokio::test]
async fn soracloud_public_local_read_route_returns_503_for_unhydrated_runtime() {
    use tower::ServiceExt as _;
    let mut app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    install_unavailable_local_read_runtime(&mut app, None, "runtime hydration incomplete");
    let router = axum::Router::new()
        .fallback(any(handler_soracloud_public_local_read))
        .with_state(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/app/query")
                .header(axum::http::header::HOST, "portal.sora")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
#[tokio::test]
async fn soracloud_public_ordered_mailbox_route_invokes_runtime_with_authoritative_context() {
    use tower::ServiceExt as _;
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestMailboxRuntime {
        snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
        state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
        local_peer_id: None,
        result: Ok(
            iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionResult {
                state_mutations: Vec::new(),
                outbound_mailbox_messages: Vec::new(),
                response_bytes: br#"{"status":"queued"}"#.to_vec(),
                content_type: Some("application/json".to_owned()),
                runtime_state: None,
                runtime_receipt: iroha_data_model::soracloud::SoraRuntimeReceiptV1 {
                    schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                    receipt_id: Hash::new(b"public-mailbox-receipt"),
                    service_name: "web_portal".parse().expect("service"),
                    service_version: "2026.02.0".to_owned(),
                    handler_name: "update".parse().expect("handler"),
                    handler_class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update,
                    request_commitment: Hash::new(b"public-mailbox-request"),
                    result_commitment: Hash::new(b"public-mailbox-result"),
                    certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                    emitted_sequence: 1,
                    mailbox_message_id: Some(Hash::new(b"public-mailbox-message")),
                    journal_artifact_hash: None,
                    checkpoint_artifact_hash: None,
                    placement_id: None,
                    selected_validator_account_id: None,
                    selected_peer_id: None,
                },
            },
        ),
        captured_requests: Arc::clone(&captured_requests),
    };
    let mut app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(runtime));
    let router = axum::Router::new()
        .fallback(any(handler_soracloud_public_local_read))
        .with_state(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/app/update/search?fresh=1")
                .header(axum::http::header::HOST, "portal.sora")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .extension(crate::loopback_connect_info())
                .body(Body::from(
                    br#"{"origin":"DXB","destination":"HIR"}"#.to_vec(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    let expected_receipt_id = Hash::new(b"public-mailbox-receipt").to_string();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        torii_response_header(&response, "content-type"),
        Some("application/json")
    );
    assert_eq!(
        torii_response_header(&response, "x-iroha-soracloud-receipt-id"),
        Some(expected_receipt_id.as_str())
    );
    let body = torii_body_bytes(response, "body").await;
    assert_eq!(body.as_ref(), br#"{"status":"queued"}"#);
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].deployment.service_name.as_ref(), "web_portal");
    assert_eq!(
        captured[0]
            .handler
            .as_ref()
            .expect("handler")
            .handler_name
            .as_ref(),
        "update"
    );
    assert_eq!(
        captured[0].handler.as_ref().expect("handler").class,
        iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update
    );
    assert_eq!(captured[0].execution_sequence, 1);
    assert_eq!(
        captured[0].mailbox_message.payload_bytes.as_slice(),
        br#"{"origin":"DXB","destination":"HIR"}"#
    );
    assert_eq!(captured[0].mailbox_message.to_handler.as_ref(), "update");
    assert_eq!(captured[0].authoritative_pending_mailbox_messages, 1);
}
#[tokio::test]
async fn soracloud_public_hosted_http_route_streams_sse_bodies() {
    use http_body_util::BodyExt as _;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tower::ServiceExt as _;
    const FIRST_SSE_FRAME: &[u8] = b"event: session\ndata: ready\n\n";
    const SECOND_SSE_FRAME: &[u8] = b"data: {\"id\":\"search_1\"}\n\n";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let addr = listener.local_addr().expect("upstream addr");
    let upstream_task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _addr)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                loop {
                    match socket.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => {
                            request.extend_from_slice(&buf[..n]);
                            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                                break;
                            }
                            if request.len() > 8192 {
                                return;
                            }
                        }
                    }
                }
                if socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n",
                        )
                        .await
                        .is_err()
                    {
                        return;
                    }
                let first_len = format!("{:x}\r\n", FIRST_SSE_FRAME.len());
                if socket.write_all(first_len.as_bytes()).await.is_err()
                    || socket.write_all(FIRST_SSE_FRAME).await.is_err()
                    || socket.write_all(b"\r\n").await.is_err()
                    || socket.flush().await.is_err()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                let second_len = format!("{:x}\r\n", SECOND_SSE_FRAME.len());
                let _ = socket.write_all(second_len.as_bytes()).await;
                let _ = socket.write_all(SECOND_SSE_FRAME).await;
                let _ = socket.write_all(b"\r\n0\r\n\r\n").await;
                let _ = socket.flush().await;
            });
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let temp = tempfile::tempdir().expect("tempdir");
    let materialization_dir = temp.path().join("service");
    std::fs::create_dir_all(&materialization_dir).expect("materialization dir");
    let listen_base_url = format!("http://{addr}");
    let mut world = seed_public_soracloud_world();
    let mut bundle = world
        .view()
        .soracloud_service_revisions()
        .get(&("web_portal".to_owned(), "2026.02.0".to_owned()))
        .cloned()
        .expect("public service bundle");
    bundle.container.runtime = iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou;
    bundle.container.inrou = Some(test_inrou_manifest());
    bundle.service.execution_plane =
        iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
    bundle.service.replicas = std::num::NonZeroU16::new(1).expect("replicas");
    bundle.service.state_bindings.clear();
    bundle.service.handlers.clear();
    bundle.service.lease_volumes = vec![
        iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("volume"),
            kind: iroha_data_model::soracloud::SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: std::num::NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("bytes"),
        },
        iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
            volume_name: "index_state".parse().expect("volume"),
            kind: iroha_data_model::soracloud::SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
            mount_path: "/var/lib/ton-indexer".to_owned(),
            max_total_bytes: std::num::NonZeroU64::new(1024 * 1024).expect("bytes"),
        },
    ];
    bundle.service.container.manifest_hash = bundle.container_manifest_hash();
    world.soracloud_service_revisions_mut_for_testing().insert(
        ("web_portal".to_owned(), "2026.02.0".to_owned()),
        bundle.clone(),
    );
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(
            "web_portal".parse().expect("service"),
            iroha_data_model::soracloud::SoraServiceDeploymentStateV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                service_name: "web_portal".parse().expect("service"),
                current_service_version: "2026.02.0".to_owned(),
                current_service_manifest_hash: bundle.service_manifest_hash(),
                current_container_manifest_hash: bundle.container_manifest_hash(),
                revision_count: 1,
                process_generation: 1,
                process_started_sequence: 1,
                active_rollout: None,
                last_rollout: None,
                config_generation: 0,
                secret_generation: 0,
                service_configs: BTreeMap::new(),
                service_secrets: BTreeMap::new(),
                fhe_policy_records: BTreeMap::new(),
                service_lease: Some(iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
                    status: iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
                    quota_class: "taira-open".to_owned(),
                    deployment_deposit: "1".parse().expect("deployment deposit"),
                    prepaid_runtime_balance: "50".parse().expect("runtime balance"),
                    runtime_price_per_sequence: "0.00025".parse().expect("runtime price"),
                    storage_price_per_gib_sequence: "0.000025".parse().expect("storage price"),
                    egress_price_per_mib: "0.000005".parse().expect("egress price"),
                    lease_started_sequence: 0,
                    lease_expires_sequence: 100,
                    last_billed_sequence: 0,
                    accounted_egress_bytes: 0,
                    last_status_reason: None,
                }),
                lease_volume_states: Vec::new(),
            },
        );
    let hosted_validator_account_id =
        checked_torii_test_account_id(0x49, "derive hosted SSE validator fixture key");
    let hosted_peer_id = checked_torii_test_peer_id(0x4a, "derive hosted SSE peer fixture key");
    seed_authoritative_hosted_http_revision(
        &mut world,
        &bundle,
        bundle.service.replicas.get(),
        &[(
            1,
            hosted_validator_account_id,
            hosted_peer_id.to_string(),
            iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        )],
    );
    let mut snapshot = iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default();
    snapshot.local_peer_id = Some(hosted_peer_id.to_string());
    snapshot.services.insert(
        "web_portal".to_owned(),
        BTreeMap::from([(
            "2026.02.0".to_owned(),
            iroha_core::soracloud_runtime::SoracloudRuntimeServicePlan {
                service_name: "web_portal".to_owned(),
                service_version: "2026.02.0".to_owned(),
                role: iroha_core::soracloud_runtime::SoracloudRuntimeRevisionRole::Active,
                traffic_percent: 100,
                runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou,
                execution_plane:
                    iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService,
                bundle_hash: Hash::new(b"native-public-bundle").to_string(),
                bundle_path: "/runtime/bin/launch.sh".to_owned(),
                entrypoint: "/runtime/bin/launch.sh".to_owned(),
                inrou: None,
                bundle_cache_path: temp.path().join("bundle.tar.gz").display().to_string(),
                bundle_available_locally: true,
                process_generation: Some(1),
                desired_replica_count: 1,
                local_replica_slots: vec![1],
                local_replicas: vec![SoracloudRuntimeReplicaPlan {
                    replica_slot: 1,
                    materialization_dir: materialization_dir
                        .join("replicas/replica-0001")
                        .display()
                        .to_string(),
                    health_status: iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
                    listen_base_url: Some(listen_base_url.clone()),
                    pid: Some(101),
                    last_error: None,
                }],
                health_status: iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
                load_factor_bps: 0,
                reported_pending_mailbox_messages: 0,
                authoritative_pending_mailbox_messages: 0,
                rollout_handle: None,
                config_generation: 0,
                secret_generation: 0,
                quota_class: Some("taira-open".to_owned()),
                service_lease_status: Some(
                    iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
                ),
                lease_expires_sequence: Some(100),
                remaining_runtime_balance: Some("50".parse().expect("runtime balance")),
                config_entry_count: 0,
                secret_entry_count: 0,
                config_exports: Vec::new(),
                supports_host_read_config: true,
                supports_host_read_secret_envelope: true,
                supports_private_secret_payload_reads: false,
                materialization_dir: materialization_dir.display().to_string(),
                config_materialization_dir: materialization_dir
                    .join("configs")
                    .display()
                    .to_string(),
                effective_env: BTreeMap::new(),
                effective_env_materialization_path: materialization_dir
                    .join("effective_env.json")
                    .display()
                    .to_string(),
                config_exports_materialization_dir: materialization_dir
                    .join("config_exports")
                    .display()
                    .to_string(),
                secret_envelopes_materialization_dir: materialization_dir
                    .join("secret_envelopes")
                    .display()
                    .to_string(),
                secret_payload_materialization_dir: materialization_dir
                    .join("secret_payloads")
                    .display()
                    .to_string(),
                lease_volumes: Vec::new(),
                mailboxes: Vec::new(),
                artifacts: Vec::new(),
            },
        )]),
    );
    let runtime = TestLocalReadRuntime::with_result(
        snapshot,
        temp.path().to_path_buf(),
        Some(hosted_peer_id.to_string()),
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "hosted HTTP proxy should bypass local-read execution",
            ),
        ),
    );
    let mut app = mk_app_state_for_tests_with_world(world);
    let app_mut = Arc::get_mut(&mut app).expect("unique app state");
    app_mut.local_peer_id = Some(hosted_peer_id);
    app_mut.soracloud_runtime = Some(Arc::new(runtime));
    let route_match = match soracloud::resolve_public_route(
        &app,
        "portal.sora",
        "GET",
        "/app/v1/search/search_1/events",
    )
    .expect("hosted SSE route")
    {
        soracloud::PublicRouteMatch::HostedHttp(route_match) => route_match,
        other => panic!("expected hosted route match, got {other:?}"),
    };
    assert_eq!(route_match.request_path, "/v1/search/search_1/events");
    let target = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(IpAddr::from([127, 0, 0, 1])),
        &HttpMethod::GET,
        &"/app/v1/search/search_1/events".parse().expect("uri"),
    )
    .expect("hosted SSE target");
    assert_eq!(
        target.local_listen_base_url.as_deref(),
        Some(listen_base_url.as_str())
    );
    let direct_response = tokio::time::timeout(
        Duration::from_secs(3),
        super::proxy_soracloud_public_hosted_http_locally(
            &HttpMethod::GET,
            &"/app/v1/search/search_1/events".parse().expect("uri"),
            &HeaderMap::new(),
            Bytes::new(),
            &route_match,
            &listen_base_url,
        ),
    )
    .await
    .expect("direct native proxy should not wait for the full SSE body")
    .expect("direct native proxy response");
    assert_eq!(direct_response.status(), StatusCode::OK);
    drop(direct_response);
    let router = axum::Router::new()
        .fallback(any(handler_soracloud_public_local_read))
        .with_state(app);
    let response = tokio::time::timeout(
        Duration::from_secs(3),
        router.oneshot(
            axum::http::Request::builder()
                .uri("/app/v1/search/search_1/events")
                .header(axum::http::header::HOST, "portal.sora")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        ),
    )
    .await
    .expect("native proxy should not wait for the full SSE body")
    .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        torii_response_header(&response, "content-type"),
        Some("text/event-stream")
    );
    let mut body = response.into_body();
    let first_frame = tokio::time::timeout(Duration::from_millis(300), body.frame())
        .await
        .expect("first SSE frame should be streamed promptly")
        .expect("body frame")
        .expect("streamed frame");
    let first_chunk = first_frame.into_data().expect("data frame");
    assert_eq!(first_chunk.as_ref(), FIRST_SSE_FRAME);
    let second_frame = tokio::time::timeout(Duration::from_secs(6), body.frame())
        .await
        .expect("second SSE frame should arrive")
        .expect("body frame")
        .expect("streamed frame");
    let second_chunk = second_frame.into_data().expect("data frame");
    assert_eq!(second_chunk.as_ref(), SECOND_SSE_FRAME);
    upstream_task.abort();
}
fn hosted_http_health_route(app: &SharedAppState) -> soracloud::HostedHttpRouteMatch {
    match soracloud::resolve_public_route(app, "portal.sora", "GET", "/app/v1/health")
        .expect("hosted route")
    {
        soracloud::PublicRouteMatch::HostedHttp(route_match) => route_match,
        other => panic!("expected hosted route match, got {other:?}"),
    }
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_routes_canary_traffic_by_rollout_percent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app = seed_public_hosted_http_rollout_app(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
    );
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let canary_ip = hosted_http_rollout_test_ip("web_portal", &method, &uri, |bucket| bucket < 20);
    let canary_target = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(canary_ip),
        &method,
        &uri,
    )
    .expect("healthy canary target");
    assert_eq!(canary_target.route_match.service_version, "2026.03.0");
    let baseline_ip =
        hosted_http_rollout_test_ip("web_portal", &method, &uri, |bucket| bucket >= 20);
    let baseline_target = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(baseline_ip),
        &method,
        &uri,
    )
    .expect("healthy baseline target");
    assert_eq!(baseline_target.route_match.service_version, "2026.02.0");
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_falls_back_to_baseline_when_canary_is_unhealthy() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app = seed_public_hosted_http_rollout_app(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
    );
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let canary_ip = hosted_http_rollout_test_ip("web_portal", &method, &uri, |bucket| bucket < 20);
    let selected_target = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(canary_ip),
        &method,
        &uri,
    )
    .expect("baseline should stay available");
    assert_eq!(selected_target.route_match.service_version, "2026.02.0");
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_fails_closed_without_any_healthy_revision() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app = seed_public_hosted_http_rollout_app(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
    );
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let baseline_ip =
        hosted_http_rollout_test_ip("web_portal", &method, &uri, |bucket| bucket >= 20);
    let error = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(baseline_ip),
        &method,
        &uri,
    )
    .expect_err("unhealthy revisions must fail closed");
    assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
    assert!(
        error
            .message
            .contains("no healthy hosted Soracloud revision"),
        "unexpected error: {}",
        error.message
    );
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_fails_closed_without_service_lease() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app = seed_public_hosted_http_rollout_app_with_service_lease(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        None,
    );
    let route_match = soracloud::HostedHttpRouteMatch {
        service_name: "web_portal".to_owned(),
        service_version: "2026.02.0".to_owned(),
        request_path: "/app/v1/health".to_owned(),
    };
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let error = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(IpAddr::from([127, 0, 0, 1])),
        &method,
        &uri,
    )
    .expect_err("missing hosted-service lease must fail closed");
    assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
    assert!(
        error
            .message
            .contains("lease for service `web_portal` is unavailable"),
        "unexpected error: {}",
        error.message
    );
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_fails_closed_when_service_lease_expires() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app = seed_public_hosted_http_rollout_app_with_service_lease(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        Some(hosted_http_service_lease_state(
            iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
            "50".parse().expect("runtime balance"),
            1,
        )),
    );
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let error = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(IpAddr::from([127, 0, 0, 1])),
        &method,
        &uri,
    )
    .expect_err("expired hosted-service lease must fail closed");
    assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
    assert!(
        error.message.contains("Expired"),
        "unexpected error: {}",
        error.message
    );
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_fails_closed_when_service_lease_is_exhausted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app = seed_public_hosted_http_rollout_app_with_service_lease(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        Some(hosted_http_service_lease_state(
            iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
            "0.00025".parse().expect("runtime balance"),
            100,
        )),
    );
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let error = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(IpAddr::from([127, 0, 0, 1])),
        &method,
        &uri,
    )
    .expect_err("exhausted hosted-service lease must fail closed");
    assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
    assert!(
        error.message.contains("Exhausted"),
        "unexpected error: {}",
        error.message
    );
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_balances_across_healthy_replicas_within_revision() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app = seed_public_hosted_http_rollout_app_with_local_replicas(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
        vec![
            hosted_http_runtime_replica_plan(
                &temp.path().join("service-baseline"),
                1,
                iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
                Some("http://127.0.0.1:18080"),
                Some(101),
            ),
            hosted_http_runtime_replica_plan(
                &temp.path().join("service-baseline"),
                2,
                iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
                Some("http://127.0.0.1:18081"),
                Some(102),
            ),
        ],
        vec![hosted_http_runtime_replica_plan(
            &temp.path().join("service-canary"),
            1,
            iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
            None,
            None,
        )],
    );
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let first_ip =
        hosted_http_replica_test_ip("web_portal", "2026.02.0", &method, &uri, |bucket| {
            bucket % 2 == 0
        });
    let first_target = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(first_ip),
        &method,
        &uri,
    )
    .expect("first replica target");
    assert_eq!(
        first_target.local_listen_base_url.as_deref(),
        Some("http://127.0.0.1:18080")
    );
    let second_ip =
        hosted_http_replica_test_ip("web_portal", "2026.02.0", &method, &uri, |bucket| {
            bucket % 2 == 1
        });
    let second_target = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(second_ip),
        &method,
        &uri,
    )
    .expect("second replica target");
    assert_eq!(
        second_target.local_listen_base_url.as_deref(),
        Some("http://127.0.0.1:18081")
    );
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_uses_local_snapshot_when_authoritative_runtime_state_lags()
 {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = seed_public_hosted_http_rollout_app(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
    );
    let baseline_bundle = app
        .state
        .view()
        .world()
        .soracloud_service_revisions()
        .get(&("web_portal".to_owned(), "2026.02.0".to_owned()))
        .cloned()
        .expect("baseline bundle");
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        seed_authoritative_hosted_http_revision(
            &mut state.world,
            &baseline_bundle,
            baseline_bundle.service.replicas.get(),
            &[(
                1,
                ALICE_ID.clone(),
                app_mut
                    .local_peer_id
                    .as_ref()
                    .expect("local peer id")
                    .to_string(),
                iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
            )],
        );
    }
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let baseline_ip =
        hosted_http_rollout_test_ip("web_portal", &method, &uri, |bucket| bucket >= 20);
    let target = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(baseline_ip),
        &method,
        &uri,
    )
    .expect("healthy assigned local runtime snapshot should bridge authoritative lag");
    assert_eq!(target.route_match.service_version, "2026.02.0");
    assert_eq!(
        target.local_listen_base_url.as_deref(),
        Some("http://127.0.0.1:18080")
    );
}
#[tokio::test]
async fn resolve_hosted_http_runtime_target_fails_closed_without_snapshot_replica_targets() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app = seed_public_hosted_http_rollout_app_with_local_replicas(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        Vec::new(),
        Vec::new(),
    );
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let error = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(IpAddr::from([203, 0, 113, 77])),
        &method,
        &uri,
    )
    .expect_err("runtime snapshot without local replicas must fail closed");
    assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
    assert!(
        error
            .message
            .contains("no healthy hosted Soracloud revision"),
        "unexpected error: {}",
        error.message
    );
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn resolve_hosted_http_runtime_target_rejects_snapshot_from_different_peer() {
    let temp = tempfile::tempdir().expect("tempdir");
    let remote_peer_id =
        checked_torii_test_peer_id(0x4b, "derive hosted HTTP remote snapshot peer fixture key");
    let local_peer_id =
        checked_torii_test_peer_id(0x4c, "derive hosted HTTP local snapshot peer fixture key");
    let mut app = seed_public_hosted_http_rollout_app_with_local_replicas_and_snapshot_peer_id(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
        vec![hosted_http_runtime_replica_plan(
            &temp.path().join("service-baseline"),
            1,
            iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
            Some("http://127.0.0.1:18080"),
            Some(101),
        )],
        vec![hosted_http_runtime_replica_plan(
            &temp.path().join("service-canary"),
            1,
            iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
            None,
            None,
        )],
        Some(remote_peer_id.to_string()),
        Some(hosted_http_service_lease_state(
            iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
            "50".parse().expect("runtime balance"),
            100,
        )),
    );
    let baseline_bundle = app
        .state
        .view()
        .world()
        .soracloud_service_revisions()
        .get(&("web_portal".to_owned(), "2026.02.0".to_owned()))
        .cloned()
        .expect("baseline bundle");
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .local_peer_id = Some(local_peer_id.clone());
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        let local_validator_account_id = checked_torii_test_account_id(
            0x4d,
            "derive hosted HTTP local snapshot validator fixture key",
        );
        seed_authoritative_hosted_http_revision(
            &mut state.world,
            &baseline_bundle,
            baseline_bundle.service.replicas.get(),
            &[(
                1,
                local_validator_account_id,
                local_peer_id.to_string(),
                iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
            )],
        );
    }
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let error = super::resolve_hosted_http_runtime_target(
        &app,
        &route_match,
        Some(IpAddr::from([203, 0, 113, 99])),
        &method,
        &uri,
    )
    .expect_err("foreign snapshot origin must fail closed");
    assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
    assert!(
        error.message.contains(remote_peer_id.to_string().as_str()),
        "unexpected error: {}",
        error.message
    );
    assert!(
        error.message.contains(local_peer_id.to_string().as_str()),
        "unexpected error: {}",
        error.message
    );
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn hosted_http_proxy_candidate_peers_exclude_local_and_visited() {
    let local_keypair = checked_torii_test_bls_keypair(
        0x4e,
        "derive hosted HTTP local proxy-candidate peer fixture key",
    );
    let first_remote_keypair = checked_torii_test_bls_keypair(
        0x4f,
        "derive hosted HTTP first remote proxy-candidate peer fixture key",
    );
    let second_remote_keypair = checked_torii_test_bls_keypair(
        0x50,
        "derive hosted HTTP second remote proxy-candidate peer fixture key",
    );
    let local_peer_id = PeerId::from(local_keypair.public_key().clone());
    let first_remote_peer_id = PeerId::from(first_remote_keypair.public_key().clone());
    let second_remote_peer_id = PeerId::from(second_remote_keypair.public_key().clone());
    let mut app = mk_app_state_for_tests();
    let (online_tx, online_rx) = tokio::sync::watch::channel(HashSet::new());
    online_tx
        .send(HashSet::from([
            Peer::new(
                "127.0.0.1:20001".parse().expect("valid local address"),
                local_keypair.public_key().clone(),
            ),
            Peer::new(
                "127.0.0.1:20002".parse().expect("valid remote address"),
                first_remote_keypair.public_key().clone(),
            ),
            Peer::new(
                "127.0.0.1:20003".parse().expect("valid remote address"),
                second_remote_keypair.public_key().clone(),
            ),
        ]))
        .expect("online peers update should succeed");
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .online_peers = OnlinePeersProvider::new(online_rx);
    let candidates = super::hosted_http_proxy_candidate_peer_ids(
        app.as_ref(),
        &local_peer_id,
        &[first_remote_peer_id.clone(), second_remote_peer_id.clone()],
        std::slice::from_ref(&second_remote_peer_id),
    );
    assert_eq!(candidates.peers, vec![first_remote_peer_id]);
    assert_eq!(candidates.loop_prevention_drops, 1);
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn proxy_soracloud_public_hosted_http_falls_back_to_remote_peer() {
    let temp = tempfile::tempdir().expect("tempdir");
    let local_keypair =
        checked_torii_test_bls_keypair(0x51, "derive hosted HTTP local fallback peer fixture key");
    let remote_keypair =
        checked_torii_test_bls_keypair(0x52, "derive hosted HTTP remote fallback peer fixture key");
    let local_peer_id = PeerId::from(local_keypair.public_key().clone());
    let remote_peer_id = PeerId::from(remote_keypair.public_key().clone());
    let mut app = seed_public_hosted_http_rollout_app_with_local_replicas_and_snapshot_peer_id(
        &temp,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1::Unavailable,
        Vec::new(),
        Vec::new(),
        Some(local_peer_id.to_string()),
        Some(hosted_http_service_lease_state(
            iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
            "50".parse().expect("runtime balance"),
            100,
        )),
    );
    let baseline_bundle = app
        .state
        .view()
        .world()
        .soracloud_service_revisions()
        .get(&("web_portal".to_owned(), "2026.02.0".to_owned()))
        .cloned()
        .expect("baseline bundle");
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let (online_tx, online_rx) = tokio::sync::watch::channel(HashSet::new());
        online_tx
            .send(HashSet::from([
                Peer::new(
                    "127.0.0.1:21001".parse().expect("valid local address"),
                    local_keypair.public_key().clone(),
                ),
                Peer::new(
                    "127.0.0.1:21002".parse().expect("valid remote address"),
                    remote_keypair.public_key().clone(),
                ),
            ]))
            .expect("online peers update should succeed");
        app_mut.online_peers = OnlinePeersProvider::new(online_rx);
        app_mut.local_peer_id = Some(local_peer_id.clone());
        app_mut.p2p = Some(iroha_core::IrohaNetwork::closed_for_tests());
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        let remote_validator_account_id = checked_torii_test_account_id(
            0x53,
            "derive hosted HTTP remote fallback validator fixture key",
        );
        seed_authoritative_hosted_http_revision(
            &mut state.world,
            &baseline_bundle,
            baseline_bundle.service.replicas.get(),
            &[(
                1,
                remote_validator_account_id,
                remote_peer_id.to_string(),
                iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
            )],
        );
    }
    let route_match = hosted_http_health_route(&app);
    let method = HttpMethod::GET;
    let uri: axum::http::Uri = "/app/v1/health".parse().expect("uri");
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::HOST,
        HeaderValue::from_static("portal.sora"),
    );
    headers.insert("x-test-forward", HeaderValue::from_static("1"));
    let app_for_response = app.clone();
    let remote_peer_for_response = remote_peer_id.clone();
    let response_task = tokio::spawn(async move {
        let request_id = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let pending = app_for_response.torii_proxy_pending.lock().await;
                if let Some((request_id, _peer_id)) = pending
                    .keys()
                    .find(|(_request_id, peer_id)| *peer_id == remote_peer_for_response)
                {
                    break *request_id;
                }
                drop(pending);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("hosted HTTP proxy request should become pending");
        super::process_incoming_torii_proxy_response(
            &app_for_response,
            remote_peer_for_response,
            ToriiProxyResponseV1 {
                schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
                request_id,
                response: ToriiProxyHttpResponseV1 {
                    status_code: StatusCode::OK.as_u16(),
                    headers: vec![iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                        name: "content-type".to_owned(),
                        value: b"text/plain".to_vec(),
                    }],
                    body: b"remote-hosted-http".to_vec(),
                },
            },
        )
        .await;
    });
    let response = super::proxy_soracloud_public_hosted_http(
        State(app),
        method,
        uri,
        headers,
        Bytes::from_static(b"remote-body"),
        route_match,
        Some(IpAddr::from([203, 0, 113, 77])),
    )
    .await;
    response_task
        .await
        .expect("proxy response task should complete");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        torii_response_header(&response, "content-type"),
        Some("text/plain")
    );
    let body = torii_body_bytes(response, "response body should be readable").await;
    assert_eq!(body.as_ref(), b"remote-hosted-http");
}
fn sample_generated_hf_infer_request(
    service_name: String,
    service_version: String,
) -> SoracloudLocalReadRequest {
    let mut request = SoracloudLocalReadRequest {
        observed_height: 1,
        observed_block_hash: None,
        service_name,
        service_version,
        handler_name: "infer".to_owned(),
        handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
        request_method: "POST".to_owned(),
        request_path: "/infer".to_owned(),
        handler_path: "/".to_owned(),
        request_query: None,
        request_headers: BTreeMap::new(),
        request_body: b"{\"input\":\"hello\"}".to_vec(),
        request_commitment: Hash::new(b""),
    };
    request.request_commitment = super::soracloud_local_read_request_commitment(&request);
    request
}
fn sample_public_query_request(
    service_name: String,
    service_version: String,
) -> SoracloudLocalReadRequest {
    let mut request = SoracloudLocalReadRequest {
        observed_height: 1,
        observed_block_hash: None,
        service_name,
        service_version,
        handler_name: "query".to_owned(),
        handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
        request_method: "GET".to_owned(),
        request_path: "/app/query".to_owned(),
        handler_path: "/".to_owned(),
        request_query: None,
        request_headers: BTreeMap::new(),
        request_body: Vec::new(),
        request_commitment: Hash::new(b""),
    };
    request.request_commitment = super::soracloud_local_read_request_commitment(&request);
    request
}
fn sample_generated_hf_proxy_response(
    app: &SharedAppState,
    request: &SoracloudLocalReadRequest,
) -> iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
    let world = app.state.world_view();
    let bundle = world
        .soracloud_service_revisions()
        .get(&(
            request.service_name.clone(),
            request.service_version.clone(),
        ))
        .expect("generated HF service fixture bundle");
    let binding = iroha_core::soracloud_runtime::soracloud_hf_generated_source_binding(bundle)
        .expect("generated HF binding");
    let placement = iroha_core::soracloud_runtime::resolve_generated_hf_active_placement(
        &world,
        request.service_name.as_str(),
        &binding.source_id,
    )
    .expect("generated HF placement lookup")
    .expect("generated HF placement");
    let primary_assignment = placement
        .assigned_hosts
        .iter()
        .find(|assignment| {
            assignment.role == iroha_data_model::soracloud::SoraHfPlacementHostRoleV1::Primary
                && assignment.status
                    == iroha_data_model::soracloud::SoraHfPlacementHostStatusV1::Warm
        })
        .expect("generated HF primary assignment");
    let result_commitment = Hash::new(b"generated-hf-proxy-result");
    iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
        response_bytes: br#"{"text":"hello"}"#.to_vec(),
        content_type: Some("application/json".to_owned()),
        content_encoding: None,
        cache_control: None,
        bindings: Vec::new(),
        result_commitment,
        certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::AuditReceipt,
        runtime_receipt: Some(iroha_data_model::soracloud::SoraRuntimeReceiptV1 {
            schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
            receipt_id: Hash::new(b"generated-hf-proxy-receipt"),
            service_name: bundle.service.service_name.clone(),
            service_version: bundle.service.service_version.clone(),
            handler_name: "infer".parse().expect("infer handler"),
            handler_class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query,
            request_commitment: request.request_commitment,
            result_commitment,
            certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::AuditReceipt,
            emitted_sequence: 1,
            placement_id: Some(placement.placement_id),
            selected_validator_account_id: Some(primary_assignment.validator_account_id.clone()),
            selected_peer_id: Some(primary_assignment.peer_id.clone()),
            mailbox_message_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
        }),
    }
}
fn active_generated_hf_replica_peer_id(
    world: &World,
    service_name: &str,
    service_version: &str,
) -> String {
    let world = world.view();
    let bundle = world
        .soracloud_service_revisions()
        .get(&(service_name.to_owned(), service_version.to_owned()))
        .expect("generated HF service fixture bundle");
    let binding = iroha_core::soracloud_runtime::soracloud_hf_generated_source_binding(bundle)
        .expect("generated HF binding");
    let placement = iroha_core::soracloud_runtime::resolve_generated_hf_active_placement(
        &world,
        service_name,
        &binding.source_id,
    )
    .expect("generated HF placement lookup")
    .expect("generated HF placement");
    placement
        .assigned_hosts
        .iter()
        .find(|assignment| {
            assignment.role == iroha_data_model::soracloud::SoraHfPlacementHostRoleV1::Replica
                && assignment.status
                    != iroha_data_model::soracloud::SoraHfPlacementHostStatusV1::Retired
        })
        .expect("generated HF replica assignment")
        .peer_id
        .clone()
}
#[tokio::test]
async fn resolve_soracloud_local_read_proxy_target_returns_primary_when_local_is_not_primary() {
    let primary_peer_id =
        checked_torii_test_peer_id(0x54, "derive generated-HF primary proxy target fixture key")
            .to_string();
    let local_peer_id =
        checked_torii_test_peer_id(0x55, "derive generated-HF local proxy target fixture key")
            .to_string();
    let (world, service_name, service_version) = seed_generated_hf_public_world(&primary_peer_id);
    let mut app = mk_app_state_for_tests_with_world(world);
    install_unavailable_local_read_runtime(
        &mut app,
        Some(local_peer_id),
        "proxy target tests should not execute the runtime locally",
    );
    let target = super::resolve_soracloud_local_read_proxy_target(
        &app,
        &sample_generated_hf_infer_request(service_name, service_version),
    )
    .expect("proxy target resolution should succeed")
    .expect("non-primary ingress should proxy to the authoritative primary");
    assert_eq!(target.to_string(), primary_peer_id);
}
#[tokio::test]
async fn resolve_soracloud_local_read_proxy_target_skips_proxy_when_local_is_primary() {
    let local_primary_peer_id = checked_torii_test_peer_id(
        0x56,
        "derive generated-HF local primary proxy target fixture key",
    )
    .to_string();
    let (world, service_name, service_version) =
        seed_generated_hf_public_world(&local_primary_peer_id);
    let mut app = mk_app_state_for_tests_with_world(world);
    install_unavailable_local_read_runtime(
        &mut app,
        Some(local_primary_peer_id),
        "proxy target tests should not execute the runtime locally",
    );
    let target = super::resolve_soracloud_local_read_proxy_target(
        &app,
        &sample_generated_hf_infer_request(service_name, service_version),
    )
    .expect("proxy target resolution should succeed");
    assert!(
        target.is_none(),
        "local primary ingress should execute without proxying"
    );
}
#[tokio::test]
async fn soracloud_proxy_response_completes_pending_request() {
    let app = mk_app_state_for_tests();
    let request_id = Hash::new(b"soracloud-proxy-response");
    let responder_peer_id = checked_torii_test_peer_id(
        0x57,
        "derive generated-HF proxy response responder fixture key",
    );
    let response = iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
        response_bytes: b"proxied-body".to_vec(),
        content_type: Some("text/plain".to_owned()),
        content_encoding: None,
        cache_control: None,
        bindings: Vec::new(),
        result_commitment: Hash::new(b"proxied-result"),
        certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::AuditReceipt,
        runtime_receipt: None,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.soracloud_proxy_pending.lock().await.insert(
        request_id,
        PendingSoracloudProxyRequest {
            sender: tx,
            expected_peer_id: responder_peer_id.clone(),
            request: sample_public_query_request("web_portal".to_owned(), "2026.02.0".to_owned()),
        },
    );
    super::process_incoming_soracloud_proxy_response(
        &app,
        responder_peer_id,
        SoracloudLocalReadProxyResponseV1 {
            schema_version: SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1,
            request_id,
            outcome: SoracloudLocalReadProxyOutcomeV1::Ok(response.clone()),
        },
    )
    .await;
    match rx.await.expect("pending proxy request should resolve") {
        SoracloudLocalReadProxyOutcomeV1::Ok(delivered) => {
            assert_eq!(delivered.response_bytes, response.response_bytes);
            assert_eq!(delivered.result_commitment, response.result_commitment);
        }
        SoracloudLocalReadProxyOutcomeV1::Err(error) => {
            panic!("expected successful proxied response, got error: {error:?}")
        }
    }
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn authoritative_lane_peers_require_explicit_bindings_for_permissioned_routes() {
    let local_keypair =
        checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
    let remote_keypair =
        checked_torii_test_ed25519_keypair(0x59, "derive authoritative-lane remote fixture key");
    let local_peer_id = PeerId::from(local_keypair.public_key().clone());
    let remote_peer_id = PeerId::from(remote_keypair.public_key().clone());
    let identityless_app = mk_app_state_for_tests();
    assert!(
        !super::is_local_authoritative_for_peers(
            identityless_app.as_ref(),
            std::slice::from_ref(&remote_peer_id),
        ),
        "a missing local peer identity must never authorize a nonempty lane roster"
    );
    assert!(
        !super::should_execute_route_locally(
            identityless_app.as_ref(),
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        ),
        "an unresolved default-lane roster must not authorize an identityless local executor"
    );
    let mut app = mk_app_state_for_tests();
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let (online_tx, online_rx) = tokio::sync::watch::channel(std::collections::HashSet::new());
        let local_peer = Peer::new(
            "127.0.0.1:10001".parse().expect("valid local address"),
            local_keypair.public_key().clone(),
        );
        let remote_peer = Peer::new(
            "127.0.0.1:10002".parse().expect("valid remote address"),
            remote_keypair.public_key().clone(),
        );
        online_tx
            .send(HashSet::from([local_peer, remote_peer]))
            .expect("online peers update should succeed");
        app_mut.online_peers = OnlinePeersProvider::new(online_rx);
        app_mut.local_peer_id = Some(local_peer_id.clone());
    }
    {
        let mut topology = app.state.commit_topology.block();
        topology.clear();
        topology.push(local_peer_id.clone());
        topology.push(remote_peer_id.clone());
        topology.commit();
    }
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = app.state.block(header);
    let mut peers = block.world.peers_mut_for_testing().transaction();
    peers.clear();
    peers.push(local_peer_id.clone());
    peers.push(remote_peer_id.clone());
    peers.apply();
    block.commit().expect("commit permissioned peer roster");
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let authoritative = super::authoritative_lane_peers(app.as_ref(), route).authoritative;
    assert!(
        authoritative.is_empty(),
        "permissioned public routes should fail closed without explicit authoritative bindings"
    );
    assert!(
        !super::is_local_authoritative_for_route(app.as_ref(), route),
        "permissioned public ingress should not infer authority from commit topology"
    );
    assert!(
        !super::should_execute_route_locally(app.as_ref(), route),
        "the default lane must fail closed when no authoritative bindings exist"
    );
}
struct TwoOnlinePeerFixture {
    app: SharedAppState,
    local_peer_id: PeerId,
    remote_peer_id: PeerId,
}
fn two_online_peer_fixture() -> TwoOnlinePeerFixture {
    let local_keypair =
        checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
    let remote_keypair =
        checked_torii_test_ed25519_keypair(0x59, "derive authoritative-lane remote fixture key");
    let local_peer_id = PeerId::from(local_keypair.public_key().clone());
    let remote_peer_id = PeerId::from(remote_keypair.public_key().clone());
    let mut app = mk_app_state_for_tests();
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let (online_tx, online_rx) = tokio::sync::watch::channel(std::collections::HashSet::new());
        let local_peer = Peer::new(
            "127.0.0.1:10001".parse().expect("valid local address"),
            local_keypair.public_key().clone(),
        );
        let remote_peer = Peer::new(
            "127.0.0.1:10002".parse().expect("valid remote address"),
            remote_keypair.public_key().clone(),
        );
        online_tx
            .send(HashSet::from([local_peer, remote_peer]))
            .expect("online peers update should succeed");
        app_mut.online_peers = OnlinePeersProvider::new(online_rx);
        app_mut.local_peer_id = Some(local_peer_id.clone());
    }
    TwoOnlinePeerFixture {
        app,
        local_peer_id,
        remote_peer_id,
    }
}
fn install_two_peer_npos_roster(
    app: &SharedAppState,
    local_peer_id: &PeerId,
    remote_peer_id: &PeerId,
) {
    {
        let mut topology = app.state.commit_topology.block();
        topology.clear();
        topology.push(local_peer_id.clone());
        topology.push(remote_peer_id.clone());
        topology.commit();
    }
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = app.state.block(header);
    block
        .world
        .parameters
        .get_mut()
        .set_parameter(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ));
    let mut peers = block.world.peers_mut_for_testing().transaction();
    peers.clear();
    peers.push(local_peer_id.clone());
    peers.push(remote_peer_id.clone());
    peers.apply();
    block.commit().expect("commit npos peer roster");
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn authoritative_lane_peers_do_not_fall_back_to_commit_topology_for_npos_core_lane() {
    let TwoOnlinePeerFixture {
        app,
        local_peer_id,
        remote_peer_id,
    } = two_online_peer_fixture();
    install_two_peer_npos_roster(&app, &local_peer_id, &remote_peer_id);
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let authoritative = super::authoritative_lane_peers(app.as_ref(), route).authoritative;
    assert!(
        authoritative.is_empty(),
        "NPoS core-lane routing should fail closed when no authoritative bindings are present"
    );
    assert!(
        !super::is_local_authoritative_for_route(app.as_ref(), route),
        "NPoS core-lane ingress should no longer infer authority from commit topology"
    );
    assert!(
        !super::should_execute_route_locally(app.as_ref(), route),
        "the default NPoS lane must fail closed without explicit bindings"
    );
}
#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
#[tokio::test]
async fn one_lane_without_authoritative_binding_returns_route_unavailable() {
    let TwoOnlinePeerFixture {
        app,
        local_peer_id: _,
        remote_peer_id: _,
    } = two_online_peer_fixture();
    let nexus = app.state.nexus_snapshot();
    assert_eq!(
        nexus.lane_catalog.lanes().len(),
        1,
        "fixture must exercise the one-lane catalog"
    );
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let request = ToriiProxyRequestKindV4::Read(super::torii_read_request(
        ToriiReadEndpointV1::AccountGet,
        route,
        vec![
            checked_torii_test_account_id(
                0x5A,
                "derive unresolved one-lane routed-read account fixture key",
            )
            .to_string(),
        ],
        None,
        Vec::new(),
    ));
    let response = super::execute_torii_proxy_request_with_fallback(&app, route, request).await;
    assert_route_unavailable_response(&response);
    assert_eq!(
        torii_response_header(&response, "x-iroha-route-unavailable-reason"),
        Some("missing_authoritative_binding")
    );
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn authoritative_lane_peers_do_not_fall_back_to_online_peers_when_state_is_empty() {
    let TwoOnlinePeerFixture {
        app,
        local_peer_id: _,
        remote_peer_id: _,
    } = two_online_peer_fixture();
    {
        let mut topology = app.state.commit_topology.block();
        topology.clear();
        topology.commit();
    }
    {
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        block
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(
                SumeragiNposParameters::default().into_custom_parameter(),
            ));
        let mut peers = block.world.peers_mut_for_testing().transaction();
        peers.clear();
        peers.apply();
        block.commit().expect("commit empty npos peer roster");
    }
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let authoritative = super::authoritative_lane_peers(app.as_ref(), route).authoritative;
    assert!(
        authoritative.is_empty(),
        "empty state snapshots should leave authoritative routing unresolved"
    );
    assert!(
        !super::is_local_authoritative_for_route(app.as_ref(), route),
        "empty state snapshots should not infer local authority from connected peers"
    );
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn authoritative_lane_peers_do_not_fall_back_for_npos_non_core_lane() {
    let TwoOnlinePeerFixture {
        app,
        local_peer_id,
        remote_peer_id,
    } = two_online_peer_fixture();
    install_two_peer_npos_roster(&app, &local_peer_id, &remote_peer_id);
    let route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
    let authoritative = super::authoritative_lane_peers(app.as_ref(), route).authoritative;
    assert!(
        authoritative.is_empty(),
        "non-core NPoS routes should still require explicit public validator records"
    );
    assert!(
        !super::is_local_authoritative_for_route(app.as_ref(), route),
        "non-core NPoS routes should not treat commit topology peers as authoritative"
    );
    assert!(
        !super::should_execute_route_locally(app.as_ref(), route),
        "non-core routes should continue to fail closed without explicit bindings"
    );
}
struct AdminManagedLaneFixture {
    app: SharedAppState,
    nexus: iroha_config::parameters::actual::Nexus,
    local_validator: AccountId,
    remote_validator: AccountId,
    local_peer_id: PeerId,
    remote_peer_id: PeerId,
    additional_remote_authorities: Vec<(AccountId, PeerId)>,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
}
fn admin_managed_lane_fixture() -> AdminManagedLaneFixture {
    let local_validator_keypair = checked_torii_test_ed25519_keypair(
        0x5a,
        "derive authoritative-lane local validator fixture key",
    );
    let remote_validator_keypair = checked_torii_test_ed25519_keypair(
        0x5b,
        "derive authoritative-lane remote validator fixture key",
    );
    let local_peer_keypair =
        checked_torii_test_bls_keypair(0x5c, "derive authoritative-lane local peer fixture key");
    let remote_peer_keypair =
        checked_torii_test_bls_keypair(0x5d, "derive authoritative-lane remote peer fixture key");
    let additional_remote_authorities = (0_u8..3)
        .map(|index| {
            let validator_keypair = checked_torii_test_ed25519_keypair(
                0x80 + index,
                "derive additional admin-lane validator fixture key",
            );
            let peer_keypair = checked_torii_test_bls_keypair(
                0x83 + index,
                "derive additional admin-lane peer fixture key",
            );
            (
                AccountId::new(validator_keypair.public_key().clone()),
                peer_keypair,
            )
        })
        .collect::<Vec<_>>();
    let local_validator = AccountId::new(local_validator_keypair.public_key().clone());
    let remote_validator = AccountId::new(remote_validator_keypair.public_key().clone());
    let local_peer_id = PeerId::from(local_peer_keypair.public_key().clone());
    let remote_peer_id = PeerId::from(remote_peer_keypair.public_key().clone());
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(1);
    let mut app = mk_app_state_for_tests();
    let nexus = {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let (online_tx, online_rx) = tokio::sync::watch::channel(std::collections::HashSet::new());
        online_tx
            .send(std::collections::HashSet::from([
                Peer::new(
                    "127.0.0.1:10001".parse().expect("valid local address"),
                    local_peer_keypair.public_key().clone(),
                ),
                Peer::new(
                    "127.0.0.1:10002".parse().expect("valid remote address"),
                    remote_peer_keypair.public_key().clone(),
                ),
            ]))
            .expect("online peers update should succeed");
        app_mut.online_peers = OnlinePeersProvider::new(online_rx);
        app_mut.local_peer_id = Some(local_peer_id.clone());
        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                iroha_data_model::nexus::LaneConfig {
                    id: lane_id,
                    dataspace_id,
                    alias: format!("lane-{}", lane_id.as_u32()),
                    visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        let dataspace_catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: dataspace_id,
                alias: "restricted".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let nexus = iroha_config::parameters::actual::Nexus {
            lane_catalog,
            dataspace_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        };
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        state.set_nexus(nexus.clone()).expect("apply nexus config");
        ensure_runtime_peer_binding_for_test(state, &local_validator, &local_peer_keypair, "local");
        ensure_runtime_peer_binding_for_test(
            state,
            &remote_validator,
            &remote_peer_keypair,
            "remote",
        );
        for (index, (validator, peer_keypair)) in additional_remote_authorities.iter().enumerate() {
            ensure_runtime_peer_binding_for_test(
                state,
                validator,
                peer_keypair,
                &format!("remote-{}", index + 2),
            );
        }
        nexus
    };
    let additional_remote_authorities = additional_remote_authorities
        .into_iter()
        .map(|(validator, peer_keypair)| {
            (validator, PeerId::from(peer_keypair.public_key().clone()))
        })
        .collect();
    AdminManagedLaneFixture {
        app,
        nexus,
        local_validator,
        remote_validator,
        local_peer_id,
        remote_peer_id,
        additional_remote_authorities,
        lane_id,
        dataspace_id,
    }
}
fn exact_admin_committee_with_local(
    local_validator: &AccountId,
    local_peer_id: &PeerId,
    remote_validator: &AccountId,
    remote_peer_id: &PeerId,
    additional_remote_authorities: &[(AccountId, PeerId)],
) -> Vec<(AccountId, PeerId)> {
    let mut committee = vec![
        (local_validator.clone(), local_peer_id.clone()),
        (remote_validator.clone(), remote_peer_id.clone()),
    ];
    committee.extend(additional_remote_authorities.iter().take(2).cloned());
    assert_eq!(committee.len(), 4, "f=1 fixture committee must be 3f+1");
    committee
}
fn exact_remote_admin_committee(
    remote_validator: &AccountId,
    remote_peer_id: &PeerId,
    additional_remote_authorities: &[(AccountId, PeerId)],
) -> Vec<(AccountId, PeerId)> {
    let mut committee = vec![(remote_validator.clone(), remote_peer_id.clone())];
    committee.extend(additional_remote_authorities.iter().cloned());
    assert_eq!(committee.len(), 4, "f=1 fixture committee must be 3f+1");
    committee
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn authoritative_lane_peers_use_manifest_validators_for_admin_managed_lane() {
    let AdminManagedLaneFixture {
        mut app,
        nexus,
        local_validator,
        remote_validator,
        local_peer_id,
        remote_peer_id,
        additional_remote_authorities,
        lane_id,
        dataspace_id,
    } = admin_managed_lane_fixture();
    let exact_committee = exact_admin_committee_with_local(
        &local_validator,
        &local_peer_id,
        &remote_validator,
        &remote_peer_id,
        &additional_remote_authorities,
    );
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        {
            let mut topology = state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.push(remote_peer_id.clone());
            topology.commit();
        }
        install_lane_manifest_registry_for_test(state, &[(lane_id, exact_committee)]);
        let state_view = app_mut.state.view();
        app_mut.queue.reconfigure_nexus(&nexus, &state_view, None);
    }
    let route = RoutingDecision::new(lane_id, dataspace_id);
    let authoritative = super::authoritative_lane_peers(app.as_ref(), route).authoritative;
    assert!(
        authoritative.contains(&local_peer_id),
        "manifest-backed restricted lane should treat the local validator as authoritative"
    );
    assert!(
        authoritative.contains(&remote_peer_id),
        "manifest-backed restricted lane should include the remote validator"
    );
    assert_eq!(
        authoritative.len(),
        4,
        "f=1 restricted lane authority must resolve to exactly 3f+1 validators"
    );
    assert!(
        super::is_local_authoritative_for_route(app.as_ref(), route),
        "manifest-backed restricted lane should be routable without staking records"
    );
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn authoritative_lane_peers_use_pinned_committee_after_autoscale_activation() {
    let local_keypair =
        checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
    let authoritative_validator_keypair = checked_torii_test_ed25519_keypair(
        0x5e,
        "derive authoritative-lane manifest validator fixture key",
    );
    let authoritative_peer_keypair =
        checked_torii_test_bls_keypair(0x5f, "derive authoritative-lane manifest peer fixture key");
    let local_peer_id = PeerId::from(local_keypair.public_key().clone());
    let authoritative_validator =
        AccountId::new(authoritative_validator_keypair.public_key().clone());
    let authoritative_peer_id = PeerId::from(authoritative_peer_keypair.public_key().clone());
    let pinned_keypairs = (0x76_u8..=0x79)
        .map(|seed| {
            checked_torii_test_bls_keypair(seed, "derive activated autoscale peer fixture key")
        })
        .collect::<Vec<_>>();
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let mut app = mk_app_state_for_tests();
    let pinned_peer_ids;
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let (online_tx, online_rx) = tokio::sync::watch::channel(std::collections::HashSet::new());
        online_tx
            .send(std::collections::HashSet::from([
                Peer::new(
                    "127.0.0.1:10001".parse().expect("valid local address"),
                    local_keypair.public_key().clone(),
                ),
                Peer::new(
                    "127.0.0.1:10002"
                        .parse()
                        .expect("valid authoritative address"),
                    authoritative_peer_keypair.public_key().clone(),
                ),
            ]))
            .expect("online peers update should succeed");
        app_mut.online_peers = OnlinePeersProvider::new(online_rx);
        app_mut.local_peer_id = Some(local_peer_id.clone());
        let mut autoscale_lane = iroha_data_model::nexus::LaneConfig {
            id: lane_id,
            alias: format!("elastic-lane-{}", lane_id.as_u32()),
            ..iroha_data_model::nexus::LaneConfig::default()
        };
        autoscale_lane.metadata.insert(
            iroha_data_model::nexus::AUTOSCALE_META_MANAGED.to_owned(),
            "true".to_owned(),
        );
        autoscale_lane.metadata.insert(
            iroha_data_model::nexus::AUTOSCALE_META_CREATED_HEIGHT.to_owned(),
            "7".to_owned(),
        );
        pinned_peer_ids =
            pin_autoscale_lane_committee_for_test(&mut autoscale_lane, &pinned_keypairs);
        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                autoscale_lane,
            ],
        )
        .expect("autoscale lane catalog");
        let mut nexus = iroha_config::parameters::actual::Nexus {
            lane_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        };
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("non-zero min lanes");
        nexus.autoscale.max_lanes = NonZeroU32::new(2).expect("non-zero max lanes");
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        {
            let mut current = state.nexus.write();
            *current = nexus.clone();
        }
        ensure_runtime_peer_binding_for_test(
            state,
            &authoritative_validator,
            &authoritative_peer_keypair,
            "authoritative",
        );
        {
            let mut topology = state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.push(authoritative_peer_id.clone());
            topology.commit();
        }
        install_lane_manifest_registry_for_test(
            state,
            &[(
                lane_id,
                vec![(authoritative_validator, authoritative_peer_id.clone())],
            )],
        );
        state.update_latest_block_header_cache_for_tests(BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        ));
    }
    let route = RoutingDecision::new(lane_id, dataspace_id);
    assert!(
        super::authoritative_lane_peers(app.as_ref(), route)
            .authoritative
            .is_empty(),
        "future-created autoscale manifest bindings must not bypass active-height authority"
    );
    app.state
        .update_latest_block_header_cache_for_tests(BlockHeader::new(
            NonZeroU64::new(7).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        ));
    assert_eq!(
        super::authoritative_lane_peers(app.as_ref(), route).authoritative,
        pinned_peer_ids,
        "the immutable committee, not mutable manifest bindings, must become authoritative at the autoscale creation height"
    );
    assert!(
        !pinned_peer_ids.contains(&authoritative_peer_id),
        "fixture manifest authority must remain disjoint from the pinned committee"
    );
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn manifest_backed_admin_managed_lane_ignores_local_commit_topology_filtering() {
    let AdminManagedLaneFixture {
        mut app,
        nexus,
        local_validator: _,
        remote_validator,
        local_peer_id,
        remote_peer_id,
        additional_remote_authorities,
        lane_id,
        dataspace_id,
    } = admin_managed_lane_fixture();
    let exact_remote_committee = exact_remote_admin_committee(
        &remote_validator,
        &remote_peer_id,
        &additional_remote_authorities,
    );
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        {
            let mut topology = state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.commit();
        }
        install_lane_manifest_registry_for_test(state, &[(lane_id, exact_remote_committee)]);
        let state_view = app_mut.state.view();
        app_mut.queue.reconfigure_nexus(&nexus, &state_view, None);
    }
    let route = RoutingDecision::new(lane_id, dataspace_id);
    let authoritative = super::authoritative_lane_peers(app.as_ref(), route);
    let candidates =
        super::torii_proxy_candidate_peer_ids(app.as_ref(), &local_peer_id, route, None, &[]);
    assert!(
        authoritative.authoritative.contains(&remote_peer_id),
        "manifest-backed admin-managed lanes should keep remote authorities even when the local commit topology omits them"
    );
    assert_eq!(
        authoritative.authoritative.len(),
        4,
        "f=1 remote committee must retain exact 3f+1 authority outside local topology"
    );
    assert!(
        !super::is_local_authoritative_for_route(app.as_ref(), route),
        "a peer outside the lane manifest should not become authoritative just because it is local"
    );
    assert!(
        !super::should_execute_route_locally(app.as_ref(), route),
        "restricted routes without local manifest authority should still proxy"
    );
    assert_eq!(
        candidates.peers,
        vec![ToriiProxyCandidate::P2p(remote_peer_id)],
        "Torii proxy candidate discovery should route to the manifest-backed remote authority"
    );
}
#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
#[tokio::test]
async fn incoming_proxy_reads_and_fanout_are_terminal_when_route_ownership_is_stale() {
    let AdminManagedLaneFixture {
        mut app,
        nexus,
        local_validator: _,
        remote_validator,
        local_peer_id: _,
        remote_peer_id,
        additional_remote_authorities,
        lane_id,
        dataspace_id,
    } = admin_managed_lane_fixture();
    let exact_remote_committee = exact_remote_admin_committee(
        &remote_validator,
        &remote_peer_id,
        &additional_remote_authorities,
    );
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        install_lane_manifest_registry_for_test(state, &[(lane_id, exact_remote_committee)]);
        let state_view = app_mut.state.view();
        app_mut.queue.reconfigure_nexus(&nexus, &state_view, None);
    }
    let route = RoutingDecision::new(lane_id, dataspace_id);
    assert!(
        !super::should_execute_route_locally(app.as_ref(), route),
        "test requires a route the receiver would otherwise re-forward"
    );
    let verified_query = ToriiProxyRequestKindV4::SignedQueryRouteScan {
        query_bytes: Vec::new(),
        expected_route: ToriiRouteHintV1::from(route),
        response_format: ToriiProxyResponseFormatV1::Norito,
    };
    let read_request = ToriiProxyRequestKindV4::Read(super::torii_read_request(
        ToriiReadEndpointV1::AccountGet,
        route,
        vec![
            checked_torii_test_account_id(
                0x67,
                "derive authoritative-lane proxied read account fixture key",
            )
            .to_string(),
        ],
        None,
        Vec::new(),
    ));
    let read_fanout = ToriiProxyRequestKindV4::ReadFanout(super::torii_read_fanout_request(
        ToriiReadEndpointV1::AccountGet,
        ToriiFanoutRouteScopeV1::AllDataspaces,
        ToriiReadFanoutMergeV1::Account,
        vec![ALICE_ID.to_string()],
        None,
        Vec::new(),
        ToriiProxyResponseFormatV1::Json,
    ));
    let signed_query = ToriiProxyRequestKindV4::SignedQuery {
        query_bytes: Vec::new(),
        expected_route: ToriiRouteHintV1::from(route),
        response_format: ToriiProxyResponseFormatV1::Norito,
    };
    assert!(
        super::should_execute_incoming_torii_proxy_request_locally(
            app.as_ref(),
            &verified_query,
            route,
        ),
        "signed route scans should execute on the ingress-selected receiver"
    );
    assert!(
        super::should_execute_incoming_torii_proxy_request_locally(
            app.as_ref(),
            &read_request,
            route,
        ),
        "proxied app-api reads should execute on the ingress-selected receiver"
    );
    assert!(
        super::should_execute_incoming_torii_proxy_request_locally(
            app.as_ref(),
            &read_fanout,
            route,
        ),
        "a stale/cyclic ownership view cannot re-forward an ingress-selected read fanout back to its sender"
    );
    assert!(
        !super::should_execute_incoming_torii_proxy_request_locally(
            app.as_ref(),
            &signed_query,
            route,
        ),
        "write-path signed queries must still honor local authority checks"
    );
}
#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
async fn incoming_read_proxy_response_for_route(
    app: SharedAppState,
    route: RoutingDecision,
) -> Response {
    let ingress_peer_id = checked_torii_test_peer_id(
        0x67,
        "derive stale-route proxied read ingress peer fixture key",
    );
    let request = ToriiProxyRequestV6 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V6,
        request_id: Hash::new(b"incoming-read-proxy-stale-route"),
        deadline_unix_ms: super::torii_proxy_test_deadline_unix_ms(),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: vec![ingress_peer_id],
        request: ToriiProxyRequestKindV4::Read(super::torii_read_request(
            ToriiReadEndpointV1::AccountGet,
            route,
            vec![
                checked_torii_test_account_id(
                    0x68,
                    "derive stale-route proxied read account fixture key",
                )
                .to_string(),
            ],
            None,
            Vec::new(),
        )),
    };
    super::execute_incoming_torii_proxy_request(&app, request, None).await
}
#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
async fn incoming_verified_query_proxy_response_for_route(
    app: SharedAppState,
    route: RoutingDecision,
) -> Response {
    let key_pair =
        checked_torii_test_ed25519_keypair(0x69, "derive stale-route verified query key");
    let ingress_peer_id = PeerId::from(key_pair.public_key().clone());
    let authority = AccountId::new(key_pair.public_key().clone());
    let signed_query = authorize_query_for_test(
        iroha_data_model::query::QueryRequest::Start(build_find_triggers_query_for_test()),
        authority,
    )
    .sign(&key_pair);
    let request = ToriiProxyRequestV6 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V6,
        request_id: Hash::new(b"incoming-verified-query-proxy-stale-route"),
        deadline_unix_ms: super::torii_proxy_test_deadline_unix_ms(),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: vec![ingress_peer_id],
        request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
            query_bytes: iroha_version::codec::EncodeVersioned::encode_versioned(&signed_query),
            expected_route: ToriiRouteHintV1::from(route),
            response_format: ToriiProxyResponseFormatV1::Norito,
        },
    };
    super::execute_incoming_torii_proxy_request(&app, request, None).await
}
#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
fn assert_incoming_proxy_stale_route_rejection(response: &Response, route: RoutingDecision) {
    let expected_lane = route.lane_id.as_u32().to_string();
    let expected_dataspace = route.dataspace_id.as_u64().to_string();
    assert_route_unavailable_response(&response);
    assert_eq!(
        torii_response_header(&response, "x-iroha-route-unavailable-reason"),
        Some("stale_route")
    );
    assert_eq!(
        torii_response_header(&response, "x-iroha-route-lane-id"),
        Some(expected_lane.as_str())
    );
    assert_eq!(
        torii_response_header(&response, "x-iroha-route-dataspace-id"),
        Some(expected_dataspace.as_str())
    );
}
#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
#[tokio::test]
async fn incoming_read_proxy_rejects_retired_lane_hint() {
    let app = mk_app_state_for_tests();
    let route = RoutingDecision::new(LaneId::new(42), DataSpaceId::UNIVERSAL);
    let response = incoming_read_proxy_response_for_route(app, route).await;
    assert_incoming_proxy_stale_route_rejection(&response, route);
}
#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
#[tokio::test]
async fn incoming_read_proxy_rejects_lane_dataspace_mismatch_hint() {
    let mut app = mk_app_state_for_tests();
    configure_multiple_dataspace_routes_for_test(&mut app);
    let route = RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL);
    let response = incoming_read_proxy_response_for_route(app, route).await;
    assert_incoming_proxy_stale_route_rejection(&response, route);
}
