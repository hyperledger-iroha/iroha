#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn torii_proxy_snapshot_roundtrips_status_headers_and_body() {
    let mut response = Response::new(Body::from("proxied-body"));
    *response.status_mut() = StatusCode::ACCEPTED;
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain"),
    );
    response.headers_mut().insert(
        axum::http::HeaderName::from_static("x-iroha-routed-by"),
        HeaderValue::from_static("proxy"),
    );

    let snapshot = super::response_to_torii_proxy_snapshot(response, usize::MAX).await;
    let restored = super::torii_proxy_snapshot_to_response(snapshot);
    let headers = restored.headers().clone();
    assert_eq!(restored.status(), StatusCode::ACCEPTED);
    assert_eq!(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain")
    );
    assert_eq!(
        headers
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok()),
        Some("proxy")
    );
    let body = axum::body::to_bytes(restored.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    assert_eq!(body.as_ref(), b"proxied-body");
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn torii_proxy_snapshot_caps_buffered_response_bodies() {
    let response = Response::new(Body::from("proxied-body"));

    let snapshot = super::response_to_torii_proxy_snapshot(response, 4).await;

    assert_eq!(snapshot.status_code, StatusCode::BAD_GATEWAY.as_u16());
    let body = String::from_utf8(snapshot.body).expect("error body is utf8");
    assert!(
        body.contains("configured limit of 4 bytes"),
        "unexpected cap error body: {body}"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn torii_proxy_snapshot_restore_drops_invalid_headers_and_status() {
    let snapshot = ToriiProxyHttpResponseV1 {
        status_code: 99,
        headers: vec![
            iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                name: "x-valid-proxy-header".to_owned(),
                value: b"kept".to_vec(),
            },
            iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                name: "bad header".to_owned(),
                value: b"dropped".to_vec(),
            },
            iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                name: "x-invalid-value".to_owned(),
                value: b"bad\nvalue".to_vec(),
            },
        ],
        body: b"restored".to_vec(),
    };

    let restored = super::torii_proxy_snapshot_to_response(snapshot);

    assert_eq!(restored.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        restored
            .headers()
            .get("x-valid-proxy-header")
            .and_then(|value| value.to_str().ok()),
        Some("kept")
    );
    assert!(restored.headers().get("bad header").is_none());
    assert!(restored.headers().get("x-invalid-value").is_none());
    let body = axum::body::to_bytes(restored.into_body(), usize::MAX)
        .await
        .expect("restored body");
    assert_eq!(body.as_ref(), b"restored");
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn torii_proxy_header_conversion_preserves_duplicates_and_skips_invalid() {
    let mut headers = HeaderMap::new();
    headers.append(
        axum::http::HeaderName::from_static("x-repeat"),
        HeaderValue::from_static("one"),
    );
    headers.append(
        axum::http::HeaderName::from_static("x-repeat"),
        HeaderValue::from_static("two"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("x-single"),
        HeaderValue::from_static("kept"),
    );

    let mut proxy_headers = super::header_map_to_torii_proxy_headers(&headers);
    proxy_headers.push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
        name: "bad header".to_owned(),
        value: b"dropped".to_vec(),
    });
    proxy_headers.push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
        name: "x-bad-value".to_owned(),
        value: b"bad\r\nvalue".to_vec(),
    });
    let restored = super::torii_proxy_headers_to_header_map(&proxy_headers);
    let repeated = restored
        .get_all("x-repeat")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>();

    assert_eq!(repeated, vec!["one", "two"]);
    assert_eq!(
        restored
            .get("x-single")
            .and_then(|value| value.to_str().ok()),
        Some("kept")
    );
    assert!(restored.get("bad header").is_none());
    assert!(restored.get("x-bad-value").is_none());
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn torii_proxy_snapshot_accepts_exact_limit_and_preserves_headers() {
    let mut response = Response::new(Body::from("four"));
    *response.status_mut() = StatusCode::CREATED;
    response.headers_mut().insert(
        axum::http::HeaderName::from_static("x-proxy-test"),
        HeaderValue::from_static("kept"),
    );

    let snapshot = super::response_to_torii_proxy_snapshot(response, 4).await;

    assert_eq!(snapshot.status_code, StatusCode::CREATED.as_u16());
    assert_eq!(snapshot.body, b"four");
    assert!(
        snapshot
            .headers
            .iter()
            .any(|header| { header.name == "x-proxy-test" && header.value.as_slice() == b"kept" }),
        "exact-limit responses should keep proxied headers"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn reqwest_torii_proxy_snapshot_caps_buffered_bridge_response_bodies() {
    let upstream = axum::Router::new().route(
        "/oversized",
        axum::routing::get(|| async { Response::new(Body::from("proxied-body")) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let addr = listener.local_addr().expect("upstream addr");
    let upstream_task = tokio::spawn(async move {
        axum::serve(listener, upstream.into_make_service())
            .await
            .expect("serve upstream");
    });

    let response = reqwest::get(format!("http://{addr}/oversized"))
        .await
        .expect("fetch upstream response");
    let error = match super::reqwest_response_to_torii_proxy_snapshot(response, 4, false).await {
        Ok(_) => panic!("expected capped response error"),
        Err(error) => error,
    };
    upstream_task.abort();

    assert!(
        error.contains("configured limit of 4 bytes"),
        "unexpected cap error: {error}"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn reqwest_torii_proxy_snapshot_accepts_exact_limit_bridge_response() {
    let upstream = axum::Router::new().route(
        "/exact",
        axum::routing::get(|| async {
            let mut response = Response::new(Body::from("four"));
            *response.status_mut() = StatusCode::PARTIAL_CONTENT;
            response.headers_mut().insert(
                axum::http::HeaderName::from_static("x-upstream-test"),
                HeaderValue::from_static("kept"),
            );
            response
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let addr = listener.local_addr().expect("upstream addr");
    let upstream_task = tokio::spawn(async move {
        axum::serve(listener, upstream.into_make_service())
            .await
            .expect("serve upstream");
    });

    let response = reqwest::get(format!("http://{addr}/exact"))
        .await
        .expect("fetch upstream response");
    let snapshot = super::reqwest_response_to_torii_proxy_snapshot(response, 4, false)
        .await
        .expect("exact-limit response should be accepted");
    upstream_task.abort();

    assert_eq!(snapshot.status_code, StatusCode::PARTIAL_CONTENT.as_u16());
    assert_eq!(snapshot.body, b"four");
    assert!(
        snapshot.headers.iter().any(|header| {
            header.name == "x-upstream-test" && header.value.as_slice() == b"kept"
        }),
        "exact-limit bridge responses should keep proxied headers"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn torii_proxy_response_body_limit_caps_hosted_http_and_strict_receipts() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique app state");
    app_mut.soracloud_public_max_response_bytes = 0;
    app_mut.transaction_max_content_len = 1;
    let route = RoutingDecision::new(LaneId::new(9), DataSpaceId::new(12));
    let hosted_request = ToriiProxyRequestKindV4::HostedHttp(ToriiHostedHttpProxyRequestV1 {
        service_name: "svc".to_owned(),
        service_version: "v1".to_owned(),
        replica_slot: 1,
        request_path: "/health".to_owned(),
        method: "GET".to_owned(),
        query_string: None,
        headers: Vec::new(),
        body: Vec::new(),
        remote_ip: None,
    });
    let query_request = ToriiProxyRequestKindV4::SignedQueryRouteScan {
        query_bytes: Vec::new(),
        expected_route: ToriiRouteHintV1::from(route),
        response_format: ToriiProxyResponseFormatV1::Norito,
    };
    let (_strict_app, strict_request) =
        incoming_proxy_submit_fixture(0xaa, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);

    assert_eq!(
        super::torii_proxy_response_body_limit(app.as_ref(), &hosted_request),
        1,
        "hosted HTTP proxy responses clamp zero config to a one-byte cap"
    );
    assert_eq!(
        super::torii_proxy_response_body_limit(app.as_ref(), &query_request),
        usize::MAX,
        "non-hosted proxy responses are not capped by the public HTTP response limit"
    );
    assert_eq!(
        super::torii_proxy_response_body_limit(app.as_ref(), &strict_request.request),
        QUEUE_PLAN_SYNCED_CERTIFICATE_MAX_BODY_BYTES_V2,
        "strict durable-admission receipts retain a bounded protocol budget even when the public transaction cap is smaller"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn torii_proxy_retry_policy_only_retries_gateway_class_statuses() {
    assert!(super::should_retry_torii_proxy_status(
        StatusCode::BAD_GATEWAY
    ));
    assert!(super::should_retry_torii_proxy_status(
        StatusCode::SERVICE_UNAVAILABLE
    ));
    assert!(super::should_retry_torii_proxy_status(
        StatusCode::GATEWAY_TIMEOUT
    ));
    assert!(!super::should_retry_torii_proxy_status(
        StatusCode::TOO_MANY_REQUESTS
    ));
    assert!(!super::should_retry_torii_proxy_status(
        StatusCode::INTERNAL_SERVER_ERROR
    ));
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn torii_proxy_hosted_http_request_kind_uses_route_timeout() {
    let hosted_request = ToriiProxyRequestKindV4::HostedHttp(ToriiHostedHttpProxyRequestV1 {
        service_name: "svc".to_owned(),
        service_version: "v1".to_owned(),
        replica_slot: 1,
        request_path: "/health".to_owned(),
        method: "GET".to_owned(),
        query_string: Some("ready=true".to_owned()),
        headers: Vec::new(),
        body: Vec::new(),
        remote_ip: Some("127.0.0.1".to_owned()),
    });

    assert_eq!(
        super::torii_proxy_attempt_timeout(&hosted_request),
        DEFAULT_ROUTE_TIMEOUT
    );
    assert_eq!(
        super::torii_proxy_request_kind_name(&hosted_request),
        "hosted_http"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn queue_plan_synced_runtime_timing_uses_the_complete_remaining_route_budget() {
    assert_eq!(
        super::queue_plan_synced_runtime_timing(Duration::from_millis(1), DEFAULT_ROUTE_TIMEOUT,),
        (
            Duration::from_millis(25),
            DEFAULT_ROUTE_TIMEOUT.saturating_sub(Duration::from_millis(25)),
        ),
    );
    assert_eq!(
        super::queue_plan_synced_runtime_timing(Duration::from_secs(1), DEFAULT_ROUTE_TIMEOUT,),
        (
            Duration::from_millis(250),
            DEFAULT_ROUTE_TIMEOUT.saturating_sub(Duration::from_millis(250)),
        ),
    );
    assert_eq!(
        super::queue_plan_synced_runtime_timing(Duration::from_secs(1), Duration::from_secs(13),),
        (Duration::from_millis(250), Duration::from_millis(12_750)),
    );
    let (poll_interval, carrier_wait) =
        super::queue_plan_synced_runtime_timing(Duration::MAX, DEFAULT_ROUTE_TIMEOUT);
    assert!(
        DEFAULT_ROUTE_TIMEOUT >= carrier_wait.saturating_add(poll_interval),
        "the request deadline must outlive the canonical carrier wait and its final poll"
    );
    assert!(
        carrier_wait > Duration::from_secs(12),
        "a one-round prediction must not cause a premature 503 while finality is progressing"
    );
    assert_eq!(
        super::queue_plan_synced_runtime_timing(Duration::from_secs(1), Duration::ZERO),
        (Duration::from_millis(250), Duration::ZERO),
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn durable_queue_plan_carrier_waits_when_owner_wake_is_deferred() {
    assert_eq!(
        super::durable_queue_plan_wake_disposition(Some(false)),
        super::DurableQueuePlanWakeDisposition::Deferred,
        "a transient missed wake must not turn a durable certificate into an immediate 503"
    );
    assert_eq!(
        super::durable_queue_plan_wake_disposition(Some(true)),
        super::DurableQueuePlanWakeDisposition::Delivered
    );
    assert_eq!(
        super::durable_queue_plan_wake_disposition(None),
        super::DurableQueuePlanWakeDisposition::OwnerMissing
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn queue_plan_synced_max_roster_is_not_serialized_by_proxy_hedging() {
    let roster_len = iroha_data_model::consensus::MAX_LANE_CONSENSUS_VALIDATORS;
    let byzantine_prefix = roster_len.saturating_sub(1) / 3;
    let durability_threshold = roster_len.div_ceil(3);
    let last_required_honest_index = byzantine_prefix
        .saturating_add(durability_threshold)
        .saturating_sub(1);
    let hedge_delay = Duration::from_millis(250);

    assert_eq!(
        roster_len, 128,
        "the adversarial timing case models 128 validators"
    );
    assert_eq!(byzantine_prefix, 42);
    assert_eq!(durability_threshold, 43);
    assert_eq!(last_required_honest_index, 84);
    assert!(
        (0..roster_len).all(|index| {
            super::torii_proxy_candidate_launch_delay(true, hedge_delay, index) == Duration::ZERO
        }),
        "strict QueuePlanSynced authorities must all launch in the first bounded wave"
    );
    assert_eq!(
        super::torii_proxy_candidate_launch_delay(false, hedge_delay, last_required_honest_index,),
        Duration::from_secs(21),
        "ordinary proxy traffic must retain staggered hedging"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn torii_proxy_attempt_timeout_uses_route_budget_for_queries() {
    let route = RoutingDecision::new(LaneId::new(9), DataSpaceId::new(12));
    let query_request = ToriiProxyRequestKindV4::SignedQueryRouteScan {
        query_bytes: Vec::new(),
        expected_route: ToriiRouteHintV1::from(route),
        response_format: ToriiProxyResponseFormatV1::Norito,
    };
    assert_eq!(
        super::torii_proxy_attempt_timeout(&query_request),
        DEFAULT_ROUTE_TIMEOUT
    );
    assert_eq!(
        super::torii_proxy_request_kind_name(&query_request),
        "signed_query_route_scan"
    );

    let keypair =
        checked_torii_test_ed25519_keypair(0xfc, "derive proxy submit timeout fixture key");
    let authority = AccountId::new(keypair.public_key().clone());
    let tx = TransactionBuilder::new(
        signed_query_test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(keypair.private_key());
    let transaction = iroha_data_model::transaction::TransactionEntrypoint::External(tx);
    let expected_plan = ToriiRoutingPlanHintV1::from(RoutingPlan::single(route));
    let submit_request = ToriiProxyRequestKindV4::SubmitTransaction {
        transaction,
        expected_plan,
        admission: ToriiProxyTransactionAdmissionV2::QueuePlanSynced,
        admission_binding: None,
    };
    assert_eq!(
        super::torii_proxy_attempt_timeout(&submit_request),
        DEFAULT_ROUTE_TIMEOUT
    );
    assert_eq!(
        super::torii_proxy_request_kind_name(&submit_request),
        "submit_transaction"
    );
    let process_session = Hash::new(b"torii-proxy-request-id-test-session");
    let ingress_peer = PeerId::from(keypair.public_key().clone());
    assert_ne!(
        super::torii_proxy_request_id_for_session_sequence(
            &process_session,
            &ingress_peer,
            7,
            &submit_request,
        ),
        super::torii_proxy_request_id_for_session_sequence(
            &Hash::new(b"torii-proxy-request-id-restarted-session"),
            &ingress_peer,
            7,
            &submit_request,
        ),
        "a receipt from an earlier process session must not match a request after restart"
    );
    let other_ingress_peer = PeerId::from(
        checked_torii_test_ed25519_keypair(
            0xfb,
            "derive alternate proxy request-id ingress fixture key",
        )
        .public_key()
        .clone(),
    );
    assert_ne!(
        super::torii_proxy_request_id_for_session_sequence(
            &process_session,
            &ingress_peer,
            7,
            &submit_request,
        ),
        super::torii_proxy_request_id_for_session_sequence(
            &process_session,
            &other_ingress_peer,
            7,
            &submit_request,
        ),
        "different ingress peers must not share proxy request identities"
    );

    let first_app = mk_app_state_for_tests();
    let second_app = mk_app_state_for_tests();
    assert_ne!(
        first_app.torii_proxy_session_id, second_app.torii_proxy_session_id,
        "independent AppState instances must use independent OS-random proxy sessions"
    );
    let first_process_request_id =
        super::next_torii_proxy_request_id(&first_app, &ingress_peer, &submit_request)
            .expect("first process sequence must be available");
    let second_process_request_id =
        super::next_torii_proxy_request_id(&second_app, &ingress_peer, &submit_request)
            .expect("second process sequence must be available");
    assert_ne!(
        first_process_request_id, second_process_request_id,
        "restart/process-session separation must hold even at the same sequence"
    );
    first_app
        .torii_proxy_sequence
        .store(u64::MAX, std::sync::atomic::Ordering::Relaxed);
    assert!(
        super::next_torii_proxy_request_id(&first_app, &ingress_peer, &submit_request,).is_err(),
        "request sequence exhaustion must fail closed instead of wrapping"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn torii_proxy_v5_roundtrip_and_forwarding_preserve_transaction_admission_binding() {
    let keypair = checked_torii_test_ed25519_keypair(0xfd, "derive proxy V5 admission fixture key");
    let authority = AccountId::new(keypair.public_key().clone());
    let network_id = signed_query_test_network_id();
    let transaction = iroha_data_model::transaction::TransactionEntrypoint::External(
        TransactionBuilder::new(
            network_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(keypair.private_key()),
    );
    let expected_plan = ToriiRoutingPlanHintV1::from(RoutingPlan::single(RoutingDecision::new(
        LaneId::new(9),
        DataSpaceId::new(12),
    )));
    let forwarding_peer = PeerId::from(keypair.public_key().clone());

    let admission = ToriiProxyTransactionAdmissionV2::QueuePlanSynced;
    let request_id =
        Hash::new(norito::to_bytes(&admission).expect("encode admission request identity"));
    let admission_authorities = vec![forwarding_peer.clone()];
    let context = queue::QueuePlanAdmissionContextV2 {
        version: queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V2,
        authority_height: 7,
        proposal_height: 8,
        predecessor_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"proxy-v5-roundtrip-predecessor",
        ))),
        routing_plan_digest: RoutingPlan::single(RoutingDecision::new(
            LaneId::new(9),
            DataSpaceId::new(12),
        ))
        .digest(),
        route_incarnations: vec![queue::QueuePlanRouteIncarnationV2 {
            leg: iroha_core::queue::RouteLeg::new(
                RoutingDecision::new(LaneId::new(9), DataSpaceId::new(12)),
                iroha_core::queue::RouteLegRole::Coordinator,
            ),
            lane_incarnation: Hash::new(b"proxy-v5-roundtrip-incarnation"),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&admission_authorities),
            validator_set: admission_authorities,
            validator_count: 1,
            durability_threshold: 1,
        }],
    };
    let admission_binding = Some(
        QueuePlanAdmissionBindingV2::new(
            &network_id,
            &transaction,
            &RoutingPlan::single(RoutingDecision::new(LaneId::new(9), DataSpaceId::new(12))),
            context,
            42,
        )
        .expect("build exact V5 admission binding"),
    );
    let request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id,
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV4::SubmitTransaction {
            transaction,
            expected_plan,
            admission,
            admission_binding,
        },
    };
    let encoded = norito::to_bytes(&request).expect("encode V5 proxy admission request");
    let decoded = norito::decode_from_bytes::<ToriiProxyRequestV5>(&encoded)
        .expect("decode V5 proxy admission request");
    assert_eq!(decoded, request);

    let forwarded = super::forwarded_torii_proxy_request(&request, &forwarding_peer);
    assert_eq!(forwarded.schema_version, TORII_PROXY_REQUEST_VERSION_V5);
    assert_eq!(forwarded.request_id, request.request_id);
    assert_eq!(forwarded.request, request.request);
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn incoming_proxy_submit_fixture(
    seed: u8,
    admission: ToriiProxyTransactionAdmissionV2,
) -> (SharedAppState, ToriiProxyRequestV5) {
    let keypair =
        checked_torii_test_ed25519_keypair(seed, "derive incoming proxy Submit fixture key");
    let authority = AccountId::new(keypair.public_key().clone());
    let ingress_peer_id = PeerId::from(keypair.public_key().clone());
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    {
        let local_signer = checked_torii_test_keypair_from_seed_byte(
            seed,
            Algorithm::BlsNormal,
            "derive incoming proxy validator fixture key",
        );
        let local_validator = AccountId::new(local_signer.public_key().clone());
        let local_peer_id = PeerId::from(local_signer.public_key().clone());
        let app =
            Arc::get_mut(&mut app).expect("incoming proxy fixture app must be uniquely owned");
        app.torii_proxy_bridge_signer = local_signer.clone();
        app.local_peer_id = Some(local_peer_id.clone());
        let state = Arc::get_mut(&mut app.state)
            .expect("incoming proxy fixture state must be uniquely owned");
        ensure_runtime_peer_binding_for_test(
            state,
            &local_validator,
            &local_signer,
            "incoming-proxy",
        );
        let mut topology = state.commit_topology.block();
        topology.clear();
        topology.push(local_peer_id.clone());
        topology.commit();
        install_lane_manifest_registry_for_test(
            state,
            &[(LaneId::SINGLE, vec![(local_validator, local_peer_id)])],
        );
    }
    let transaction = TransactionEntrypoint::External(
        TransactionBuilder::new(
            *app.state.network_id_ref(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            format!("incoming-proxy-submit-{seed:02x}"),
        )])
        .sign(keypair.private_key()),
    );
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let routing_plan = RoutingPlan::single(route);
    let request_id =
        queue_plan_synced_proxy_request_id_for_entrypoint(app.as_ref(), transaction.hash());
    let context = app
        .queue
        .plan_admission_context_with_state(app.state.as_ref(), &routing_plan)
        .expect("strict proxy fixture admission context");
    let admission_binding = Some(
        QueuePlanAdmissionBindingV2::new(
            app.state.network_id_ref(),
            &transaction,
            &routing_plan,
            context,
            app.queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("strict proxy fixture admission binding"),
    );
    let request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id,
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: vec![ingress_peer_id],
        request: ToriiProxyRequestKindV4::SubmitTransaction {
            transaction,
            expected_plan: ToriiRoutingPlanHintV1::from(routing_plan),
            admission,
            admission_binding,
        },
    };

    (app, request)
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn single_route_queue_plan_authorities(
    context: &iroha_core::queue::QueuePlanAdmissionContextV2,
) -> Vec<PeerId> {
    assert_eq!(
        context.route_incarnations.len(),
        1,
        "fixture must carry exactly one coordinator route"
    );
    let route = &context.route_incarnations[0];
    assert_eq!(
        route.leg.role,
        iroha_core::queue::RouteLegRole::Coordinator,
        "fixture route must be the coordinator"
    );
    assert!(
        !route.validator_set.is_empty(),
        "fixture coordinator roster must not be empty"
    );
    route.validator_set.clone()
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn set_proxy_fixture_latest_block_height(app: &SharedAppState, height: u64) {
    app.state
        .update_latest_block_header_cache_for_tests(BlockHeader::new(
            NonZeroU64::new(height).expect("proxy fixture height must be non-zero"),
            None,
            None,
            None,
            0,
            0,
        ));
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn accepted_queue_hash_for_proxy_submit(
    app: &SharedAppState,
    request: &ToriiProxyRequestV5,
) -> HashOf<SignedTransaction> {
    let ToriiProxyRequestKindV4::SubmitTransaction { transaction, .. } = &request.request else {
        panic!("proxy Submit fixture must contain a transaction");
    };
    let parameters = app.state.world.view().parameters().clone();
    AcceptedTransaction::accept_entrypoint(
        transaction.clone(),
        app.state.network_id_ref(),
        parameters.sumeragi().max_clock_drift(),
        parameters.transaction(),
        app.state.crypto().as_ref(),
    )
    .expect("proxy Submit fixture must pass canonical transaction admission")
    .hash()
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
async fn exact_queue_plan_synced_acceptance_snapshot(
    app: &SharedAppState,
    request: &ToriiProxyRequestV5,
) -> ToriiProxyHttpResponseV1 {
    let journal_dir =
        tempfile::tempdir().expect("create exact strict acceptance journal directory");
    app.queue
        .install_plan_journal(
            &journal_dir.path().join("queue_plan_journal.norito"),
            1024 * 1024,
            true,
        )
        .expect("install exact strict acceptance queue plan journal");
    let response = super::execute_incoming_torii_proxy_request(app, request.clone(), None).await;
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "the production receiver must emit strict acceptance only after journal sync"
    );
    super::response_to_torii_proxy_snapshot(response, app.transaction_max_content_len.max(1)).await
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn sign_queue_plan_synced_test_receipt(
    binding: &QueuePlanAdmissionBindingV2,
    validator_index: u16,
    signer: &KeyPair,
) -> QueuePlanAdmissionAttestationV2 {
    let signing_bytes = queue_plan_admission_attestation_signing_bytes_v2(
        binding.canonical_hash(),
        validator_index,
    )
    .expect("encode QueuePlanSynced attestation signing bytes");
    let signature = Signature::try_new(signer.private_key(), &signing_bytes)
        .expect("sign QueuePlanSynced test attestation");
    QueuePlanAdmissionAttestationV2 {
        version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2,
        validator_index,
        signature,
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn bind_queue_plan_synced_test_authorities(
    request: &mut ToriiProxyRequestV5,
    signers: &[KeyPair],
) -> Vec<PeerId> {
    let authorities = signers
        .iter()
        .map(|signer| PeerId::new(signer.public_key().clone()))
        .collect::<Vec<_>>();
    let ToriiProxyRequestKindV4::SubmitTransaction {
        transaction,
        expected_plan,
        admission_binding,
        ..
    } = &mut request.request
    else {
        panic!("QueuePlanSynced test request must contain a transaction");
    };
    let binding = admission_binding
        .as_mut()
        .expect("QueuePlanSynced test request must contain an admission binding");
    {
        let coordinator = binding
            .admission_context
            .route_incarnations
            .first_mut()
            .expect("QueuePlanSynced test context must contain a coordinator");
        coordinator.validator_set_hash = HashOf::new(&authorities);
        coordinator.validator_set.clone_from(&authorities);
        coordinator.validator_count =
            u16::try_from(authorities.len()).expect("bounded test authority count");
        coordinator.durability_threshold =
            u16::try_from(authorities.len().div_ceil(3)).expect("bounded test threshold");
    }
    let routing_plan = expected_plan
        .clone()
        .try_into_routing_plan()
        .expect("QueuePlanSynced test routing plan");
    binding.journal_record_digest = queue::queue_plan_journal_record_claim_digest(
        transaction.clone(),
        routing_plan,
        binding.admission_context.clone(),
        binding.enqueue_timestamp_ms,
        Some(binding.global_admission_identity()),
    )
    .expect("rebuild exact QueuePlanSynced journal claim");
    authorities
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn exact_queue_plan_synced_test_receipt(
    request: &ToriiProxyRequestV5,
    signer: &KeyPair,
    _enqueue_timestamp_ms: u64,
) -> QueuePlanAdmissionAttestationV2 {
    let expected = super::queue_plan_synced_acceptance_expectation(request)
        .expect("QueuePlanSynced test expectation must be valid")
        .expect("QueuePlanSynced test request must require strict admission");
    let signer_peer = PeerId::new(signer.public_key().clone());
    let validator_index = expected
        .admission_binding
        .admission_context
        .route_incarnations
        .first()
        .expect("QueuePlanSynced coordinator context")
        .validator_set
        .iter()
        .position(|peer| peer == &signer_peer)
        .and_then(|index| u16::try_from(index).ok())
        .expect("QueuePlanSynced test signer must be in the coordinator roster");
    sign_queue_plan_synced_test_receipt(&expected.admission_binding, validator_index, signer)
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn queue_plan_synced_test_certificate_snapshot(
    request: &ToriiProxyRequestV5,
    mut receipts: Vec<QueuePlanAdmissionAttestationV2>,
) -> ToriiProxyHttpResponseV1 {
    receipts.sort_by_key(|attestation| attestation.validator_index);
    let expected = super::queue_plan_synced_acceptance_expectation(request)
        .expect("QueuePlanSynced test expectation must be valid")
        .expect("QueuePlanSynced test request must require strict admission");
    let entrypoint_hash_literal = expected.entrypoint_hash.to_string();
    let mut headers = vec![
        iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "content-type".to_owned(),
            value: utils::NORITO_MIME_TYPE.as_bytes().to_vec(),
        },
        iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "x-iroha-entrypoint-hash".to_owned(),
            value: entrypoint_hash_literal.as_bytes().to_vec(),
        },
        iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "x-iroha-transaction-hash".to_owned(),
            value: entrypoint_hash_literal.into_bytes(),
        },
    ];
    if let Some(signed_transaction_hash) = expected.signed_transaction_hash {
        headers.push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "x-iroha-signed-transaction-hash".to_owned(),
            value: signed_transaction_hash.to_string().into_bytes(),
        });
    }
    ToriiProxyHttpResponseV1 {
        status_code: StatusCode::ACCEPTED.as_u16(),
        headers,
        body: norito::to_bytes(&QueuePlanAdmissionCertificateV2 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
            binding: expected.admission_binding,
            attestations: receipts,
        })
        .expect("encode QueuePlanSynced test certificate"),
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn queue_plan_admission_publication_targets_every_live_successor_except_self() {
    let (_app, mut request) =
        incoming_proxy_submit_fixture(0xc0, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let signers = (0_u8..4)
        .map(|offset| {
            checked_torii_test_ed25519_keypair(
                0xc1_u8.saturating_add(offset),
                "derive QueuePlan publication target fixture key",
            )
        })
        .collect::<Vec<_>>();
    let authorities = bind_queue_plan_synced_test_authorities(&mut request, &signers);
    let ToriiProxyRequestKindV4::SubmitTransaction {
        admission_binding: Some(binding),
        ..
    } = &request.request
    else {
        panic!("QueuePlan publication target fixture must contain a binding");
    };
    let outsider = PeerId::new(
        checked_torii_test_ed25519_keypair(
            0xc9,
            "derive QueuePlan publication target outsider key",
        )
        .public_key()
        .clone(),
    );
    let online = BTreeSet::from([
        authorities[0].clone(),
        authorities[1].clone(),
        authorities[2].clone(),
        outsider,
    ]);

    assert_eq!(
        super::queue_plan_admission_publication_targets(&authorities[0], &online, binding,)
            .expect("resolve certified publication targets"),
        vec![authorities[1].clone(), authorities[2].clone()],
        "the local authority, one dead authority, and an online outsider must be excluded"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn queue_plan_admission_publication_validates_and_persists_idempotently() {
    let (app, request) =
        incoming_proxy_submit_fixture(0xb0, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let receipt =
        exact_queue_plan_synced_test_receipt(&request, &app.torii_proxy_bridge_signer, 40_001);
    let snapshot = queue_plan_synced_test_certificate_snapshot(&request, vec![receipt]);
    let publication = QueuePlanAdmissionPublicationV1 {
        schema_version: QUEUE_PLAN_ADMISSION_PUBLICATION_VERSION_V1,
        certificate: snapshot.body,
    };
    let expected = super::queue_plan_synced_acceptance_expectation(&request)
        .expect("publication fixture expectation must validate")
        .expect("publication fixture must use strict admission");

    assert_eq!(
        super::validate_queue_plan_admission_publication(&app, &publication)
            .expect("certified publication must validate against local state"),
        expected.admission_binding
    );
    let first = super::ingest_queue_plan_admission_publication(&app, &publication)
        .expect("first certified publication must persist");
    let second = super::ingest_queue_plan_admission_publication(&app, &publication)
        .expect("exact publication replay must remain idempotent");
    let (
        QueuePlanAdmissionPublicationIngestOutcome::Durable {
            certificate_hash: first_hash,
            ..
        },
        QueuePlanAdmissionPublicationIngestOutcome::Durable {
            certificate_hash: second_hash,
            ..
        },
    ) = (first, second)
    else {
        panic!("an absent registry must retain the exact durable certificate");
    };
    assert_eq!(first_hash, second_hash);

    let mut unsupported = publication;
    unsupported.schema_version = unsupported.schema_version.saturating_add(1);
    assert!(
        super::validate_queue_plan_admission_publication(&app, &unsupported)
            .expect_err("unsupported publication schema must fail")
            .contains("schema_version")
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_certificate_requires_canonical_distinct_authority_quorum() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (_app, mut request) =
        incoming_proxy_submit_fixture(0xd0, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let signers = (0_u8..4)
        .map(|offset| {
            checked_torii_test_ed25519_keypair(
                0xd1_u8.saturating_add(offset),
                "derive QueuePlanSynced quorum signer fixture key",
            )
        })
        .collect::<Vec<_>>();
    let authorities = bind_queue_plan_synced_test_authorities(&mut request, &signers);
    let expectation = super::queue_plan_synced_acceptance_expectation(&request)
        .expect("four-authority expectation must be valid")
        .expect("strict request must have an expectation");
    assert_eq!(expectation.durability_threshold, 2);
    let receipts = signers
        .iter()
        .enumerate()
        .map(|(index, signer)| {
            exact_queue_plan_synced_test_receipt(
                &request,
                signer,
                10_000_u64.saturating_add(u64::try_from(index).unwrap_or(u64::MAX)),
            )
        })
        .collect::<Vec<_>>();

    let one_receipt_snapshot =
        queue_plan_synced_test_certificate_snapshot(&request, vec![receipts[0].clone()]);
    assert_eq!(
        super::validate_queue_plan_synced_acceptance(&one_receipt_snapshot, &expectation,)
            .expect("one exact leaf receipt remains individually valid")
            .len(),
        1
    );
    let one_receipt_response = super::execute_torii_proxy_request_across_candidates(
        vec![ToriiProxyCandidate::P2p(authorities[0].clone())],
        route,
        request.clone(),
        Duration::ZERO,
        {
            let one_receipt_snapshot = one_receipt_snapshot.clone();
            move |_candidate, _request| {
                let snapshot = one_receipt_snapshot.clone();
                async move { Ok::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(snapshot) }
            }
        },
        |_request_id| async move {},
    )
    .await;
    assert_eq!(
        one_receipt_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(super::is_queue_plan_outcome_unknown_response(
        &one_receipt_response
    ));

    let snapshots = authorities
        .iter()
        .cloned()
        .zip(receipts.iter().cloned())
        .map(|(authority, receipt)| {
            let mut snapshot = queue_plan_synced_test_certificate_snapshot(&request, vec![receipt]);
            snapshot
                .headers
                .push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                    name: "x-untrusted-upstream-metadata".to_owned(),
                    value: authority.to_string().into_bytes(),
                });
            (authority, snapshot)
        })
        .collect::<BTreeMap<_, _>>();
    let quorum_response = super::execute_torii_proxy_request_across_candidates(
        authorities
            .iter()
            .cloned()
            .map(ToriiProxyCandidate::P2p)
            .collect(),
        route,
        request.clone(),
        Duration::ZERO,
        move |candidate, _request| {
            let snapshot = snapshots
                .get(candidate.peer_id())
                .expect("candidate has a signed quorum fixture")
                .clone();
            async move { Ok::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(snapshot) }
        },
        |_request_id| async move {},
    )
    .await;
    assert_eq!(quorum_response.status(), StatusCode::ACCEPTED);
    assert!(
        !quorum_response
            .headers()
            .contains_key("x-untrusted-upstream-metadata"),
        "aggregated acceptance must rebuild headers instead of inheriting an upstream frame"
    );
    assert!(
        !quorum_response
            .headers()
            .contains_key(axum::http::header::CONTENT_LENGTH),
        "aggregated acceptance must not retain an upstream Content-Length"
    );
    let quorum_body = axum::body::to_bytes(quorum_response.into_body(), usize::MAX)
        .await
        .expect("read aggregated quorum certificate");
    let quorum_certificate: QueuePlanAdmissionCertificateV2 =
        norito::decode_from_bytes(&quorum_body).expect("decode aggregated quorum certificate");
    assert_eq!(quorum_certificate.attestations.len(), 2);
    assert!(
        quorum_certificate
            .attestations
            .windows(2)
            .all(|pair| pair[0].validator_index < pair[1].validator_index)
    );

    let duplicate_snapshot = queue_plan_synced_test_certificate_snapshot(
        &request,
        vec![receipts[0].clone(), receipts[0].clone()],
    );
    assert!(
        super::validate_queue_plan_synced_acceptance(&duplicate_snapshot, &expectation).is_err(),
        "duplicate validator-index attestations must never count twice"
    );

    let mut noncanonical_receipts = receipts[..2].to_vec();
    noncanonical_receipts.sort_by(|left, right| right.validator_index.cmp(&left.validator_index));
    let mut noncanonical_snapshot =
        queue_plan_synced_test_certificate_snapshot(&request, Vec::new());
    noncanonical_snapshot.body = norito::to_bytes(&QueuePlanAdmissionCertificateV2 {
        version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
        binding: expectation.admission_binding.clone(),
        attestations: noncanonical_receipts,
    })
    .expect("encode noncanonical validator-index certificate");
    assert!(
        super::validate_queue_plan_synced_acceptance(&noncanonical_snapshot, &expectation).is_err(),
        "certificate validator-index order must be canonical"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_max_roster_reaches_honest_quorum_past_byzantine_prefix() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (_app, mut request) =
        incoming_proxy_submit_fixture(0xca, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let roster_len = iroha_data_model::consensus::MAX_LANE_CONSENSUS_VALIDATORS;
    let signers = (1_u8..=u8::try_from(roster_len).expect("validator bound fits u8"))
        .map(|seed| {
            checked_torii_test_ed25519_keypair(
                seed,
                "derive max-roster QueuePlanSynced authority fixture key",
            )
        })
        .collect::<Vec<_>>();
    let authorities = bind_queue_plan_synced_test_authorities(&mut request, &signers);
    let byzantine_prefix = roster_len.saturating_sub(1) / 3;
    let durability_threshold = roster_len.div_ceil(3);
    let first_honest_index = byzantine_prefix;
    let last_required_honest_index = first_honest_index
        .saturating_add(durability_threshold)
        .saturating_sub(1);
    let snapshots = Arc::new(
        (first_honest_index..=last_required_honest_index)
            .map(|index| {
                (
                    authorities[index].clone(),
                    queue_plan_synced_test_certificate_snapshot(
                        &request,
                        vec![exact_queue_plan_synced_test_receipt(
                            &request,
                            &signers[index],
                            12_000_u64.saturating_add(
                                u64::try_from(index).expect("validator index fits u64"),
                            ),
                        )],
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    );
    let started = Arc::new(Mutex::new(HashSet::new()));
    let started_for_attempts = started.clone();
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        super::execute_torii_proxy_request_across_candidates(
            authorities
                .iter()
                .cloned()
                .map(ToriiProxyCandidate::P2p)
                .collect(),
            route,
            request,
            Duration::from_millis(250),
            move |candidate, _request| {
                let snapshots = snapshots.clone();
                let started = started_for_attempts.clone();
                async move {
                    let peer_id = candidate.peer_id().clone();
                    started
                        .lock()
                        .expect("max-roster attempt tracker should lock")
                        .insert(peer_id.clone());
                    let Some(snapshot) = snapshots.get(&peer_id).cloned() else {
                        return core::future::pending::<
                            Result<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>,
                        >()
                        .await;
                    };
                    Ok(snapshot)
                }
            },
            |_request_id| async move {},
        ),
    )
    .await
    .expect("42 pending Byzantine authorities must not hedge-delay the honest quorum at index 84");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(
        started
            .lock()
            .expect("max-roster attempt tracker should lock")
            .contains(&authorities[last_required_honest_index]),
        "the final honest authority required for quorum must launch without waiting 21 seconds"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_first_quorum_never_waits_for_pending_equivocation_evidence() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (_app, mut request) =
        incoming_proxy_submit_fixture(0xed, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let signers = (0_u8..4)
        .map(|offset| {
            checked_torii_test_ed25519_keypair(
                0xee_u8.saturating_add(offset),
                "derive pending QueuePlanSynced authority fixture key",
            )
        })
        .collect::<Vec<_>>();
    let authorities = bind_queue_plan_synced_test_authorities(&mut request, &signers);
    let snapshots = [
        (
            authorities[0].clone(),
            queue_plan_synced_test_certificate_snapshot(
                &request,
                vec![exact_queue_plan_synced_test_receipt(
                    &request,
                    &signers[0],
                    11_200,
                )],
            ),
        ),
        (
            authorities[1].clone(),
            queue_plan_synced_test_certificate_snapshot(
                &request,
                vec![exact_queue_plan_synced_test_receipt(
                    &request,
                    &signers[1],
                    11_201,
                )],
            ),
        ),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let aggregation = super::execute_torii_proxy_request_across_candidates(
        authorities[..3]
            .iter()
            .cloned()
            .map(ToriiProxyCandidate::P2p)
            .collect(),
        route,
        request,
        Duration::ZERO,
        move |candidate, _request| {
            let snapshot = snapshots.get(candidate.peer_id()).cloned();
            async move {
                match snapshot {
                    Some(snapshot) => {
                        Ok::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(snapshot)
                    }
                    None => {
                        std::future::pending::<
                            Result<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>,
                        >()
                        .await
                    }
                }
            }
        },
        |_request_id| async move {},
    );
    let response = tokio::time::timeout(Duration::from_secs(1), aggregation)
        .await
        .expect("a pending authority must not extend first-quorum acceptance");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_candidates_use_exact_bound_roster_and_count_local_quorum() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (mut app, mut request) =
        incoming_proxy_submit_fixture(0xe0, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let signers = vec![
        app.torii_proxy_bridge_signer.clone(),
        checked_torii_test_ed25519_keypair(0xe1, "derive exact-roster first remote fixture key"),
        checked_torii_test_ed25519_keypair(0xe2, "derive exact-roster joining remote fixture key"),
        checked_torii_test_ed25519_keypair(0xe3, "derive exact-roster offline remote fixture key"),
    ];
    let authorities = bind_queue_plan_synced_test_authorities(&mut request, &signers);
    let local_peer_id = app
        .local_peer_id
        .clone()
        .expect("strict fixture configures a local peer");
    assert_eq!(local_peer_id, authorities[0]);
    let self_listed_signer = checked_torii_test_ed25519_keypair(
        0xe5,
        "derive self-listed untrusted authority fixture key",
    );
    let self_listed_peer_id = PeerId::from(self_listed_signer.public_key().clone());
    let mut self_listed_request = request.clone();
    let self_listed_authorities =
        bind_queue_plan_synced_test_authorities(&mut self_listed_request, &[self_listed_signer]);
    assert_eq!(self_listed_authorities, vec![self_listed_peer_id.clone()]);
    assert!(
        !super::torii_proxy_authenticated_peer_is_trusted(app.as_ref(), &self_listed_peer_id,),
        "an unknown signer must not cross the HTTP trust boundary by self-listing in an unvalidated request"
    );
    let outsider =
        checked_torii_test_ed25519_keypair(0xe4, "derive current-roster outsider fixture key");
    let outsider_peer_id = PeerId::from(outsider.public_key().clone());
    let (_online_tx, online_rx) = tokio::sync::watch::channel(HashSet::from([
        Peer::new(
            "127.0.0.1:18101".parse().expect("first remote address"),
            signers[1].public_key().clone(),
        ),
        Peer::new(
            "127.0.0.1:18102".parse().expect("joining remote address"),
            signers[2].public_key().clone(),
        ),
        Peer::new(
            "127.0.0.1:18104".parse().expect("outsider address"),
            outsider.public_key().clone(),
        ),
    ]));
    Arc::get_mut(&mut app)
        .expect("exact-roster fixture app must be uniquely owned")
        .online_peers = OnlinePeersProvider::new(online_rx);

    let candidates = super::torii_proxy_candidate_peer_ids_for_request(
        app.as_ref(),
        &local_peer_id,
        route,
        None,
        &[],
        &request,
        true,
    )
    .expect("exact bound authority roster must select candidates");
    assert_eq!(
        candidates.peers,
        vec![
            ToriiProxyCandidate::Local(local_peer_id.clone()),
            ToriiProxyCandidate::P2p(authorities[1].clone()),
            ToriiProxyCandidate::P2p(authorities[2].clone()),
        ],
        "current topology/online outsiders must not substitute for exact bound authorities"
    );
    assert_eq!(candidates.authoritative_total_count, 4);
    assert_eq!(candidates.offline_authoritative_count, 1);
    assert!(
        candidates
            .peers
            .iter()
            .all(|candidate| candidate.peer_id() != &outsider_peer_id)
    );

    let rotated_bound_roster = vec![local_peer_id.clone(), authorities[2].clone()];
    let proposal_height = match &request.request {
        ToriiProxyRequestKindV4::SubmitTransaction {
            admission_binding: Some(binding),
            ..
        } => binding.admission_context.proposal_height,
        _ => panic!("strict exact-roster fixture must carry an admission binding"),
    };
    let rotated = super::queue_plan_synced_proxy_candidate_peer_ids(
        app.as_ref(),
        &local_peer_id,
        route,
        &rotated_bound_roster,
        proposal_height,
        None,
        &[],
        true,
    );
    assert_eq!(
        rotated.peers,
        vec![
            ToriiProxyCandidate::Local(local_peer_id.clone()),
            ToriiProxyCandidate::P2p(authorities[2].clone()),
        ],
        "joining/leaving rotation must use the proposal-bound roster verbatim"
    );

    let local_receipt = exact_queue_plan_synced_test_receipt(&request, &signers[0], 12_000);
    let remote_receipt = exact_queue_plan_synced_test_receipt(&request, &signers[1], 12_001);
    let snapshots = BTreeMap::from([
        (
            authorities[0].clone(),
            queue_plan_synced_test_certificate_snapshot(&request, vec![local_receipt]),
        ),
        (
            authorities[1].clone(),
            queue_plan_synced_test_certificate_snapshot(&request, vec![remote_receipt]),
        ),
    ]);
    let response = super::execute_torii_proxy_request_across_candidates(
        vec![
            ToriiProxyCandidate::Local(authorities[0].clone()),
            ToriiProxyCandidate::P2p(authorities[1].clone()),
        ],
        route,
        request,
        Duration::ZERO,
        move |candidate, _request| {
            let snapshot = snapshots
                .get(candidate.peer_id())
                .expect("local/remote exact quorum snapshot")
                .clone();
            async move { Ok::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(snapshot) }
        },
        |_request_id| async move {},
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "the local exact authority must contribute one distinct f+1 durable attestation"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_real_local_journal_receipt_combines_with_remote_quorum_receipt() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let journal_dir = tempfile::tempdir().expect("create real-local quorum journal directory");
    let journal_path = journal_dir.path().join("queue_plan_journal.norito");
    let (app, mut request) =
        incoming_proxy_submit_fixture(0x90, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    assert_eq!(
        app.queue
            .install_plan_journal(&journal_path, 1024 * 1024, true)
            .expect("install real-local quorum queue plan journal"),
        0
    );

    let authority_signers = [
        app.torii_proxy_bridge_signer.clone(),
        checked_torii_test_ed25519_keypair(
            0x91,
            "derive real-local quorum first remote fixture key",
        ),
        checked_torii_test_ed25519_keypair(
            0x92,
            "derive real-local quorum second remote fixture key",
        ),
        checked_torii_test_ed25519_keypair(
            0x93,
            "derive real-local quorum third remote fixture key",
        ),
    ];
    let authorities = authority_signers
        .iter()
        .map(|signer| PeerId::new(signer.public_key().clone()))
        .collect::<Vec<_>>();
    {
        let mut topology = app.state.commit_topology.block();
        topology.clear();
        for authority in &authorities {
            topology.push(authority.clone());
        }
        topology.commit();
    }

    let routing_plan = RoutingPlan::single(route);
    let admission_context = app
        .queue
        .plan_admission_context_with_state(app.state.as_ref(), &routing_plan)
        .expect("capture real-local quorum admission context");
    let admission_authorities = single_route_queue_plan_authorities(&admission_context);
    assert_eq!(
        admission_authorities, authorities,
        "the durable receiver and aggregator must bind the same n=4 authority roster"
    );
    let ToriiProxyRequestKindV4::SubmitTransaction {
        transaction,
        admission_binding,
        ..
    } = &mut request.request
    else {
        panic!("real-local quorum fixture must contain a transaction");
    };
    *admission_binding = Some(
        QueuePlanAdmissionBindingV2::new(
            app.state.network_id_ref(),
            transaction,
            &routing_plan,
            admission_context,
            app.queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("bind real-local quorum request"),
    );

    let local_peer_id = authorities[0].clone();
    let remote_peer_id = authorities[1].clone();
    assert_eq!(
        app.local_peer_id.as_ref(),
        Some(&local_peer_id),
        "the real local receipt signer must be the configured local authority"
    );
    request.hop_count = 1;
    request.visited_peer_ids = vec![local_peer_id.clone()];
    let expected = super::queue_plan_synced_acceptance_expectation(&request)
        .expect("real-local quorum expectation must be valid")
        .expect("real-local quorum request must require strict admission");
    assert_eq!(
        expected.durability_threshold, 2,
        "four authorities must require f+1=2 durable receipts"
    );

    let remote_receipt =
        exact_queue_plan_synced_test_receipt(&request, &authority_signers[1], 12_100);
    let remote_snapshot =
        queue_plan_synced_test_certificate_snapshot(&request, vec![remote_receipt]);
    let app_for_local = Arc::clone(&app);
    let remote_peer_for_dispatch = remote_peer_id.clone();
    let response = super::execute_torii_proxy_request_across_candidates(
        vec![
            ToriiProxyCandidate::Local(local_peer_id.clone()),
            ToriiProxyCandidate::P2p(remote_peer_id.clone()),
        ],
        route,
        request,
        Duration::ZERO,
        move |candidate, candidate_request| {
            let app = Arc::clone(&app_for_local);
            let remote_snapshot = remote_snapshot.clone();
            let remote_peer_id = remote_peer_for_dispatch.clone();
            async move {
                match candidate {
                    ToriiProxyCandidate::Local(peer_id) => {
                        super::execute_torii_proxy_request_locally(&app, peer_id, candidate_request)
                            .await
                    }
                    ToriiProxyCandidate::P2p(peer_id) if peer_id == remote_peer_id => {
                        Ok(remote_snapshot)
                    }
                    ToriiProxyCandidate::P2p(_) | ToriiProxyCandidate::HttpBridge { .. } => {
                        Err(ToriiProxyAttemptError::before_dispatch(
                            "unexpected real-local quorum candidate",
                        ))
                    }
                }
            }
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        app.queue.active_len(),
        1,
        "the real local authority must retain exactly one admitted transaction"
    );
    assert!(
        std::fs::metadata(&journal_path)
            .expect("real-local quorum journal metadata")
            .len()
            > 0,
        "f+1 acceptance must include a locally fsynced V4 journal record"
    );

    let snapshot = super::response_to_torii_proxy_snapshot(
        response,
        QUEUE_PLAN_SYNCED_CERTIFICATE_MAX_BODY_BYTES_V2,
    )
    .await;
    let validated_receipts = super::validate_queue_plan_synced_acceptance(&snapshot, &expected)
        .expect("production validation must accept the aggregated exact certificate");
    assert_eq!(validated_receipts.len(), 2);
    let certificate = super::decode_queue_plan_synced_certificate(&snapshot.body)
        .expect("decode canonical real-local quorum certificate");
    assert_eq!(certificate.attestations.len(), 2);
    assert!(
        certificate
            .attestations
            .windows(2)
            .all(|pair| pair[0].validator_index < pair[1].validator_index),
        "the aggregated certificate must retain canonical validator-index order"
    );
    let certificate_signers = certificate
        .attestations
        .iter()
        .map(|attestation| authorities[usize::from(attestation.validator_index)].clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        certificate_signers,
        BTreeSet::from([local_peer_id, remote_peer_id]),
        "the f+1 certificate must contain the real local and synthetic remote authorities"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_admission_context_rejects_height_plan_leg_incarnation_roster_and_threshold_drift()
 {
    let (app, request) =
        incoming_proxy_submit_fixture(0xd6, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let mutate_context =
        |mut request: ToriiProxyRequestV5,
         mutation: fn(&mut queue::QueuePlanAdmissionContextV2)| {
            let ToriiProxyRequestKindV4::SubmitTransaction {
                admission_binding, ..
            } = &mut request.request
            else {
                panic!("strict context fixture must contain a transaction");
            };
            mutation(
                &mut admission_binding
                    .as_mut()
                    .expect("strict context fixture must contain a binding")
                    .admission_context,
            );
            request
        };
    let mut drift_cases = Vec::new();
    drift_cases.push((
        "version",
        mutate_context(request.clone(), |context| {
            context.version = context.version.saturating_add(1);
        }),
    ));
    drift_cases.push((
        "authority_height",
        mutate_context(request.clone(), |context| {
            context.authority_height = context.authority_height.saturating_add(1);
        }),
    ));
    drift_cases.push((
        "proposal_height",
        mutate_context(request.clone(), |context| {
            context.proposal_height = context.proposal_height.saturating_add(1);
        }),
    ));
    drift_cases.push((
        "predecessor_hash",
        mutate_context(request.clone(), |context| {
            context.predecessor_block_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
                b"forged-queue-plan-predecessor",
            )));
        }),
    ));
    drift_cases.push((
        "plan_digest",
        mutate_context(request.clone(), |context| {
            context.routing_plan_digest = Hash::new(b"stale-admission-plan-digest");
        }),
    ));
    drift_cases.push((
        "leg_role",
        mutate_context(request.clone(), |context| {
            context.route_incarnations[0].leg.role = iroha_core::queue::RouteLegRole::Participant;
        }),
    ));
    drift_cases.push((
        "aba_incarnation",
        mutate_context(request.clone(), |context| {
            context.route_incarnations[0].lane_incarnation =
                Hash::new(b"retired-a-incarnation-replayed-after-b-a");
        }),
    ));
    drift_cases.push((
        "roster_hash",
        mutate_context(request.clone(), |context| {
            context.route_incarnations[0].validator_set_hash = HashOf::new(&Vec::<PeerId>::new());
        }),
    ));
    drift_cases.push((
        "roster_count",
        mutate_context(request.clone(), |context| {
            context.route_incarnations[0].validator_count = context.route_incarnations[0]
                .validator_count
                .saturating_add(1);
        }),
    ));
    drift_cases.push((
        "durability_threshold",
        mutate_context(request.clone(), |context| {
            context.route_incarnations[0].durability_threshold = context.route_incarnations[0]
                .durability_threshold
                .saturating_add(1);
        }),
    ));
    let mut duplicate_roster = request.clone();
    let ToriiProxyRequestKindV4::SubmitTransaction {
        admission_binding, ..
    } = &mut duplicate_roster.request
    else {
        unreachable!("strict fixture shape checked above");
    };
    let coordinator = admission_binding
        .as_mut()
        .expect("strict fixture admission binding")
        .admission_context
        .route_incarnations
        .first_mut()
        .expect("strict fixture coordinator context");
    coordinator
        .validator_set
        .push(coordinator.validator_set[0].clone());
    drift_cases.push(("duplicate_roster", duplicate_roster));

    for (label, drifted_request) in drift_cases {
        assert!(
            super::queue_plan_synced_acceptance_expectation(&drifted_request).is_err()
                || label == "aba_incarnation",
            "{label} must fail structural expectation validation unless it requires live-state incarnation comparison"
        );
        let response =
            super::execute_incoming_torii_proxy_request(&app, drifted_request, None).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{label} must fail before durable queue admission"
        );
        assert_eq!(app.queue.active_len(), 0, "{label} changed queue ownership");
    }

    let mut nondeterministic_request_id = request.clone();
    let forged_request_id = Hash::new(b"nondeterministic-cross-ingress-request-id");
    nondeterministic_request_id.request_id = forged_request_id;
    let ToriiProxyRequestKindV4::SubmitTransaction {
        transaction,
        expected_plan,
        admission_binding: Some(binding),
        ..
    } = &mut nondeterministic_request_id.request
    else {
        unreachable!("strict fixture shape checked above");
    };
    binding.request_id = forged_request_id;
    let routing_plan = expected_plan
        .clone()
        .try_into_routing_plan()
        .expect("strict fixture routing plan");
    binding.journal_record_digest = queue::queue_plan_journal_record_claim_digest(
        transaction.clone(),
        routing_plan,
        binding.admission_context.clone(),
        binding.enqueue_timestamp_ms,
        Some(binding.global_admission_identity()),
    )
    .expect("rebind forged request-id journal claim");
    super::queue_plan_synced_acceptance_expectation(&nondeterministic_request_id)
        .expect("forged request id remains structurally self-consistent")
        .expect("forged request still claims synchronized admission");
    let response =
        super::execute_incoming_torii_proxy_request(&app, nondeterministic_request_id, None).await;
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "authorities must reject a self-consistent but nondeterministic request identity"
    );
    assert_eq!(app.queue.active_len(), 0);

    let mut missing = request.clone();
    let ToriiProxyRequestKindV4::SubmitTransaction {
        admission_binding, ..
    } = &mut missing.request
    else {
        unreachable!("strict fixture shape checked above");
    };
    *admission_binding = None;
    assert!(
        super::queue_plan_synced_acceptance_expectation(&missing).is_err(),
        "missing binding must fail ingress expectation validation"
    );
    let response = super::execute_incoming_torii_proxy_request(&app, missing, None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(app.queue.active_len(), 0);
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_response_bounds_reject_headers_body_encoding_and_decode_amplification() {
    let (app, request) =
        incoming_proxy_submit_fixture(0xd7, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let receipt =
        exact_queue_plan_synced_test_receipt(&request, &app.torii_proxy_bridge_signer, 20_001);
    let exact_snapshot = queue_plan_synced_test_certificate_snapshot(&request, vec![receipt]);
    let expectation = super::queue_plan_synced_acceptance_expectation(&request)
        .expect("bounded-response expectation must be valid")
        .expect("bounded-response fixture must be strict");
    super::validate_queue_plan_synced_acceptance(&exact_snapshot, &expectation)
        .expect("exact bounded certificate must validate");

    let mut oversized_body = exact_snapshot.clone();
    oversized_body.body =
        vec![0_u8; QUEUE_PLAN_SYNCED_CERTIFICATE_MAX_BODY_BYTES_V2.saturating_add(1)];
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&oversized_body)
            .expect_err("body limit + 1 must fail")
            .contains("body")
    );

    let mut too_many_headers = exact_snapshot.clone();
    too_many_headers.headers = (0..=QUEUE_PLAN_SYNCED_MAX_HEADERS_V2)
        .map(|index| iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: format!("x-bound-{index}"),
            value: Vec::new(),
        })
        .collect();
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&too_many_headers)
            .expect_err("header count limit + 1 must fail")
            .contains("too many headers")
    );

    let mut oversized_name = exact_snapshot.clone();
    oversized_name.headers = vec![iroha_core::torii_proxy::ToriiProxyHeaderV1 {
        name: "n".repeat(QUEUE_PLAN_SYNCED_MAX_HEADER_NAME_BYTES_V2.saturating_add(1)),
        value: Vec::new(),
    }];
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&oversized_name)
            .expect_err("header-name limit + 1 must fail")
            .contains("field limit")
    );

    let mut oversized_value = exact_snapshot.clone();
    oversized_value.headers = vec![iroha_core::torii_proxy::ToriiProxyHeaderV1 {
        name: "x-bound".to_owned(),
        value: vec![b'v'; QUEUE_PLAN_SYNCED_MAX_HEADER_VALUE_BYTES_V2.saturating_add(1)],
    }];
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&oversized_value)
            .expect_err("header-value limit + 1 must fail")
            .contains("field limit")
    );

    let mut oversized_aggregate = exact_snapshot.clone();
    oversized_aggregate.headers = (0..9)
        .map(|index| iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: format!("x-aggregate-{index}"),
            value: vec![b'a'; 500],
        })
        .collect();
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&oversized_aggregate)
            .expect_err("aggregate header limit + 1 must fail")
            .contains("aggregate limit")
    );

    let mut non_identity_encoding = exact_snapshot.clone();
    non_identity_encoding
        .headers
        .push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "content-encoding".to_owned(),
            value: b"gzip".to_vec(),
        });
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&non_identity_encoding)
            .expect_err("compressed strict P2P response must fail")
            .contains("non-identity")
    );
    let mut duplicate_encoding = exact_snapshot.clone();
    duplicate_encoding.headers.extend([
        iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "content-encoding".to_owned(),
            value: b"identity".to_vec(),
        },
        iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "Content-Encoding".to_owned(),
            value: b"identity".to_vec(),
        },
    ]);
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&duplicate_encoding)
            .expect_err("duplicate strict P2P encodings must fail")
            .contains("non-identity")
    );
    let mut exact_content_length = exact_snapshot.clone();
    exact_content_length
        .headers
        .push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "content-length".to_owned(),
            value: exact_content_length.body.len().to_string().into_bytes(),
        });
    super::validate_queue_plan_synced_snapshot_bounds(&exact_content_length)
        .expect("one exact Content-Length is permitted");
    let mut mismatched_content_length = exact_content_length.clone();
    mismatched_content_length
        .headers
        .last_mut()
        .expect("Content-Length fixture")
        .value = exact_content_length
        .body
        .len()
        .saturating_add(1)
        .to_string()
        .into_bytes();
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&mismatched_content_length)
            .expect_err("mismatched strict P2P Content-Length must fail")
            .contains("does not match")
    );
    let mut duplicate_content_length = exact_content_length;
    duplicate_content_length
        .headers
        .push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "Content-Length".to_owned(),
            value: duplicate_content_length.body.len().to_string().into_bytes(),
        });
    assert!(
        super::validate_queue_plan_synced_snapshot_bounds(&duplicate_content_length)
            .expect_err("duplicate strict P2P Content-Length must fail")
            .contains("duplicate")
    );

    let mut empty_body = exact_snapshot.clone();
    empty_body.body.clear();
    assert!(super::decode_queue_plan_synced_certificate(&empty_body.body).is_err());
    let mut malformed_bounded_body = exact_snapshot.clone();
    malformed_bounded_body.body = vec![0xff; 4096];
    assert!(
        super::decode_queue_plan_synced_certificate(&malformed_bounded_body.body).is_err(),
        "bounded decoder must reject allocation/vector amplification input"
    );
    let mut noncanonical_body = exact_snapshot.clone();
    noncanonical_body.body.push(0);
    assert!(
        super::decode_queue_plan_synced_certificate(&noncanonical_body.body).is_err(),
        "trailing bytes must not produce a second canonical certificate encoding"
    );

    let upstream = axum::Router::new().route(
        "/encoded",
        axum::routing::get(|| async {
            let mut response = Response::new(Body::from("four"));
            response.headers_mut().insert(
                axum::http::header::CONTENT_ENCODING,
                HeaderValue::from_static("x-queue-plan-test"),
            );
            response
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind encoded-response listener");
    let addr = listener.local_addr().expect("encoded-response address");
    let upstream_task = tokio::spawn(async move {
        axum::serve(listener, upstream.into_make_service())
            .await
            .expect("serve encoded response");
    });
    let encoded_response = reqwest::Client::new()
        .get(format!("http://{addr}/encoded"))
        .header(axum::http::header::ACCEPT_ENCODING, "identity")
        .send()
        .await
        .expect("fetch encoded bridge response");
    let encoding_error =
        super::reqwest_response_to_torii_proxy_snapshot(encoded_response, 16, true)
            .await
            .expect_err("non-identity encoding must fail");
    upstream_task.abort();
    assert!(encoding_error.contains("non-identity content encoding"));

    let request_id = Hash::new(b"strict-p2p-response-bound");
    let responder = PeerId::new(
        checked_torii_test_ed25519_keypair(0xd8, "derive strict P2P bound responder fixture key")
            .public_key()
            .clone(),
    );
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.torii_proxy_pending.lock().await.insert(
        (request_id, responder.clone()),
        PendingToriiProxyRequest {
            sender: tx,
            max_body_bytes: 4,
            strict_queue_plan_synced: true,
        },
    );
    super::process_incoming_torii_proxy_response(
        &app,
        responder,
        ToriiProxyResponseV1 {
            schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
            request_id,
            response: ToriiProxyHttpResponseV1 {
                status_code: StatusCode::ACCEPTED.as_u16(),
                headers: Vec::new(),
                body: b"five!".to_vec(),
            },
        },
    )
    .await;
    assert!(
        rx.await.is_err(),
        "P2P must drop an oversized strict response before handing bytes to the decoder"
    );
}

#[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
#[tokio::test]
async fn incoming_torii_proxy_rejects_malformed_v4_hop_chain_before_dispatch() {
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&ALICE_ID));
    let sender = PeerId::new(
        checked_torii_test_ed25519_keypair(
            0xd9,
            "derive malformed-hop authenticated sender fixture key",
        )
        .public_key()
        .clone(),
    );
    let local_peer = PeerId::new(
        checked_torii_test_ed25519_keypair(0xda, "derive malformed-hop local peer fixture key")
            .public_key()
            .clone(),
    );
    Arc::get_mut(&mut app)
        .expect("malformed-hop fixture app must be uniquely owned")
        .local_peer_id = Some(local_peer.clone());
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let base_request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id: Hash::new(b"malformed-v4-hop-chain"),
        hop_count: 1,
        max_hops: TORII_PROXY_DEFAULT_MAX_HOPS,
        visited_peer_ids: vec![sender.clone()],
        request: ToriiProxyRequestKindV4::Read(super::torii_read_request(
            ToriiReadEndpointV1::AccountGet,
            route,
            vec![ALICE_ID.to_string()],
            None,
            Vec::new(),
        )),
    };

    let mut malformed = Vec::new();
    let mut zero_hops = base_request.clone();
    zero_hops.hop_count = 0;
    malformed.push(("zero_hops", zero_hops, Some(sender.clone())));
    let mut oversized_budget = base_request.clone();
    oversized_budget.max_hops = TORII_PROXY_DEFAULT_MAX_HOPS.saturating_add(1);
    malformed.push(("oversized_budget", oversized_budget, Some(sender.clone())));
    let mut length_mismatch = base_request.clone();
    length_mismatch.hop_count = 2;
    malformed.push(("length_mismatch", length_mismatch, Some(sender.clone())));
    let mut duplicate_history = base_request.clone();
    duplicate_history.hop_count = 2;
    duplicate_history.visited_peer_ids.push(sender.clone());
    malformed.push(("duplicate_history", duplicate_history, Some(sender.clone())));
    malformed.push((
        "sender_mismatch",
        base_request.clone(),
        Some(local_peer.clone()),
    ));
    let mut receiver_revisit = base_request.clone();
    receiver_revisit.hop_count = 2;
    receiver_revisit.visited_peer_ids = vec![local_peer.clone(), sender.clone()];
    malformed.push((
        "receiver_revisit_before_local_dispatch",
        receiver_revisit,
        Some(sender.clone()),
    ));

    for (label, request, authenticated_sender) in malformed {
        let response =
            super::execute_incoming_torii_proxy_request(&app, request, authenticated_sender).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{label} must fail before read dispatch"
        );
        assert_eq!(app.queue.active_len(), 0);
    }

    let mut revisit = base_request;
    revisit.visited_peer_ids = vec![local_peer];
    let response =
        super::forward_incoming_torii_proxy_request(&app, &sender, route, &revisit).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(app.queue.active_len(), 0);
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn queue_plan_synced_certificate_binds_exact_durable_journal_claim() {
    let (app, request) =
        incoming_proxy_submit_fixture(0xdb, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let expectation = super::queue_plan_synced_acceptance_expectation(&request)
        .expect("exact-claim expectation must be valid")
        .expect("exact-claim fixture must be strict");
    let expected_signed_hash = expectation
        .signed_transaction_hash
        .as_ref()
        .expect("external exact-claim fixture must have a signed transaction hash");
    assert_eq!(
        Some(expected_signed_hash),
        expectation
            .admission_binding
            .signed_transaction_hash
            .as_ref(),
        "ingress must expect the same inner signed-wire hash carried by the durable binding"
    );
    assert_ne!(
        expected_signed_hash.as_ref(),
        expectation.entrypoint_hash.as_ref(),
        "the inner signed-wire hash must remain distinct from the external entrypoint hash"
    );
    let exact_receipt =
        exact_queue_plan_synced_test_receipt(&request, &app.torii_proxy_bridge_signer, 30_001);
    let exact_snapshot =
        queue_plan_synced_test_certificate_snapshot(&request, vec![exact_receipt.clone()]);
    super::validate_queue_plan_synced_acceptance(&exact_snapshot, &expectation)
        .expect("exact durable journal claim must validate");

    let outsider = checked_torii_test_ed25519_keypair(
        0xdc,
        "derive exact durable journal claim outsider fixture key",
    );
    let snapshot_for = |mutation: fn(&mut QueuePlanAdmissionCertificateV2)| {
        let mut snapshot = exact_snapshot.clone();
        let mut certificate = super::decode_queue_plan_synced_certificate(&snapshot.body)
            .expect("decode exact certificate for mutation");
        mutation(&mut certificate);
        snapshot.body = norito::to_bytes(&certificate).expect("encode mutated V2 certificate");
        snapshot
    };
    let mut mutations = vec![
        (
            "certificate_version",
            snapshot_for(|certificate| {
                certificate.version = certificate.version.saturating_add(1);
            }),
        ),
        (
            "attestation_version",
            snapshot_for(|certificate| {
                certificate.attestations[0].version =
                    certificate.attestations[0].version.saturating_add(1);
            }),
        ),
        (
            "validator_index",
            snapshot_for(|certificate| {
                certificate.attestations[0].validator_index = u16::MAX;
            }),
        ),
        (
            "request_id",
            snapshot_for(|certificate| {
                certificate.binding.request_id = Hash::new(b"replayed-exact-durable-claim-request");
            }),
        ),
        (
            "entrypoint_hash",
            snapshot_for(|certificate| {
                certificate.binding.entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new(
                    b"forged-exact-durable-entrypoint-hash",
                ));
            }),
        ),
        (
            "signed_hash",
            snapshot_for(|certificate| {
                certificate.binding.signed_transaction_hash = None;
            }),
        ),
        (
            "plan_digest",
            snapshot_for(|certificate| {
                certificate.binding.routing_plan_digest =
                    Hash::new(b"forged-exact-durable-plan-digest");
            }),
        ),
        (
            "claim_version",
            snapshot_for(|certificate| {
                certificate.binding.durable_admission_version = certificate
                    .binding
                    .durable_admission_version
                    .saturating_add(1);
            }),
        ),
        (
            "context",
            snapshot_for(|certificate| {
                certificate.binding.admission_context.route_incarnations[0].lane_incarnation =
                    Hash::new(b"stale-exact-durable-context");
            }),
        ),
        (
            "timestamp",
            snapshot_for(|certificate| {
                certificate.binding.enqueue_timestamp_ms =
                    certificate.binding.enqueue_timestamp_ms.saturating_add(1);
            }),
        ),
        (
            "journal_digest",
            snapshot_for(|certificate| {
                certificate.binding.journal_record_digest =
                    Hash::new(b"forged-exact-durable-journal-record");
            }),
        ),
    ];
    let forged_attestation = sign_queue_plan_synced_test_receipt(
        &expectation.admission_binding,
        exact_receipt.validator_index,
        &outsider,
    );
    mutations.push((
        "signature",
        queue_plan_synced_test_certificate_snapshot(&request, vec![forged_attestation]),
    ));

    for (label, snapshot) in mutations {
        assert!(
            super::validate_queue_plan_synced_acceptance(&snapshot, &expectation).is_err(),
            "{label} drift must invalidate the exact durable journal claim"
        );
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn queue_plan_synced_reconciliation_hash_matches_accepted_queue_identity() {
    let (app, request) =
        incoming_proxy_submit_fixture(0xe0, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let accepted_queue_hash = accepted_queue_hash_for_proxy_submit(&app, &request);
    let reconciliation_hash = super::queue_plan_synced_entrypoint_hash(&request.request)
        .expect("QueuePlanSynced Submit must expose its queue identity");
    assert_eq!(
        Hash::from(reconciliation_hash.clone()),
        Hash::from(accepted_queue_hash),
    );

    let ToriiProxyRequestKindV4::SubmitTransaction { transaction, .. } = &request.request else {
        unreachable!("fixture shape checked above");
    };
    let entrypoint_hash = transaction.hash();
    assert_eq!(
        Hash::from(reconciliation_hash),
        Hash::from(entrypoint_hash),
        "durable reconciliation must retain its exact typed entrypoint identity"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_registry_timeout_never_returns_accepted() {
    let (app, request) =
        incoming_proxy_submit_fixture(0xde, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let expected = super::queue_plan_synced_acceptance_expectation(&request)
        .expect("timeout fixture must be valid")
        .expect("timeout fixture must require synchronized admission");

    let outcome = super::wait_for_exact_queue_plan_admission_registry(
        app.state.as_ref(),
        &expected.admission_binding,
        Duration::from_millis(1),
        Duration::ZERO,
    )
    .await;
    assert_eq!(
        outcome,
        super::QueuePlanAdmissionRegistryWaitOutcome::TimedOut
    );
    let response = super::queue_plan_outcome_unknown_response(
        expected.entrypoint_hash,
        "test timeout before canonical WSV publication",
    );
    assert_ne!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(super::is_queue_plan_outcome_unknown_response(&response));
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn incoming_submit_queue_plan_synced_without_journal_is_stably_unavailable() {
    let (app, request) =
        incoming_proxy_submit_fixture(0xe2, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);

    let response = super::execute_incoming_torii_proxy_request(&app, request, None).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        app.queue.active_len(),
        0,
        "failed QueuePlanSynced admission must roll back ordinary queue ownership"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read QueuePlanSynced rejection body");
    let envelope: ErrorEnvelope =
        norito::decode_from_bytes(&body).expect("decode QueuePlanSynced rejection envelope");
    assert_eq!(envelope.code(), "queue_plan_journal_unavailable");
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn incoming_submit_queue_plan_synced_succeeds_with_installed_journal() {
    let journal_dir = tempfile::tempdir().expect("create proxy Submit journal directory");
    let journal_path = journal_dir.path().join("queue_plan_journal.norito");
    let (app, request) =
        incoming_proxy_submit_fixture(0xe3, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    assert_eq!(
        app.queue
            .install_plan_journal(&journal_path, 1024 * 1024, true)
            .expect("install queue plan journal"),
        0
    );

    let local_peer_id = app
        .local_peer_id
        .clone()
        .expect("strict local fixture configures a local peer");
    let mut local_request = request.clone();
    local_request.hop_count = 1;
    local_request.visited_peer_ids = vec![local_peer_id.clone()];
    let snapshot = super::execute_torii_proxy_request_locally(&app, local_peer_id, local_request)
        .await
        .expect("local strict proxy execution must produce a bounded response");

    assert_eq!(snapshot.status_code, StatusCode::ACCEPTED.as_u16());
    assert_eq!(app.queue.active_len(), 1);
    assert!(
        std::fs::metadata(&journal_path)
            .expect("queue plan journal metadata")
            .len()
            > 0,
        "QueuePlanSynced acknowledgement must leave a durable plan record"
    );
    let expected = super::queue_plan_synced_acceptance_expectation(&request)
        .expect("strict incoming request expectation must be valid")
        .expect("strict incoming request must have an acceptance expectation");
    super::validate_queue_plan_synced_acceptance(&snapshot, &expected)
        .expect("production strict response must attest the exact durable request and peer");
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn incoming_queue_plan_synced_exact_retry_survives_height_and_roster_advance() {
    let journal_dir =
        tempfile::tempdir().expect("create historical strict-retry journal directory");
    let journal_path = journal_dir.path().join("queue_plan_journal.norito");
    let (app, request) =
        incoming_proxy_submit_fixture(0xd5, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    app.queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install historical strict-retry queue plan journal");

    let first = super::execute_incoming_torii_proxy_request(&app, request.clone(), None).await;
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    assert_eq!(app.queue.active_len(), 1);
    let durable_len = std::fs::metadata(&journal_path)
        .expect("strict-retry journal metadata after first admission")
        .len();

    set_proxy_fixture_latest_block_height(&app, 1);
    let rotated_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(
            0xd6,
            "derive rotated strict-retry authority fixture key",
        )
        .public_key()
        .clone(),
    );
    {
        let mut topology = app.state.commit_topology.block();
        topology.clear();
        topology.push(rotated_peer_id);
        topology.commit();
    }
    let ToriiProxyRequestKindV4::SubmitTransaction {
        expected_plan,
        admission_binding: Some(historical_binding),
        ..
    } = &request.request
    else {
        panic!("strict-retry fixture must contain an exact durable context");
    };
    let routing_plan = expected_plan
        .clone()
        .try_into_routing_plan()
        .expect("strict-retry fixture routing plan");
    let current_context = app
        .queue
        .plan_admission_context_with_state(app.state.as_ref(), &routing_plan)
        .expect("capture advanced strict-retry context");
    assert_ne!(
        &current_context, &historical_binding.admission_context,
        "the retry must exercise a historical height and authority roster"
    );
    let current_authorities = single_route_queue_plan_authorities(&current_context);
    assert!(
        app.local_peer_id
            .as_ref()
            .is_some_and(|local| !current_authorities.contains(local)),
        "the old durable authority must no longer belong to the current roster"
    );

    let retry = super::execute_incoming_torii_proxy_request(&app, request, None).await;
    assert_eq!(
        retry.status(),
        StatusCode::ACCEPTED,
        "an exact still-owned durable claim must remain retryable after ordinary chain and roster advancement"
    );
    assert_eq!(app.queue.active_len(), 1);
    assert_eq!(
        std::fs::metadata(&journal_path)
            .expect("strict-retry journal metadata after retry")
            .len(),
        durable_len,
        "an exact retry must return the original durable claim without appending a replacement"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn incoming_queue_plan_synced_historical_context_without_owned_claim_fails_closed() {
    let journal_dir =
        tempfile::tempdir().expect("create stale unowned admission journal directory");
    let journal_path = journal_dir.path().join("queue_plan_journal.norito");
    let (app, request) =
        incoming_proxy_submit_fixture(0xd7, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    app.queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install stale unowned admission queue plan journal");
    let journal_len_before_rejection = std::fs::metadata(&journal_path)
        .expect("stale unowned admission journal baseline metadata")
        .len();
    set_proxy_fixture_latest_block_height(&app, 1);

    let response = super::execute_incoming_torii_proxy_request(&app, request, None).await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        app.queue.active_len(),
        0,
        "historical context must not create queue ownership without the exact durable claim"
    );
    assert_eq!(
        std::fs::metadata(&journal_path)
            .expect("stale unowned admission journal metadata")
            .len(),
        journal_len_before_rejection,
        "rejected historical admission must not append a journal record"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read historical unowned admission rejection");
    let envelope: ErrorEnvelope =
        norito::decode_from_bytes(&body).expect("decode historical unowned admission rejection");
    assert_eq!(envelope.code(), "queue_plan_admission_context_mismatch");
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn incoming_submit_queue_plan_synced_fails_closed_when_local_peer_and_signer_diverge() {
    let journal_dir = tempfile::tempdir().expect("create signer-mismatch journal directory");
    let journal_path = journal_dir.path().join("queue_plan_journal.norito");
    let (mut app, request) =
        incoming_proxy_submit_fixture(0xe4, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let mismatched_peer = PeerId::from(
        checked_torii_test_ed25519_keypair(
            0xe5,
            "derive mismatched strict admission peer fixture key",
        )
        .public_key()
        .clone(),
    );
    Arc::get_mut(&mut app)
        .expect("strict admission fixture app must be uniquely owned")
        .local_peer_id = Some(mismatched_peer);
    app.queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install signer-mismatch queue plan journal");
    let journal_len_before_rejection = std::fs::metadata(&journal_path)
        .expect("signer-mismatch queue plan journal baseline metadata")
        .len();

    let response = super::execute_incoming_torii_proxy_request(&app, request, None).await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        app.queue.active_len(),
        0,
        "the signer mismatch must be rejected before durable queue ownership changes"
    );
    assert_eq!(
        std::fs::metadata(&journal_path)
            .expect("queue plan journal metadata")
            .len(),
        journal_len_before_rejection,
        "signer mismatch must not append a durable queue record"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read signer-mismatch response");
    let envelope: ErrorEnvelope =
        norito::decode_from_bytes(&body).expect("decode signer-mismatch error envelope");
    assert_eq!(envelope.code(), "queue_plan_synced_signer_mismatch");
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn validate_proxy_routing_plan_hint_rejects_forged_digest_and_roles() {
    let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    let native_plan = RoutingPlan::native_amx(
        coordinator,
        vec![iroha_core::queue::RouteLeg::new(
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(8)),
            iroha_core::queue::RouteLegRole::Participant,
        )],
    );
    let mut forged_digest = ToriiRoutingPlanHintV1::from(native_plan.clone());
    let advertised = Hash::new(b"torii-submit-proxy-forged-native-amx-plan-digest");
    let ToriiRoutingPlanHintV1::NativeAmx { plan_digest, .. } = &mut forged_digest else {
        panic!("expected native AMX hint");
    };
    *plan_digest = advertised;

    let err = super::validate_proxy_routing_plan_hint(forged_digest)
        .expect_err("submit proxy must reject a forged Native AMX plan digest");
    assert_eq!(
        err,
        iroha_core::torii_proxy::ToriiRoutingPlanHintError::native_amx_plan_digest_mismatch(
            advertised,
            native_plan.digest()
        )
    );

    let role_err = super::validate_proxy_routing_plan_hint(ToriiRoutingPlanHintV1::Single(
        iroha_core::torii_proxy::ToriiRouteLegHintV1 {
            route: ToriiRouteHintV1::from(coordinator),
            role: iroha_core::torii_proxy::ToriiRouteLegRoleV1::Participant,
        },
    ))
    .expect_err("submit proxy must reject malformed coordinator leg roles");
    assert_eq!(
        role_err,
        iroha_core::torii_proxy::ToriiRoutingPlanHintError::unexpected_coordinator_role(
            iroha_core::torii_proxy::ToriiRouteLegRoleV1::Participant
        )
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn validate_proxy_routing_plan_rejects_receiver_recomputed_plan() {
    let ingress_hint = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let resolved_route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(10));
    let ingress_plan = RoutingPlan::single(ingress_hint);
    let resolved_plan = RoutingPlan::single(resolved_route);

    let err = super::validate_proxy_routing_plan("submit_transaction", resolved_plan, ingress_plan)
        .expect_err("submit proxy must reject routing-plan drift");

    assert_eq!(
        err.ingress_digest,
        RoutingPlan::single(ingress_hint).digest()
    );
    assert_eq!(
        err.receiver_digest,
        RoutingPlan::single(resolved_route).digest()
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn validate_proxy_routing_plan_rejects_native_amx_participant_drift() {
    let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    let shared_participant = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    let ingress_participant = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(8));
    let receiver_participant = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(8));
    let ingress_plan = RoutingPlan::native_amx(
        coordinator,
        vec![
            iroha_core::queue::RouteLeg::new(
                shared_participant,
                iroha_core::queue::RouteLegRole::Participant,
            ),
            iroha_core::queue::RouteLeg::new(
                ingress_participant,
                iroha_core::queue::RouteLegRole::Participant,
            ),
        ],
    );
    let resolved_plan = RoutingPlan::native_amx(
        coordinator,
        vec![
            iroha_core::queue::RouteLeg::new(
                shared_participant,
                iroha_core::queue::RouteLegRole::Participant,
            ),
            iroha_core::queue::RouteLeg::new(
                receiver_participant,
                iroha_core::queue::RouteLegRole::Participant,
            ),
        ],
    );

    assert_eq!(
        ingress_plan.coordinator_route(),
        resolved_plan.coordinator_route()
    );
    let err = super::validate_proxy_routing_plan(
        "submit_transaction",
        resolved_plan.clone(),
        ingress_plan.clone(),
    )
    .expect_err("submit proxy must reject Native AMX participant drift");

    assert_eq!(err.ingress_digest, ingress_plan.digest());
    assert_eq!(err.receiver_digest, resolved_plan.digest());
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn effective_proxy_routing_decision_prefers_receiver_recomputed_route() {
    let ingress_hint = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let resolved_route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(10));

    assert_eq!(
        super::effective_proxy_routing_decision("verified_query", resolved_route, ingress_hint),
        resolved_route
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn proxy_signed_query_decode_requires_valid_signature_and_exact_bytes() {
    let key_pair =
        checked_torii_test_ed25519_keypair(0xfd, "derive signed proxy query fixture key");
    let authority = AccountId::new(key_pair.public_key().clone());
    let signed_query = signed_find_triggers_query_for_test(authority.clone(), &key_pair);
    let query_bytes = iroha_version::codec::EncodeVersioned::encode_versioned(&signed_query);
    let admission = signed_query_test_admission();

    let verified =
        super::decode_verified_proxy_signed_query(&query_bytes, "test proxy", admission.as_ref())
            .expect("the original signed query should verify");
    assert_eq!(verified.authority, authority);

    let mut forged_authority =
        <SignedQuery as iroha_version::codec::DecodeVersioned>::decode_all_versioned(&query_bytes)
            .expect("signed proxy query should round-trip");
    forged_authority.payload.authority = AccountId::new(
        checked_torii_test_ed25519_keypair(
            0xfe,
            "derive forged signed proxy query authority fixture key",
        )
        .public_key()
        .clone(),
    );
    assert!(
        super::decode_verified_proxy_signed_query(
            &iroha_version::codec::EncodeVersioned::encode_versioned(&forged_authority),
            "test proxy",
            admission.as_ref(),
        )
        .is_err(),
        "a peer cannot replace the client authority after signing",
    );

    let mut forged_request = signed_query;
    forged_request.payload.request = iroha_data_model::query::QueryRequest::Start(
        build_find_active_trigger_ids_query_for_test(),
    );
    assert!(
        super::decode_verified_proxy_signed_query(
            &iroha_version::codec::EncodeVersioned::encode_versioned(&forged_request),
            "test proxy",
            admission.as_ref(),
        )
        .is_err(),
        "a peer cannot replace the signed query payload",
    );

    let mut trailing = query_bytes;
    trailing.push(0);
    let Err(response) =
        super::decode_verified_proxy_signed_query(&trailing, "test proxy", admission.as_ref())
    else {
        panic!("trailing proxy query bytes must fail exact decoding");
    };
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn signed_proxy_route_scan_rejects_client_continuations_and_route_tampering() {
    let key_pair =
        checked_torii_test_ed25519_keypair(0xf9, "derive signed proxy continuation fixture key");
    let authority = AccountId::new(key_pair.public_key().clone());
    let cursor = iroha_data_model::query::parameters::ForwardCursor {
        query: "00".repeat(32),
        cursor: std::num::NonZeroU64::new(1).expect("one is non-zero"),
        gas_budget: None,
    };
    let continuation = authorize_query_for_test(
        iroha_data_model::query::QueryRequest::Continue(cursor),
        authority.clone(),
    )
    .sign(&key_pair);
    let admission = signed_query_test_admission();
    let request = super::decode_verified_proxy_signed_query(
        &iroha_version::codec::EncodeVersioned::encode_versioned(&continuation),
        "test route scan",
        admission.as_ref(),
    )
    .expect("the client continuation signature should be valid");
    let response = super::reject_proxy_client_continuation(&request, "signed route scan")
        .expect_err("client-provided proxy continuations must fail closed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let authorized = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(10));
    let tampered = RoutingDecision::new(LaneId::new(4), DataSpaceId::new(12));
    let response = super::validate_proxy_signed_query_route(&authority, &[authorized], tampered)
        .expect_err("a peer cannot replace the authorized route hint");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[test]
fn effective_proxy_signed_query_routing_decision_prefers_receiver_recomputed_route() {
    let ingress_hint = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let resolved_route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(10));

    assert_eq!(
        super::effective_proxy_signed_query_routing_decision(resolved_route, ingress_hint),
        resolved_route
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn signed_query_proxy_does_not_retry_after_ambiguous_dispatch() {
    let first_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x91, "derive retry first proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let second_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x92, "derive retry second proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
    let request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id: Hash::new(b"signed-query-ambiguous-dispatch"),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
            query_bytes: Vec::new(),
            expected_route: ToriiRouteHintV1::from(route),
            response_format: ToriiProxyResponseFormatV1::Norito,
        },
    };
    let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let attempts_ref = attempts.clone();
    let first_peer_id_for_closure = first_peer_id.clone();

    let response = super::execute_torii_proxy_request_across_candidates(
        vec![
            ToriiProxyCandidate::P2p(first_peer_id.clone()),
            ToriiProxyCandidate::P2p(second_peer_id.clone()),
        ],
        route,
        request,
        Duration::from_millis(50),
        move |candidate, _request| {
            let attempts = attempts_ref.clone();
            let first_peer_id = first_peer_id_for_closure.clone();
            async move {
                let peer_id = candidate.peer_id().clone();
                attempts
                    .lock()
                    .expect("attempt tracker should lock")
                    .push(peer_id.clone());
                if peer_id == first_peer_id {
                    return Err(ToriiProxyAttemptError::after_dispatch(
                        "authority response was lost after request dispatch",
                    ));
                }
                Ok(ToriiProxyHttpResponseV1 {
                    status_code: StatusCode::OK.as_u16(),
                    headers: Vec::new(),
                    body: b"proxy-ok".to_vec(),
                })
            }
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(
        attempts
            .lock()
            .expect("attempt tracker should lock")
            .as_slice(),
        &[first_peer_id]
    );
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("signed_query_outcome_unknown")
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn signed_query_proxy_tries_next_candidate_only_before_dispatch() {
    let first_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x93, "derive hedged first proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let second_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x94, "derive hedged second proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2));
    let request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id: Hash::new(b"signed-query-pre-dispatch-fallback"),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
            query_bytes: Vec::new(),
            expected_route: ToriiRouteHintV1::from(route),
            response_format: ToriiProxyResponseFormatV1::Norito,
        },
    };
    let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let attempts_ref = attempts.clone();
    let first_peer_id_for_closure = first_peer_id.clone();

    let response = super::execute_torii_proxy_request_across_candidates(
        vec![
            ToriiProxyCandidate::P2p(first_peer_id.clone()),
            ToriiProxyCandidate::P2p(second_peer_id.clone()),
        ],
        route,
        request,
        Duration::from_millis(20),
        move |candidate, _request| {
            let attempts = attempts_ref.clone();
            let first_peer_id = first_peer_id_for_closure.clone();
            async move {
                let peer_id = candidate.peer_id().clone();
                attempts
                    .lock()
                    .expect("attempt tracker should lock")
                    .push(peer_id.clone());
                if peer_id == first_peer_id {
                    return Err(ToriiProxyAttemptError::before_dispatch(
                        "request encoding failed before transport dispatch",
                    ));
                }
                Ok(ToriiProxyHttpResponseV1 {
                    status_code: StatusCode::OK.as_u16(),
                    headers: Vec::new(),
                    body: b"pre-dispatch-fallback-ok".to_vec(),
                })
            }
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(
        attempts
            .lock()
            .expect("attempt tracker should lock")
            .as_slice(),
        &[first_peer_id, second_peer_id]
    );
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    assert_eq!(body.as_ref(), b"pre-dispatch-fallback-ok");
}
