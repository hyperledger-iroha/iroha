#[tokio::test]
async fn sccp_recent_endpoint_never_reads_or_verifies_finality_sidecars() {
    let (app, message_id, _) = app_with_indexed_sccp_message_for_test(false);
    let message_id_hex = hex::encode(message_id);
    let sidecar = app
        .kura
        .store_root()
        .join("blocks")
        .join("v2_finality")
        .join("00000000000000000001.norito");
    assert!(!sidecar.exists(), "fixture starts without finality sidecar");
    let recent_response = routing::handle_v1_sccp_messages_recent(
        Arc::clone(&app.state),
        routing::SccpRecentWindowQuery::default(),
        utils::ResponseFormat::Json,
        acquire_query_admission(app.as_ref(), true)
            .await
            .expect("acquire recent-message test admission"),
    )
    .await
    .expect("recent metadata does not require finality");
    let recent_before = torii_body_bytes(recent_response, "recent body").await;
    let recent =
        norito::json::from_slice::<norito::json::Value>(&recent_before).expect("recent JSON");
    let item = recent
        .get("items")
        .and_then(norito::json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(norito::json::Value::as_object)
        .expect("one recent item");
    assert_eq!(
        item.get("message_id_hex")
            .and_then(norito::json::Value::as_str),
        Some(message_id_hex.as_str())
    );
    let links = item
        .get("links")
        .and_then(norito::json::Value::as_object)
        .expect("recent links");
    assert_eq!(links.len(), 2);
    assert!(links.contains_key("bundle_path"));
    assert!(links.contains_key("proof_request_path"));
    assert!(
        matches!(
            routing::handle_v1_sccp_message_bundle(
                Arc::clone(&app.state),
                message_id_hex.clone(),
                utils::ResponseFormat::Json,
                acquire_query_admission(app.as_ref(), true)
                    .await
                    .expect("acquire missing-bundle test admission"),
            )
            .await,
            Err(Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound
            )))
        ),
        "a valid legacy QC must not substitute for a missing exact-v2 artifact"
    );
    std::fs::create_dir_all(sidecar.parent().expect("sidecar directory"))
        .expect("create adversarial finality directory");
    std::fs::write(&sidecar, b"malformed-finality-sidecar")
        .expect("write adversarial finality sidecar");
    let recent_response = routing::handle_v1_sccp_messages_recent(
        Arc::clone(&app.state),
        routing::SccpRecentWindowQuery::default(),
        utils::ResponseFormat::Json,
        acquire_query_admission(app.as_ref(), true)
            .await
            .expect("acquire corrupted-recent test admission"),
    )
    .await
    .expect("malformed finality remains outside recent metadata path");
    let recent_after =
        torii_body_bytes(recent_response, "recent body after sidecar corruption").await;
    assert_eq!(recent_after, recent_before);
    assert!(matches!(
        routing::handle_v1_sccp_message_bundle(
            app.state.clone(),
            message_id_hex,
            utils::ResponseFormat::Json,
            acquire_query_admission(app.as_ref(), true)
                .await
                .expect("acquire corrupted-bundle test admission"),
        )
        .await,
        Err(Error::Query(ValidationFail::InternalError(_)))
    ));
}
#[tokio::test]
async fn finality_and_sccp_proof_routes_require_heavy_query_admission() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::from_millis(1);
    let message_id = "11".repeat(32);
    let errors = [
        handler_bridge_finality_proof(
            State(app.clone()),
            axum::extract::Path(1),
            HeaderMap::new(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("finality proof must acquire heavy admission"),
        handler_bridge_finality_bundle(
            State(app.clone()),
            axum::extract::Path(1),
            HeaderMap::new(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("finality bundle must acquire heavy admission"),
        handler_sccp_message_proof(
            State(app.clone()),
            axum::extract::Path(message_id.clone()),
            axum::extract::RawQuery(Some("retired_route=1".to_owned())),
            HeaderMap::new(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("SCCP message proof must acquire heavy admission"),
        handler_sccp_proof_request(
            State(app.clone()),
            axum::extract::Path(message_id),
            axum::extract::RawQuery(Some("retired_route=1".to_owned())),
            HeaderMap::new(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("SCCP proof request must acquire heavy admission"),
        handler_sccp_messages_recent(
            State(app),
            axum::extract::RawQuery(Some("retired_route=1".to_owned())),
            HeaderMap::new(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("SCCP recent query must acquire heavy admission"),
    ];
    for error in errors {
        assert!(
            matches!(
                error,
                Error::Query(ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
                ))
            ),
            "unexpected heavy-admission error: {error}"
        );
    }
}
#[tokio::test]
async fn zk_tree_queries_require_heavy_admission_before_state_integrity_work() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::ZERO;
    let roots_error = match handler_zk_roots(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        NoritoJson(routing::ZkRootsGetRequestDto {
            asset_id: String::new(),
            max: 1,
        }),
    )
    .await
    {
        Ok(_) => panic!("roots query must acquire heavy admission"),
        Err(error) => error,
    };
    let merkle_error = match handler_zk_merkle_path(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        NoritoJson(routing::ZkMerklePathGetRequestDto {
            asset_id: String::new(),
            commitments: Vec::new(),
        }),
    )
    .await
    {
        Ok(_) => panic!("Merkle-path query must acquire heavy admission"),
        Err(error) => error,
    };
    let errors = [roots_error, merkle_error];
    for error in errors {
        assert!(
            matches!(
                error,
                Error::Query(ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
                ))
            ),
            "unexpected ZK tree-query admission error: {error}"
        );
    }
}
#[cfg(feature = "zk-verify-batch")]
#[tokio::test]
async fn zk_verify_batch_honors_halo2_gate_before_compute_admission() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    Arc::get_mut(&mut app_mut.state)
        .expect("unique core state fixture")
        .zk
        .halo2
        .enabled = false;
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::ZERO;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    let error = match handler_zk_verify_batch(
        State(app),
        headers,
        crate::loopback_connect_info(),
        axum::body::Bytes::from_static(b"[]"),
    )
    .await
    {
        Ok(_) => panic!("disabled Halo2 verifier must fail closed"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::Query(ValidationFail::NotPermitted(message))
            if message == "halo2 verification is disabled in node configuration"
    ));
}
#[cfg(feature = "zk-verify-batch")]
#[tokio::test]
async fn enabled_zk_verify_batch_requires_heavy_compute_admission() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    Arc::get_mut(&mut app_mut.state)
        .expect("unique core state fixture")
        .zk
        .halo2
        .enabled = true;
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::ZERO;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    let error = match handler_zk_verify_batch(
        State(app),
        headers,
        crate::loopback_connect_info(),
        axum::body::Bytes::from_static(b"[]"),
    )
    .await
    {
        Ok(_) => panic!("batch verification must acquire heavy admission"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
        ))
    ));
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn por_history_routes_require_heavy_query_admission_before_projection() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::ZERO;
    let errors = [
        handler_get_sorafs_por_status(
            State(app.clone()),
            AxQuery(routing::PorStatusQueryDto {
                manifest: None,
                provider: None,
                epoch: None,
                status: None,
                limit: 1,
                max_bytes: 1,
                cursor: None,
            }),
        )
        .await
        .expect_err("PoR status projection must acquire heavy admission"),
        handler_get_sorafs_por_export(
            State(app.clone()),
            AxQuery(routing::PorExportQueryDto {
                start_epoch: None,
                end_epoch: None,
                limit: 1,
                max_bytes: 1,
                cursor: None,
            }),
        )
        .await
        .expect_err("PoR export projection must acquire heavy admission"),
        handler_get_sorafs_por_report(State(app), axum::extract::Path("2026-W01".to_owned()))
            .await
            .expect_err("PoR report projection must acquire heavy admission"),
    ];
    for error in errors {
        assert!(
            matches!(
                error,
                Error::Query(ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
                ))
            ),
            "unexpected PoR heavy-admission error: {error}"
        );
    }
}
async fn heavy_route_auth_errors_for_test(app: SharedAppState, headers: HeaderMap) -> [Error; 5] {
    let message_id = "11".repeat(32);
    [
        handler_bridge_finality_proof(
            State(app.clone()),
            axum::extract::Path(1),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("finality proof authentication must reject"),
        handler_bridge_finality_bundle(
            State(app.clone()),
            axum::extract::Path(1),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("finality bundle authentication must reject"),
        handler_sccp_message_proof(
            State(app.clone()),
            axum::extract::Path(message_id.clone()),
            axum::extract::RawQuery(None),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("SCCP message authentication must reject"),
        handler_sccp_proof_request(
            State(app.clone()),
            axum::extract::Path(message_id),
            axum::extract::RawQuery(None),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("SCCP request authentication must reject"),
        handler_sccp_messages_recent(
            State(app),
            axum::extract::RawQuery(None),
            headers,
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("SCCP recent authentication must reject"),
    ]
}
async fn light_sccp_route_auth_errors_for_test(
    app: SharedAppState,
    headers: HeaderMap,
) -> [Error; 2] {
    [
        handler_sccp_registry(
            State(app.clone()),
            axum::extract::RawQuery(Some("retired_route=1".to_owned())),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("SCCP registry authentication must reject"),
        handler_sccp_capabilities(
            State(app),
            axum::extract::RawQuery(Some("retired_route=1".to_owned())),
            headers,
            crate::loopback_connect_info(),
        )
        .await
        .expect_err("SCCP capabilities authentication must reject"),
    ]
}
#[tokio::test]
async fn all_sccp_and_bridge_read_routes_fail_closed_on_empty_or_duplicate_tokens() {
    for duplicate in [false, true] {
        let mut app = mk_app_state_for_tests();
        let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
        app_mut.require_api_token = true;
        app_mut.api_tokens_set = if duplicate {
            Arc::new(HashSet::from(["valid-token".to_owned()]))
        } else {
            Arc::new(HashSet::new())
        };
        app_mut.rate_limiter = limits::RateLimiter::new(Some(1), Some(8));
        app_mut.query_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        app_mut.query_queue_timeout = Duration::ZERO;
        let mut headers = HeaderMap::new();
        let rate_key = if duplicate {
            headers.append(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
            headers.append(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
            "valid-token"
        } else {
            "127.0.0.1"
        };
        let heavy_errors =
            heavy_route_auth_errors_for_test(Arc::clone(&app), headers.clone()).await;
        let light_errors = light_sccp_route_auth_errors_for_test(Arc::clone(&app), headers).await;
        for error in heavy_errors.into_iter().chain(light_errors) {
            assert!(
                matches!(&error, Error::Query(ValidationFail::NotPermitted(_))),
                "unexpected authentication error: {error}"
            );
        }
        assert!(
            app.rate_limiter.allow_cost(rate_key, 8).await,
            "authentication failures must not consume rate capacity"
        );
    }
}
#[tokio::test]
async fn heavy_finality_routes_reject_unsupported_accept_before_rate_and_admission() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    app_mut.rate_limiter = limits::RateLimiter::new(Some(1), Some(8));
    app_mut.query_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::ZERO;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::ACCEPT,
        HeaderValue::from_static("image/png"),
    );
    let message_id = "11".repeat(32);
    let responses = [
        handler_bridge_finality_proof(
            State(app.clone()),
            axum::extract::Path(1),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect("early finality negotiation"),
        handler_bridge_finality_bundle(
            State(app.clone()),
            axum::extract::Path(1),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect("early finality-bundle negotiation"),
        handler_sccp_message_proof(
            State(app.clone()),
            axum::extract::Path(message_id.clone()),
            axum::extract::RawQuery(None),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect("early SCCP bundle negotiation"),
        handler_sccp_proof_request(
            State(app.clone()),
            axum::extract::Path(message_id),
            axum::extract::RawQuery(None),
            headers.clone(),
            crate::loopback_connect_info(),
        )
        .await
        .expect("early SCCP request negotiation"),
        handler_sccp_messages_recent(
            State(app.clone()),
            axum::extract::RawQuery(None),
            headers,
            crate::loopback_connect_info(),
        )
        .await
        .expect("early SCCP recent negotiation"),
    ];
    for response in responses {
        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
    }
    assert!(
        app.rate_limiter
            .allow_cost("127.0.0.1", FINALITY_HEAVY_QUERY_RATE_COST)
            .await,
        "unsupported representations must reject before rate accounting"
    );
}
#[test]
fn signed_query_preauth_key_ignores_unauthenticated_api_token_text() {
    let remote = Some(
        "203.0.113.17"
            .parse::<std::net::IpAddr>()
            .expect("valid test client address"),
    );
    let mut first = HeaderMap::new();
    first.insert(
        HEADER_API_TOKEN,
        HeaderValue::from_static("attacker-token-1"),
    );
    let mut second = HeaderMap::new();
    second.insert(
        HEADER_API_TOKEN,
        HeaderValue::from_static("attacker-token-2"),
    );
    assert_eq!(
        signed_query_preauth_rate_limit_key(&first, remote, false),
        signed_query_preauth_rate_limit_key(&second, remote, false),
        "raw API-token text must not choose a pre-verification rate bucket"
    );
    assert_ne!(
        signed_query_preauth_rate_limit_key(&first, remote, true),
        signed_query_preauth_rate_limit_key(&second, remote, true),
        "an already-validated API credential may identify its own caller budget"
    );
}
#[test]
fn signed_query_authority_keys_are_canonical_and_namespaced() {
    let first = checked_torii_test_account_id(0x71, "derive first query-admission authority");
    let second = checked_torii_test_account_id(0x72, "derive second query-admission authority");
    let first_key = signed_query_authority_rate_limit_key(&first);
    let second_key = signed_query_authority_rate_limit_key(&second);
    assert_eq!(first_key, signed_query_authority_rate_limit_key(&first));
    assert_ne!(first_key, second_key);
    assert!(first_key.starts_with("v1/query:authority:"));
}
#[tokio::test]
async fn signed_query_authority_admission_isolated_by_verified_identity() {
    let first =
        checked_torii_test_account_id(0x74, "derive first enforced query-admission authority");
    let second =
        checked_torii_test_account_id(0x75, "derive second enforced query-admission authority");
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique query-authority admission fixture")
        .query_authority_rate_limiter = limits::RateLimiter::new_per_minute(Some(1), Some(1));
    admit_signed_query_authority(app.as_ref(), &first)
        .await
        .expect("first authority consumes its own budget");
    admit_signed_query_authority(app.as_ref(), &second)
        .await
        .expect("second authority has an independent budget");
    let error = admit_signed_query_authority(app.as_ref(), &first)
        .await
        .expect_err("first authority cannot exceed its budget");
    assert!(matches!(
        error,
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
        ))
    ));
}
#[tokio::test]
async fn signed_query_token_rotation_cannot_escape_origin_and_authority_budgets() {
    use iroha_data_model::query::{
        QueryRequest, SingularQueryBox, runtime::prelude::FindAbiVersion,
    };
    let key_pair =
        checked_torii_test_ed25519_keypair(0x73, "derive signed-query admission fixture authority");
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    let app_mut = Arc::get_mut(&mut app).expect("unique signed-query admission fixture");
    app_mut.query_preauth_rate_limiter = limits::RateLimiter::new_per_minute(Some(1), Some(2));
    app_mut.query_authority_rate_limiter = limits::RateLimiter::new_per_minute(Some(1), Some(2));
    let signed_query = || {
        authorize_query_for_test(
            QueryRequest::Singular(SingularQueryBox::FindAbiVersion(FindAbiVersion)),
            authority.clone(),
        )
        .sign(&key_pair)
    };
    for token in ["attacker-token-1", "attacker-token-2"] {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_API_TOKEN,
            HeaderValue::from_str(token).expect("valid test header"),
        );
        let response = handler_signed_query(
            State(Arc::clone(&app)),
            headers,
            crate::loopback_connect_info(),
            None,
            crate::NoritoQuery(QueryOptions::default()),
            versioned_query_for_test(signed_query()),
        )
        .await
        .expect("requests within both signed-query budgets must execute");
        assert_eq!(response.status(), StatusCode::OK);
    }
    assert_eq!(
        app.query_preauth_rate_limiter.bucket_count().await,
        1,
        "rotated raw tokens must share one effective-origin bucket"
    );
    assert_eq!(
        app.query_authority_rate_limiter.bucket_count().await,
        1,
        "replayed requests from one signer must share one authority bucket"
    );
    let mut rotated = HeaderMap::new();
    rotated.insert(
        HEADER_API_TOKEN,
        HeaderValue::from_static("attacker-token-3"),
    );
    let error = handler_signed_query(
        State(app),
        rotated,
        crate::loopback_connect_info(),
        None,
        crate::NoritoQuery(QueryOptions::default()),
        versioned_query_for_test(signed_query()),
    )
    .await
    .expect_err("rotating raw token text must not create a fresh query budget");
    assert!(matches!(
        error,
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
        ))
    ));
}
#[tokio::test]
async fn zero_query_queue_timeout_is_fail_fast_and_does_not_reserve_general_capacity() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    app_mut.query_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::ZERO;
    let query = Arc::clone(&app_mut.query_inflight);
    let outcome = tokio::time::timeout(
        Duration::from_millis(50),
        acquire_query_admission(app.as_ref(), true),
    )
    .await
    .expect("zero queue timeout must never wait");
    let error = match outcome {
        Ok(_) => panic!("saturated heavy admission must reject"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
        ))
    ));
    assert_eq!(query.available_permits(), 1);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heavy_admission_waits_for_heavy_capacity_before_reserving_general_capacity() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    app_mut.query_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::from_secs(5);
    let query = Arc::clone(&app_mut.query_inflight);
    let heavy = Arc::clone(&app_mut.query_heavy_inflight);
    let app_for_task = Arc::clone(&app);
    let admission_task =
        tokio::spawn(async move { acquire_query_admission(app_for_task.as_ref(), true).await });
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;
    assert_eq!(
        query.available_permits(),
        1,
        "a heavy waiter must not occupy general query capacity"
    );
    heavy.add_permits(1);
    let admission = tokio::time::timeout(Duration::from_secs(1), admission_task)
        .await
        .expect("released heavy waiter completes")
        .expect("admission task")
        .expect("admission succeeds");
    assert_eq!(query.available_permits(), 0);
    assert_eq!(heavy.available_permits(), 0);
    drop(admission);
    assert_eq!(query.available_permits(), 1);
    assert_eq!(heavy.available_permits(), 1);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heavy_admission_uses_one_end_to_end_queue_deadline() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    app_mut.query_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::from_millis(500);
    let heavy = Arc::clone(&app_mut.query_heavy_inflight);
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        heavy.add_permits(1);
    });
    let outcome = tokio::time::timeout(
        Duration::from_millis(550),
        acquire_query_admission(app.as_ref(), true),
    )
    .await
    .expect("both semaphore waits must share the configured 500ms deadline");
    let error = match outcome {
        Ok(_) => panic!("general saturation must reject admission"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
        ))
    ));
    release.await.expect("heavy release task");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_query_cannot_release_admission_before_blocking_worker_finishes() {
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique Torii app fixture");
    app_mut.query_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app_mut.proof_body_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    let query_semaphore = Arc::clone(&app_mut.query_inflight);
    let heavy_semaphore = Arc::clone(&app_mut.query_heavy_inflight);
    let body_semaphore = Arc::clone(&app_mut.proof_body_inflight);
    let admission = acquire_query_admission(app.as_ref(), true)
        .await
        .expect("acquire sole query admission")
        .with_body_permit(
            body_semaphore
                .clone()
                .try_acquire_owned()
                .expect("acquire sole body admission"),
        );
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let awaiting_request = tokio::spawn(routing::run_admitted_blocking(
        admission,
        "admission cancellation test worker failed",
        move || {
            started_tx.send(()).expect("report physical worker start");
            release_rx
                .blocking_recv()
                .expect("release physical test worker");
            Ok(())
        },
    ));
    started_rx.await.expect("physical worker started");
    awaiting_request.abort();
    tokio::task::yield_now().await;
    assert!(
        query_semaphore.clone().try_acquire_owned().is_err(),
        "request cancellation must not release the general query permit"
    );
    assert!(
        heavy_semaphore.clone().try_acquire_owned().is_err(),
        "request cancellation must not release the heavy query permit"
    );
    assert!(
        body_semaphore.clone().try_acquire_owned().is_err(),
        "request cancellation must not release the body permit"
    );
    release_tx.send(()).expect("release physical worker");
    let query_permit =
        tokio::time::timeout(Duration::from_secs(5), query_semaphore.acquire_owned())
            .await
            .expect("general permit is released after physical completion")
            .expect("general query semaphore remains open");
    let heavy_permit =
        tokio::time::timeout(Duration::from_secs(5), heavy_semaphore.acquire_owned())
            .await
            .expect("heavy permit is released after physical completion")
            .expect("heavy query semaphore remains open");
    let body_permit = tokio::time::timeout(Duration::from_secs(5), body_semaphore.acquire_owned())
        .await
        .expect("body permit is released after physical completion")
        .expect("body semaphore remains open");
    drop((query_permit, heavy_permit, body_permit));
}
#[tokio::test]
async fn finality_rate_weight_caps_to_burst_without_disabling_the_route() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique Torii app fixture")
        .rate_limiter = limits::RateLimiter::new(Some(1), Some(7));
    let key = "capped-finality-cost";
    rate_limit_requests_with_cost(&app, key, FINALITY_HEAVY_QUERY_RATE_COST)
        .await
        .expect("a positive configured burst must keep finality serviceable");
    let finality_error = rate_limit_requests(&app, key)
        .await
        .expect_err("capped weighted request must consume the full burst");
    assert!(matches!(
        finality_error,
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
        ))
    ));
    rate_limit_requests(&app, "independent-light-route")
        .await
        .expect("weighted accounting remains isolated by caller key");
}
#[tokio::test]
async fn query_free_sccp_handlers_reject_legacy_query_material_before_lookup() {
    let app = mk_app_state_for_tests();
    let remote = axum::extract::ConnectInfo(
        "127.0.0.1:4040"
            .parse::<std::net::SocketAddr>()
            .expect("test socket"),
    );
    let legacy_query = || {
        axum::extract::RawQuery(Some(
            "network_id_hex=11&proof_bytes_hex=22&allow_unready=true".to_owned(),
        ))
    };
    let message_id = "11".repeat(32);
    let bundle_error = handler_sccp_message_proof(
        State(app.clone()),
        axum::extract::Path(message_id.clone()),
        legacy_query(),
        HeaderMap::new(),
        remote,
    )
    .await
    .expect_err("bundle query material must reject");
    let request_error = handler_sccp_proof_request(
        State(app.clone()),
        axum::extract::Path(message_id),
        legacy_query(),
        HeaderMap::new(),
        remote,
    )
    .await
    .expect_err("proof-request query material must reject");
    let registry_error =
        handler_sccp_registry(State(app.clone()), legacy_query(), HeaderMap::new(), remote)
            .await
            .expect_err("registry query material must reject");
    let recent_error =
        handler_sccp_messages_recent(State(app.clone()), legacy_query(), HeaderMap::new(), remote)
            .await
            .expect_err("recent-message legacy query material must reject");
    let capabilities_error =
        handler_sccp_capabilities(State(app), legacy_query(), HeaderMap::new(), remote)
            .await
            .expect_err("capability query material must reject");
    for error in [
        bundle_error,
        request_error,
        registry_error,
        recent_error,
        capabilities_error,
    ] {
        let Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
        )) = error
        else {
            panic!("unexpected SCCP query rejection: {error}");
        };
        assert!(
            message.contains("does not accept query parameters")
                || message.contains("is not supported")
        );
    }
}
#[test]
fn first_release_sccp_router_has_only_closed_read_surfaces() {
    let source = include_str!("../../lib.rs");
    for required in [
        "/v1/sccp/capabilities",
        "/v1/sccp/registry",
        "/v1/sccp/proofs/message/{message_id}",
        "/v1/sccp/proof-requests/{message_id}",
        "/v1/sccp/messages/recent",
        "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
    ] {
        assert!(source.contains(required), "missing SCCP route: {required}");
    }
    for retired in [
        concat!("/v1/sccp/", "manifests"),
        concat!("/v1/sccp/", "artifacts/message/{message_id}"),
        concat!("/v1/sccp/", "jobs/message/{message_id}"),
    ] {
        assert!(
            !source.contains(retired),
            "retired SCCP route reappeared: {retired}"
        );
    }
}
fn clone_private_key(
    src: &iroha_data_model::prelude::ExposedPrivateKey,
) -> iroha_data_model::prelude::ExposedPrivateKey {
    iroha_data_model::prelude::ExposedPrivateKey(src.0.clone())
}
#[derive(Clone)]
struct TestLocalReadRuntime {
    snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot,
    state_dir: PathBuf,
    local_peer_id: Option<String>,
    result: Result<
        iroha_core::soracloud_runtime::SoracloudLocalReadResponse,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    >,
    captured_requests: Arc<std::sync::Mutex<Vec<SoracloudLocalReadRequest>>>,
}
impl TestLocalReadRuntime {
    fn with_result(
        snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot,
        state_dir: PathBuf,
        local_peer_id: Option<String>,
        result: Result<
            iroha_core::soracloud_runtime::SoracloudLocalReadResponse,
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
        >,
    ) -> Self {
        Self {
            snapshot,
            state_dir,
            local_peer_id,
            result,
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn unavailable(local_peer_id: Option<String>, message: &'static str) -> Self {
        Self::with_result(
            iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id,
            Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    message,
                ),
            ),
        )
    }

    fn snapshot_only(snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot) -> Self {
        Self::with_result(
            snapshot,
            PathBuf::from("/tmp/soracloud/runtime"),
            None,
            Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "test runtime handle does not implement local reads",
                ),
            ),
        )
    }

    fn capturing_requests(
        mut self,
        captured: Arc<std::sync::Mutex<Vec<SoracloudLocalReadRequest>>>,
    ) -> Self {
        self.captured_requests = captured;
        self
    }
}
impl iroha_core::soracloud_runtime::SoracloudRuntimeReadHandle for TestLocalReadRuntime {
    fn snapshot(&self) -> iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot {
        self.snapshot.clone()
    }
    fn state_dir(&self) -> PathBuf {
        self.state_dir.clone()
    }
    fn local_peer_id(&self) -> Option<String> {
        self.local_peer_id.clone()
    }
}
impl iroha_core::soracloud_runtime::SoracloudRuntime for TestLocalReadRuntime {
    fn execute_local_read(
        &self,
        request: SoracloudLocalReadRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudLocalReadResponse,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        self.captured_requests
            .lock()
            .expect("capture lock")
            .push(request);
        self.result.clone()
    }
    fn execute_ordered_mailbox(
        &self,
        _request: iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionResult,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        let message = if self.state_dir == PathBuf::from("/tmp/soracloud/runtime") {
            "test runtime handle does not implement mailbox execution"
        } else {
            "test runtime does not implement mailbox execution"
        };
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                message,
            ),
        )
    }
    fn execute_apartment(
        &self,
        _request: iroha_core::soracloud_runtime::SoracloudApartmentExecutionRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudApartmentExecutionResult,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        let message = if self.state_dir == PathBuf::from("/tmp/soracloud/runtime") {
            "test runtime handle does not implement apartment execution"
        } else {
            "test runtime does not implement apartment execution"
        };
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                message,
            ),
        )
    }
}
#[derive(Clone)]
struct BlockingLocalReadRuntime {
    started: Arc<std::sync::atomic::AtomicBool>,
    finished: Arc<std::sync::atomic::AtomicBool>,
    release: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
}
struct BlockingLocalReadReleaseGuard {
    release: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
}
impl BlockingLocalReadReleaseGuard {
    fn new(release: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>) -> Self {
        Self { release }
    }
    fn release(&self) {
        let (release, released) = self.release.as_ref();
        *release
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        released.notify_all();
    }
}
impl Drop for BlockingLocalReadReleaseGuard {
    fn drop(&mut self) {
        self.release();
    }
}
impl iroha_core::soracloud_runtime::SoracloudRuntimeReadHandle for BlockingLocalReadRuntime {
    fn snapshot(&self) -> iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot {
        iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default()
    }
    fn state_dir(&self) -> PathBuf {
        PathBuf::from("/tmp/soracloud/blocking-local-read-test")
    }
}
impl iroha_core::soracloud_runtime::SoracloudRuntime for BlockingLocalReadRuntime {
    fn execute_local_read(
        &self,
        _request: SoracloudLocalReadRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudLocalReadResponse,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        self.started.store(true, Ordering::SeqCst);
        let (release, released) = self.release.as_ref();
        let mut can_finish = release.lock().expect("blocking runtime release lock");
        while !*can_finish {
            can_finish = released
                .wait(can_finish)
                .expect("blocking runtime release wait");
        }
        self.finished.store(true, Ordering::SeqCst);
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "blocking local-read test completed",
            ),
        )
    }
    fn execute_ordered_mailbox(
        &self,
        _request: iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionResult,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        unreachable!("blocking local-read test does not execute mailboxes")
    }
    fn execute_apartment(
        &self,
        _request: iroha_core::soracloud_runtime::SoracloudApartmentExecutionRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudApartmentExecutionResult,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        unreachable!("blocking local-read test does not execute apartments")
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_soracloud_local_read_keeps_blocking_capacity_until_worker_stops() {
    let admission = Arc::new(tokio::sync::Semaphore::new(1));
    let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let release = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let release_guard = BlockingLocalReadReleaseGuard::new(Arc::clone(&release));
    let runtime: iroha_core::soracloud_runtime::SharedSoracloudRuntime =
        Arc::new(BlockingLocalReadRuntime {
            started: Arc::clone(&started),
            finished: Arc::clone(&finished),
            release: Arc::clone(&release),
        });
    let request = SoracloudLocalReadRequest {
        observed_height: 1,
        observed_block_hash: None,
        service_name: "blocking_service".to_owned(),
        service_version: "1.0.0".to_owned(),
        handler_name: "query".to_owned(),
        handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
        request_method: "POST".to_owned(),
        request_path: "/query".to_owned(),
        handler_path: "/query".to_owned(),
        request_query: None,
        request_headers: std::collections::BTreeMap::new(),
        request_body: Vec::new(),
        request_commitment: Hash::new(b"blocking-local-read"),
    };

    let first = tokio::spawn(execute_soracloud_local_read_off_reactor_with_admission(
        Arc::clone(&admission),
        Arc::clone(&runtime),
        request.clone(),
    ));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !started.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("blocking local-read worker must start");
    first.abort();
    assert!(
        first
            .await
            .expect_err("cancelled local-read task must stop awaiting its worker")
            .is_cancelled()
    );

    let saturated = execute_soracloud_local_read_off_reactor_with_admission(
        Arc::clone(&admission),
        Arc::clone(&runtime),
        request,
    )
    .await
    .expect_err("detached blocking work must retain its admission permit");
    assert!(saturated.message.contains("capacity is saturated"));

    release_guard.release();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !finished.load(Ordering::SeqCst) || admission.available_permits() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("stopped blocking work must eventually release capacity");
}
#[derive(Clone)]
struct TestMailboxRuntime {
    snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot,
    state_dir: PathBuf,
    local_peer_id: Option<String>,
    result: Result<
        iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionResult,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    >,
    captured_requests: Arc<
        std::sync::Mutex<
            Vec<iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionRequest>,
        >,
    >,
}
impl iroha_core::soracloud_runtime::SoracloudRuntimeReadHandle for TestMailboxRuntime {
    fn snapshot(&self) -> iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot {
        self.snapshot.clone()
    }
    fn state_dir(&self) -> PathBuf {
        self.state_dir.clone()
    }
    fn local_peer_id(&self) -> Option<String> {
        self.local_peer_id.clone()
    }
}
impl iroha_core::soracloud_runtime::SoracloudRuntime for TestMailboxRuntime {
    fn execute_local_read(
        &self,
        _request: SoracloudLocalReadRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudLocalReadResponse,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test mailbox runtime does not implement local reads",
            ),
        )
    }
    fn execute_ordered_mailbox(
        &self,
        request: iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudOrderedMailboxExecutionResult,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        self.captured_requests
            .lock()
            .expect("capture lock")
            .push(request);
        self.result.clone()
    }
    fn execute_apartment(
        &self,
        _request: iroha_core::soracloud_runtime::SoracloudApartmentExecutionRequest,
    ) -> Result<
        iroha_core::soracloud_runtime::SoracloudApartmentExecutionResult,
        iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    > {
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test mailbox runtime does not implement apartment execution",
            ),
        )
    }
}
fn sample_soracloud_runtime_snapshot(
    health_status: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
) -> iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot {
    let services = std::collections::BTreeMap::from([(
        "web_portal".to_string(),
        std::collections::BTreeMap::from([(
            "1.0.0".to_string(),
            iroha_core::soracloud_runtime::SoracloudRuntimeServicePlan {
                service_name: "web_portal".to_string(),
                service_version: "1.0.0".to_string(),
                role: iroha_core::soracloud_runtime::SoracloudRuntimeRevisionRole::Active,
                traffic_percent: 100,
                runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1::Ivm,
                execution_plane:
                    iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::DeterministicService,
                bundle_hash: "hash:bundle".to_string(),
                bundle_path: "service.to".to_string(),
                entrypoint: "start".to_string(),
                inrou: None,
                bundle_cache_path: "/tmp/soracloud/runtime/web_portal/1.0.0/service.to".to_string(),
                bundle_available_locally: true,
                process_generation: Some(7),
                desired_replica_count: 1,
                local_replica_slots: Vec::new(),
                local_replicas: Vec::new(),
                health_status,
                load_factor_bps: 2_500,
                authoritative_pending_mailbox_messages: 3,
                rollout_handle: None,
                config_generation: 0,
                secret_generation: 0,
                quota_class: None,
                service_lease_status: None,
                lease_expires_height: None,
                remaining_runtime_balance: None,
                config_entry_count: 0,
                secret_entry_count: 0,
                config_exports: vec![],
                supports_host_read_config: true,
                supports_host_read_secret_envelope: true,
                materialization_dir: "/tmp/soracloud/runtime/web_portal/1.0.0".to_string(),
                config_materialization_dir: "/tmp/soracloud/runtime/web_portal/1.0.0/configs"
                    .to_string(),
                effective_env: BTreeMap::new(),
                effective_env_materialization_path:
                    "/tmp/soracloud/runtime/web_portal/1.0.0/effective_env.json".to_string(),
                config_exports_materialization_dir:
                    "/tmp/soracloud/runtime/web_portal/1.0.0/config_exports".to_string(),
                secret_envelopes_materialization_dir:
                    "/tmp/soracloud/runtime/web_portal/1.0.0/secret_envelopes".to_string(),
                lease_volumes: Vec::new(),
                mailboxes: vec![],
                artifacts: vec![
                    iroha_core::soracloud_runtime::SoracloudRuntimeArtifactPlan {
                        kind: iroha_data_model::soracloud::SoraArtifactKindV1::Bundle,
                        artifact_hash: "hash:artifact".to_string(),
                        artifact_path: "public/index.html".to_string(),
                        handler_name: Some("asset".to_string()),
                        local_cache_path: "/tmp/soracloud/cache/hash-artifact/public-index.html"
                            .to_string(),
                        available_locally: false,
                    },
                ],
            },
        )]),
    )]);
    let apartments = std::collections::BTreeMap::from([(
        "ops_agent".to_string(),
        iroha_core::soracloud_runtime::SoracloudRuntimeApartmentPlan {
            apartment_name: "ops_agent".to_string(),
            manifest_hash: "hash:manifest".to_string(),
            status: iroha_data_model::soracloud::SoraAgentRuntimeStatusV1::Running,
            process_generation: 3,
            lease_expires_height: 90,
            last_active_sequence: 88,
            materialization_dir: "/tmp/soracloud/runtime/apartments/ops_agent".to_string(),
            pending_wallet_request_count: 1,
            pending_mailbox_message_count: 4,
            autonomy_budget_remaining_units: 120,
            approved_artifact_count: 2,
            autonomy_run_count: 5,
            revoked_policy_capability_count: 1,
        },
    )]);
    iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot {
        schema_version: 1,
        observed_height: 42,
        observed_block_hash: Some("hash:block".to_string()),
        local_peer_id: None,
        services,
        apartments,
    }
}
fn seed_public_soracloud_world() -> World {
    let mut world = World::new();
    let service_name: iroha_data_model::name::Name = "web_portal".parse().expect("service");
    let bundle = iroha_data_model::soracloud::SoraDeploymentBundleV1 {
        schema_version: iroha_data_model::soracloud::SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container: iroha_data_model::soracloud::SoraContainerManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
            runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1::Ivm,
            bundle_hash: Hash::new(b"public-bundle"),
            bundle_path: "/bundles/public.to".to_owned(),
            entrypoint: "main".to_owned(),
            args: Vec::new(),
            env: BTreeMap::new(),
            inrou: None,
            required_config_names: Vec::new(),
            required_secret_names: Vec::new(),
            config_exports: Vec::new(),
            capabilities: iroha_data_model::soracloud::SoraCapabilityPolicyV1 {
                network: iroha_data_model::soracloud::SoraNetworkPolicyV1::Isolated,
                allow_state_writes: true,
                allow_model_inference: false,
                allow_model_training: false,
            },
            resources: iroha_data_model::soracloud::SoraResourceLimitsV1 {
                cpu_millis: std::num::NonZeroU32::new(500).expect("cpu"),
                memory_bytes: std::num::NonZeroU64::new(128 * 1024 * 1024).expect("memory"),
                ephemeral_storage_bytes: std::num::NonZeroU64::new(16 * 1024 * 1024)
                    .expect("storage"),
                max_open_files_per_process: std::num::NonZeroU32::new(256).expect("files"),
                max_tasks: std::num::NonZeroU16::new(16).expect("tasks"),
            },
            lifecycle: iroha_data_model::soracloud::SoraLifecycleHooksV1 {
                start_grace_secs: std::num::NonZeroU32::new(5).expect("start grace"),
                stop_grace_secs: std::num::NonZeroU32::new(5).expect("stop grace"),
                healthcheck_path: Some("/health".to_owned()),
            },
        },
        service: iroha_data_model::soracloud::SoraServiceManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_MANIFEST_VERSION_V1,
            service_name: service_name.clone(),
            service_version: "2026.02.0".to_owned(),
            execution_plane:
                iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::DeterministicService,
            container: iroha_data_model::soracloud::SoraContainerManifestRefV1 {
                manifest_hash: Hash::new(b"public-container-manifest"),
                expected_schema_version:
                    iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
            },
            replicas: std::num::NonZeroU16::new(1).expect("replicas"),
            route: Some(iroha_data_model::soracloud::SoraRouteTargetV1 {
                host: "portal.sora".to_owned(),
                path_prefix: "/app".to_owned(),
                service_port: std::num::NonZeroU16::new(8080).expect("port"),
                visibility: iroha_data_model::soracloud::SoraRouteVisibilityV1::Public,
                tls_mode: iroha_data_model::soracloud::SoraTlsModeV1::Required,
            }),
            rollout: iroha_data_model::soracloud::SoraRolloutPolicyV1 {
                canary_percent: 0,
                max_unavailable_replicas: 0,
                health_window_secs: std::num::NonZeroU32::new(30).expect("window"),
                automatic_rollback_failures: std::num::NonZeroU32::new(1).expect("rollback"),
            },
            economics: Default::default(),
            state_bindings: Vec::new(),
            lease_volumes: Vec::new(),
            handlers: vec![
                iroha_data_model::soracloud::SoraServiceHandlerV1 {
                    handler_name: "assets".parse().expect("handler"),
                    class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Asset,
                    entrypoint: "serve_assets".to_owned(),
                    route_path: Some("/assets".to_owned()),
                    certified_response:
                        iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::StateCommitment,
                    mailbox: None,
                },
                iroha_data_model::soracloud::SoraServiceHandlerV1 {
                    handler_name: "query".parse().expect("handler"),
                    class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query,
                    entrypoint: "serve_query".to_owned(),
                    route_path: Some("/query".to_owned()),
                    certified_response:
                        iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::AuditReceipt,
                    mailbox: None,
                },
                iroha_data_model::soracloud::SoraServiceHandlerV1 {
                    handler_name: "update".parse().expect("handler"),
                    class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update,
                    entrypoint: "apply_update".to_owned(),
                    route_path: Some("/update".to_owned()),
                    certified_response:
                        iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                    mailbox: Some(iroha_data_model::soracloud::SoraMailboxContractV1 {
                        queue_name: "public_updates".parse().expect("queue"),
                        max_pending_messages: std::num::NonZeroU32::new(256)
                            .expect("pending limit"),
                        max_message_bytes: std::num::NonZeroU64::new(65_536)
                            .expect("payload limit"),
                        retention_blocks: std::num::NonZeroU32::new(32).expect("retention"),
                    }),
                },
                iroha_data_model::soracloud::SoraServiceHandlerV1 {
                    handler_name: "ciphertext_update".parse().expect("handler"),
                    class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update,
                    entrypoint: "apply_ciphertext_update".to_owned(),
                    route_path: Some("/ciphertext/update".to_owned()),
                    certified_response:
                        iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                    mailbox: Some(iroha_data_model::soracloud::SoraMailboxContractV1 {
                        queue_name: "ciphertext_updates".parse().expect("queue"),
                        max_pending_messages: std::num::NonZeroU32::new(128)
                            .expect("pending limit"),
                        max_message_bytes: std::num::NonZeroU64::new(65_536)
                            .expect("payload limit"),
                        retention_blocks: std::num::NonZeroU32::new(64).expect("retention"),
                    }),
                },
            ],
            artifacts: Vec::new(),
        },
    };
    world.soracloud_service_revisions_mut_for_testing().insert(
        (
            service_name.as_ref().to_owned(),
            bundle.service.service_version.clone(),
        ),
        bundle.clone(),
    );
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(
            service_name.clone(),
            iroha_data_model::soracloud::SoraServiceDeploymentStateV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                service_name,
                current_service_version: bundle.service.service_version.clone(),
                current_service_manifest_hash: bundle.service_manifest_hash(),
                current_container_manifest_hash: bundle.container_manifest_hash(),
                revision_count: 1,
                process_generation: 1,
                process_started_sequence: 1,
                active_rollout: None,
                last_rollout: None,
                config_generation: 0,
                secret_generation: 0,
                service_configs: std::collections::BTreeMap::new(),
                service_secrets: std::collections::BTreeMap::new(),
                fhe_policy_records: BTreeMap::new(),
                service_lease: None,
                lease_volume_states: Vec::new(),
            },
        );
    world
}

fn hosted_http_runtime_plan(
    materialization_dir: &Path,
    service_name: &str,
    service_version: &str,
    materialized_bundle_hash: String,
    role: iroha_core::soracloud_runtime::SoracloudRuntimeRevisionRole,
    traffic_percent: u8,
    health_status: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    local_replicas: Vec<SoracloudRuntimeReplicaPlan>,
) -> iroha_core::soracloud_runtime::SoracloudRuntimeServicePlan {
    iroha_core::soracloud_runtime::SoracloudRuntimeServicePlan {
        service_name: service_name.to_owned(),
        service_version: service_version.to_owned(),
        role,
        traffic_percent,
        runtime: iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou,
        execution_plane: iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService,
        bundle_hash: materialized_bundle_hash,
        bundle_path: format!("/bundles/{service_version}.to"),
        entrypoint: "/runtime/bin/launch.sh".to_owned(),
        inrou: None,
        bundle_cache_path: materialization_dir
            .join("bundle.tar.gz")
            .display()
            .to_string(),
        bundle_available_locally: true,
        process_generation: Some(1),
        desired_replica_count: 2,
        local_replica_slots: local_replicas
            .iter()
            .map(|replica| replica.replica_slot)
            .collect(),
        local_replicas,
        health_status,
        load_factor_bps: 0,
        authoritative_pending_mailbox_messages: 0,
        rollout_handle: Some("rollout-2026-03".to_owned()),
        config_generation: 0,
        secret_generation: 0,
        quota_class: Some("taira-open".to_owned()),
        service_lease_status: Some(iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active),
        lease_expires_height: Some(100),
        remaining_runtime_balance: Some("50".parse().expect("runtime balance")),
        config_entry_count: 0,
        secret_entry_count: 0,
        config_exports: Vec::new(),
        supports_host_read_config: true,
        supports_host_read_secret_envelope: true,
        materialization_dir: materialization_dir.display().to_string(),
        config_materialization_dir: materialization_dir.join("configs").display().to_string(),
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
        lease_volumes: Vec::new(),
        mailboxes: Vec::new(),
        artifacts: Vec::new(),
    }
}
fn hosted_http_runtime_replica_plan(
    materialization_dir: &Path,
    replica_slot: u16,
    health_status: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    listen_base_url: Option<&str>,
    pid: Option<u32>,
) -> SoracloudRuntimeReplicaPlan {
    SoracloudRuntimeReplicaPlan {
        replica_slot,
        lease_started_height: 1,
        placement_incarnation: Hash::new(Encode::encode(&("placement", replica_slot))).to_string(),
        host_availability:
            iroha_data_model::soracloud::SoraInrouReplicaHostAvailabilityV1::Available,
        validator_account_id: ALICE_ID.to_string(),
        peer_id: PeerId::from(iroha_test_samples::ALICE_KEYPAIR.public_key().clone()).to_string(),
        materialization_dir: materialization_dir
            .join("replicas")
            .join(format!("replica-{replica_slot:04}"))
            .display()
            .to_string(),
        health_status,
        listen_base_url: listen_base_url.map(ToOwned::to_owned),
        pid,
        last_error: None,
    }
}
fn test_inrou_manifest() -> iroha_data_model::soracloud::SoraInrouManifestV1 {
    iroha_data_model::soracloud::SoraInrouManifestV1 {
        schema_version: iroha_data_model::soracloud::SORA_INROU_MANIFEST_VERSION_V1,
        guest_images: std::collections::BTreeMap::from([
            (
                iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                    rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                    initrd_image_path: None,
                    published_artifact:
                        iroha_data_model::soracloud::SoraPublishedInrouGuestImageArtifactV1 {
                            manifest_digest_hex: "31".repeat(32),
                            content_cid:
                                "bafyr6ibrgeytcmjrgeytcmjrgeytcmjrgeytcmjrgeytcmjrgeytcmjrge"
                                    .to_owned(),
                        },
                },
            ),
            (
                iroha_data_model::soracloud::SoraInrouGuestIsaV1::Aarch64,
                iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/aarch64/vmlinux".to_owned(),
                    rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_owned(),
                    initrd_image_path: None,
                    published_artifact:
                        iroha_data_model::soracloud::SoraPublishedInrouGuestImageArtifactV1 {
                            manifest_digest_hex: "32".repeat(32),
                            content_cid:
                                "bafyr6ibsgizdemrsgizdemrsgizdemrsgizdemrsgizdemrsgizdemrsgi"
                                    .to_owned(),
                        },
                },
            ),
        ]),
    }
}
fn seed_authoritative_hosted_http_revision(
    world: &mut World,
    bundle: &iroha_data_model::soracloud::SoraDeploymentBundleV1,
    desired_replica_count: u16,
    assignments: &[(
        u16,
        AccountId,
        String,
        iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    )],
) {
    for (_, validator_account_id, peer_id, _) in assignments {
        let canonical_peer_id =
            PeerId::from(validator_account_id.expect_single_signatory().clone()).to_string();
        assert_eq!(
            peer_id, &canonical_peer_id,
            "positive hosted HTTP fixtures must bind each validator account to its canonical peer"
        );
        let validator_peer_id = peer_id
            .parse::<PeerId>()
            .expect("hosted HTTP assignment peer id must be valid");
        world.public_lane_validators_mut_for_testing().insert(
            (
                iroha_data_model::nexus::LaneId::SINGLE,
                validator_account_id.clone(),
            ),
            iroha_data_model::nexus::staking::PublicLaneValidatorRecord {
                lane_id: iroha_data_model::nexus::LaneId::SINGLE,
                validator: validator_account_id.clone(),
                peer_id: validator_peer_id,
                stake_account: validator_account_id.clone(),
                total_stake: iroha_primitives::numeric::Quantity::from(1_u64),
                self_stake: iroha_primitives::numeric::Quantity::from(1_u64),
                metadata: iroha_data_model::metadata::Metadata::default(),
                status: iroha_data_model::nexus::staking::PublicLaneValidatorStatus::Active,
                activation_epoch: Some(0),
                activation_height: Some(0),
                last_reward_epoch: None,
            },
        );
        let capability = iroha_data_model::soracloud::SoraInrouHostCapabilityRecordV1 {
            schema_version:
                iroha_data_model::soracloud::SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
            validator_account_id: validator_account_id.clone(),
            peer_id: peer_id.clone(),
            supported_guest_isas: std::collections::BTreeSet::from([
                iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
            ]),
            max_hosted_replica_capacity:
                iroha_data_model::soracloud::SORA_INROU_HOSTED_REPLICA_CAPACITY_V1,
            max_cpu_millis: u32::MAX,
            max_memory_bytes: u64::MAX,
            max_storage_bytes: u64::MAX,
            geography_tags: Default::default(),
            observed_latency_ms: None,
            advertised_at_ms: 1,
            heartbeat_expires_at_ms: u64::MAX,
        };
        capability
            .validate()
            .expect("positive hosted HTTP host capability must be production-valid");
        world
            .soracloud_inrou_host_capabilities_mut_for_testing()
            .insert(validator_account_id.clone(), capability);
    }
    let placements = assignments
        .iter()
        .map(
            |(replica_slot, validator_account_id, peer_id, _health_status)| {
                iroha_data_model::soracloud::SoraInrouReplicaPlacementV1 {
                    replica_slot: *replica_slot,
                    economic_clock:
                        iroha_data_model::soracloud::SoraServiceLeaseClockV1::CanonicalBlockHeight,
                    lease_started_height: 1,
                    placement_incarnation: iroha_crypto::Hash::new(Encode::encode(&(
                        "placement",
                        *replica_slot,
                    ))),
                    host_availability:
                        iroha_data_model::soracloud::SoraInrouReplicaHostAvailabilityV1::Available,
                    validator_account_id: validator_account_id.clone(),
                    peer_id: peer_id.clone(),
                    selected_guest_isa: iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                }
            },
        )
        .collect::<Vec<_>>();
    let placement_record = iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
        schema_version: iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
        service_name: bundle.service.service_name.clone(),
        service_version: bundle.service.service_version.clone(),
        desired_replica_count,
        eligible_validator_count: u32::try_from(assignments.len()).unwrap_or(u32::MAX),
        placements: placements.clone(),
        reconciled_at_ms: 1,
        last_error: (placements.len() < usize::from(desired_replica_count)).then(|| {
            format!(
                "placed {} of {desired_replica_count} replicas using {} eligible validators",
                placements.len(),
                assignments.len()
            )
        }),
    };
    placement_record
        .validate()
        .expect("positive hosted HTTP placement must be production-valid");
    world
        .soracloud_inrou_service_placements_mut_for_testing()
        .insert(
            (
                bundle.service.service_name.to_string(),
                bundle.service.service_version.clone(),
            ),
            placement_record,
        );
    for ((replica_slot, validator_account_id, peer_id, health_status), placement) in
        assignments.iter().zip(placements.iter())
    {
        let runtime_state = iroha_data_model::soracloud::SoraInrouReplicaRuntimeStateV1 {
            schema_version:
                iroha_data_model::soracloud::SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
            service_name: bundle.service.service_name.clone(),
            service_version: bundle.service.service_version.clone(),
            replica_slot: *replica_slot,
            placement_incarnation: placement.placement_incarnation,
            validator_account_id: validator_account_id.clone(),
            peer_id: peer_id.clone(),
            selected_guest_isa: placement.selected_guest_isa,
            health_status: *health_status,
            load_factor_bps: 0,
            materialized_bundle_hash: bundle.container.bundle_hash,
            reporting_epoch: 1,
            accounted_egress_bytes: 0,
            updated_at_ms: 1,
            last_error: None,
        };
        runtime_state
            .validate()
            .expect("positive hosted HTTP runtime state must be production-valid");
        world
            .soracloud_inrou_replica_runtime_mut_for_testing()
            .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                    replica_slot.to_string(),
                ),
                runtime_state,
            );
    }
}
fn seed_public_hosted_http_rollout_app(
    temp: &tempfile::TempDir,
    baseline_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    candidate_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
) -> SharedAppState {
    seed_public_hosted_http_rollout_app_with_service_lease(
        temp,
        baseline_health,
        candidate_health,
        Some(hosted_http_service_lease_state(
            iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
            "50".parse().expect("runtime balance"),
            100,
        )),
    )
}
fn seed_public_hosted_http_rollout_app_with_service_lease(
    temp: &tempfile::TempDir,
    baseline_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    candidate_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    service_lease: Option<iroha_data_model::soracloud::SoraServiceLeaseStateV1>,
) -> SharedAppState {
    seed_public_hosted_http_rollout_app_with_replica_plans_and_snapshot_peer_id(
        temp,
        baseline_health,
        candidate_health,
        vec![hosted_http_runtime_replica_plan(
            &temp.path().join("service-baseline"),
            1,
            baseline_health,
            Some("http://127.0.0.1:18080"),
            Some(101),
        )],
        vec![hosted_http_runtime_replica_plan(
            &temp.path().join("service-canary"),
            1,
            candidate_health,
            None,
            None,
        )],
        Some(hosted_http_rollout_local_peer_id().to_string()),
        service_lease,
    )
}
fn seed_public_hosted_http_rollout_app_with_replica_plans(
    temp: &tempfile::TempDir,
    baseline_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    candidate_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    baseline_replica_plans: Vec<SoracloudRuntimeReplicaPlan>,
    candidate_replica_plans: Vec<SoracloudRuntimeReplicaPlan>,
) -> SharedAppState {
    seed_public_hosted_http_rollout_app_with_replica_plans_and_snapshot_peer_id(
        temp,
        baseline_health,
        candidate_health,
        baseline_replica_plans,
        candidate_replica_plans,
        Some(hosted_http_rollout_local_peer_id().to_string()),
        Some(hosted_http_service_lease_state(
            iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
            "50".parse().expect("runtime balance"),
            100,
        )),
    )
}
fn hosted_http_service_lease_state(
    status: iroha_data_model::soracloud::SoraServiceLeaseStatusV1,
    prepaid_runtime_balance: Quantity,
    lease_expires_height: u64,
) -> iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
    iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
        economic_clock: iroha_data_model::soracloud::SoraServiceLeaseClockV1::CanonicalBlockHeight,
        status,
        quota_class: "taira-open".to_owned(),
        replica_count: std::num::NonZeroU16::new(1).expect("nonzero"),
        deployment_deposit: "1".parse().expect("deployment deposit"),
        prepaid_runtime_balance,
        runtime_price_per_block: "0.00025".parse().expect("runtime price"),
        storage_price_per_gib_block: "0.000025".parse().expect("storage price"),
        egress_price_per_mib: "0.000005".parse().expect("egress price"),
        lease_started_height: 1,
        lease_expires_height,
        reporting_epoch: 1,
        settled_egress_bytes: 0,
        egress_reporter_checkpoints: Vec::new(),
        accounted_egress_bytes: 0,
        last_status_reason: None,
    }
}
fn checked_torii_test_inrou_host_identity(seed: u8, context: &'static str) -> (AccountId, PeerId) {
    let key_pair = checked_torii_test_ed25519_keypair(seed, context);
    let public_key = key_pair.public_key().clone();
    (AccountId::new(public_key.clone()), PeerId::from(public_key))
}
fn seed_hosted_http_rollout_public_lane_validator(
    app: &SharedAppState,
    validator: &AccountId,
    peer_id: &PeerId,
) {
    let next_height = app
        .state
        .latest_block_header_fast()
        .map_or(1, |header| header.height().get().saturating_add(1));
    let header = BlockHeader::new(
        NonZeroU64::new(next_height).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    tx.world_mut_for_testing()
        .public_lane_validators_mut_for_testing()
        .insert(
            (iroha_data_model::nexus::LaneId::SINGLE, validator.clone()),
            iroha_data_model::nexus::PublicLaneValidatorRecord {
                lane_id: iroha_data_model::nexus::LaneId::SINGLE,
                validator: validator.clone(),
                peer_id: peer_id.clone(),
                stake_account: validator.clone(),
                total_stake: Quantity::from(1_u64),
                self_stake: Quantity::from(1_u64),
                metadata: iroha_data_model::metadata::Metadata::default(),
                status: iroha_data_model::nexus::PublicLaneValidatorStatus::Active,
                activation_epoch: None,
                activation_height: None,
                last_reward_epoch: None,
            },
        );
    tx.apply();
    block
        .commit()
        .expect("commit hosted-http public-lane validator fixture");
}
fn hosted_http_lease_volume_states(
    bundle: &iroha_data_model::soracloud::SoraDeploymentBundleV1,
    service_lease: Option<&iroha_data_model::soracloud::SoraServiceLeaseStateV1>,
) -> Vec<iroha_data_model::soracloud::SoraServiceLeaseVolumeStateV1> {
    let Some(service_lease) = service_lease else {
        return Vec::new();
    };
    bundle
        .service
        .lease_volumes
        .iter()
        .map(
            |volume| iroha_data_model::soracloud::SoraServiceLeaseVolumeStateV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_SERVICE_LEASE_VOLUME_STATE_VERSION_V1,
                volume_name: volume.volume_name.clone(),
                kind: volume.kind,
                storage_class: volume.storage_class,
                mount_path: volume.mount_path.clone(),
                max_total_bytes: volume.max_total_bytes.get(),
                lease_started_height: service_lease.lease_started_height,
                lease_expires_height: service_lease.lease_expires_height,
                authoritative_generation: 1,
            },
        )
        .collect()
}
fn hosted_http_rollout_local_identity() -> (AccountId, PeerId) {
    checked_torii_test_inrou_host_identity(
        0x3d,
        "derive canonical hosted-http rollout local host fixture key",
    )
}
fn hosted_http_rollout_local_peer_id() -> PeerId {
    hosted_http_rollout_local_identity().1
}
fn seed_public_hosted_http_rollout_app_with_replica_plans_and_snapshot_peer_id(
    temp: &tempfile::TempDir,
    baseline_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    candidate_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    baseline_replica_plans: Vec<SoracloudRuntimeReplicaPlan>,
    candidate_replica_plans: Vec<SoracloudRuntimeReplicaPlan>,

    snapshot_local_peer_id: Option<String>,
    service_lease: Option<iroha_data_model::soracloud::SoraServiceLeaseStateV1>,
) -> SharedAppState {
    let mut world = seed_public_soracloud_world();
    let service_name = "web_portal";
    let baseline_version = "2026.02.0";
    let candidate_version = "2026.03.0";
    let mut baseline_bundle = world
        .view()
        .soracloud_service_revisions()
        .get(&(service_name.to_owned(), baseline_version.to_owned()))
        .cloned()
        .expect("public service bundle");
    baseline_bundle.container.runtime = iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou;
    baseline_bundle.container.inrou = Some(test_inrou_manifest());
    baseline_bundle.container.entrypoint = "/app/main".to_owned();
    baseline_bundle.service.execution_plane =
        iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
    baseline_bundle.service.replicas = std::num::NonZeroU16::new(2).expect("replicas");
    baseline_bundle.service.state_bindings.clear();
    baseline_bundle.service.handlers.clear();
    baseline_bundle.service.artifacts.clear();
    baseline_bundle.service.lease_volumes = vec![
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
            mount_path: "/var/lib/soracloud/volumes/index_state".to_owned(),
            max_total_bytes: std::num::NonZeroU64::new(1024 * 1024).expect("bytes"),
        },
    ];
    baseline_bundle.service.container.manifest_hash = baseline_bundle.container_manifest_hash();
    baseline_bundle
        .validate_for_admission()
        .expect("baseline hosted HTTP Inrou fixture must pass production validation");
    world.soracloud_service_revisions_mut_for_testing().insert(
        (service_name.to_owned(), baseline_version.to_owned()),
        baseline_bundle.clone(),
    );
    let mut candidate_bundle = baseline_bundle.clone();
    candidate_bundle.service.service_version = candidate_version.to_owned();
    candidate_bundle.container.bundle_hash = Hash::new(b"hosted-http-canary-bundle");
    candidate_bundle.container.bundle_path = "/bundles/public-canary.to".to_owned();
    candidate_bundle.service.container.manifest_hash = candidate_bundle.container_manifest_hash();
    candidate_bundle
        .validate_for_admission()
        .expect("candidate hosted HTTP Inrou fixture must pass production validation");
    world.soracloud_service_revisions_mut_for_testing().insert(
        (service_name.to_owned(), candidate_version.to_owned()),
        candidate_bundle.clone(),
    );
    let lease_volume_states =
        hosted_http_lease_volume_states(&candidate_bundle, service_lease.as_ref());
    let rollout = iroha_data_model::soracloud::SoraServiceRolloutStateV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: "rollout-2026-03".to_owned(),
        baseline_version: baseline_version.to_owned(),
        candidate_version: candidate_version.to_owned(),
        canary_percent: 20,
        traffic_percent: 20,
        stage: iroha_data_model::soracloud::SoraRolloutStageV1::Canary,
        health_failures: 0,
        max_health_failures: 3,
        health_window_secs: 60,
        created_sequence: 1,
        updated_sequence: 1,
    };
    let deployment = iroha_data_model::soracloud::SoraServiceDeploymentStateV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
        service_name: service_name.parse().expect("service"),
        current_service_version: candidate_version.to_owned(),
        current_service_manifest_hash: candidate_bundle.service_manifest_hash(),
        current_container_manifest_hash: candidate_bundle.container_manifest_hash(),
        revision_count: 2,
        process_generation: 1,
        process_started_sequence: 1,
        active_rollout: Some(rollout.clone()),
        last_rollout: Some(rollout),
        config_generation: 0,
        secret_generation: 0,
        service_configs: BTreeMap::new(),
        service_secrets: BTreeMap::new(),
        fhe_policy_records: BTreeMap::new(),
        service_lease,
        lease_volume_states,
    };
    deployment
        .validate()
        .expect("hosted HTTP rollout deployment must be production-valid");
    iroha_core::soracloud_runtime::validate_soracloud_deployment_lease_volume_bindings(
        &deployment,
        &candidate_bundle,
    )
    .expect("hosted HTTP rollout deployment must exactly match admitted lease-volume economics");
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(service_name.parse().expect("service"), deployment);

    let (local_validator_account_id, local_peer_id) = hosted_http_rollout_local_identity();
    let local_peer_id_string = local_peer_id.to_string();
    let mut local_host_available = true;
    let mut assignments_for = |replicas: &[SoracloudRuntimeReplicaPlan],
                               remote_seed_base: u8,
                               remote_context: &'static str| {
        replicas
            .iter()
            .enumerate()
            .map(|(index, replica)| {
                let (validator_account_id, peer_id) = if local_host_available {
                    local_host_available = false;
                    (local_validator_account_id.clone(), local_peer_id.clone())
                } else {
                    let seed_offset =
                        u8::try_from(index).expect("hosted HTTP fixture replica index must fit u8");
                    let seed = remote_seed_base
                        .checked_add(seed_offset)
                        .expect("hosted HTTP fixture identity seed must not overflow");
                    checked_torii_test_inrou_host_identity(seed, remote_context)
                };
                (
                    replica.replica_slot,
                    validator_account_id,
                    peer_id.to_string(),
                    replica.health_status,
                )
            })
            .collect::<Vec<_>>()
    };
    let baseline_assignments = assignments_for(
        &baseline_replica_plans,
        0x40,
        "derive canonical hosted-http rollout baseline remote host fixture key",
    );
    let candidate_assignments = assignments_for(
        &candidate_replica_plans,
        0x60,
        "derive canonical hosted-http rollout candidate remote host fixture key",
    );
    let assignment_count = baseline_assignments.len() + candidate_assignments.len();
    let distinct_validator_count = baseline_assignments
        .iter()
        .chain(candidate_assignments.iter())
        .map(|assignment| assignment.1.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_peer_count = baseline_assignments
        .iter()
        .chain(candidate_assignments.iter())
        .map(|assignment| assignment.2.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    assert_eq!(
        (distinct_validator_count, distinct_peer_count),
        (assignment_count, assignment_count),
        "first-release rollout fixtures must use one distinct canonical host per active assignment"
    );
    for (assignments, replica_plans) in [
        (
            baseline_assignments.as_slice(),
            baseline_replica_plans.as_slice(),
        ),
        (
            candidate_assignments.as_slice(),
            candidate_replica_plans.as_slice(),
        ),
    ] {
        for (assignment, replica_plan) in assignments.iter().zip(replica_plans) {
            if assignment.2.as_str() != local_peer_id_string.as_str() {
                assert!(
                    replica_plan.listen_base_url.is_none() && replica_plan.pid.is_none(),
                    "remote hosted HTTP assignments must not expose a local listener or process"
                );
            }
        }
    }
    let baseline_local_replicas = baseline_replica_plans
        .into_iter()
        .filter(|replica| {
            baseline_assignments.iter().any(|assignment| {
                assignment.0 == replica.replica_slot
                    && assignment.2.as_str() == local_peer_id_string.as_str()
            })
        })
        .collect::<Vec<_>>();
    let candidate_local_replicas = candidate_replica_plans
        .into_iter()
        .filter(|replica| {
            candidate_assignments.iter().any(|assignment| {
                assignment.0 == replica.replica_slot
                    && assignment.2.as_str() == local_peer_id_string.as_str()
            })
        })
        .collect::<Vec<_>>();
    assert!(
        baseline_local_replicas.len() + candidate_local_replicas.len() <= 1,
        "only the assignment owned by the local peer may expose local runtime state"
    );
    seed_authoritative_hosted_http_revision(
        &mut world,
        &baseline_bundle,
        baseline_bundle.service.replicas.get(),
        &baseline_assignments,
    );
    seed_authoritative_hosted_http_revision(
        &mut world,
        &candidate_bundle,
        candidate_bundle.service.replicas.get(),
        &candidate_assignments,
    );
    let baseline_dir = temp.path().join("service-baseline");
    let mut snapshot = iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default();
    snapshot.local_peer_id = snapshot_local_peer_id;
    snapshot.services.insert(
        service_name.to_owned(),
        BTreeMap::from([
            (
                baseline_version.to_owned(),
                hosted_http_runtime_plan(
                    &baseline_dir,
                    service_name,
                    baseline_version,
                    baseline_bundle.container.bundle_hash.to_string(),
                    iroha_core::soracloud_runtime::SoracloudRuntimeRevisionRole::Active,
                    80,
                    baseline_health,
                    baseline_local_replicas,
                ),
            ),
            (
                candidate_version.to_owned(),
                hosted_http_runtime_plan(
                    &candidate_dir,
                    service_name,
                    candidate_version,
                    candidate_bundle.container.bundle_hash.to_string(),
                    iroha_core::soracloud_runtime::SoracloudRuntimeRevisionRole::CanaryCandidate,
                    20,
                    candidate_health,
                    candidate_local_replicas,
                ),
            ),
        ]),
    );
    let runtime = TestLocalReadRuntime {
        snapshot,
        state_dir: temp.path().to_path_buf(),
        local_peer_id: Some(local_peer_id.to_string()),
        result: Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "hosted-http rollout tests should not execute local reads",
            ),
        ),
        captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut app = mk_app_state_for_tests_with_world(world);
    seed_hosted_http_rollout_public_lane_validator(
        &app,
        &local_validator_account_id,
        &local_peer_id,
    );
    let app_mut = Arc::get_mut(&mut app).expect("unique app state");
    app_mut.local_peer_id = Some(local_peer_id);
    app_mut.soracloud_runtime = Some(Arc::new(runtime));
    app
}
fn hosted_http_baseline_replica_test_ip<P>(
    service_name: &str,
    service_version: &str,
    method: &HttpMethod,
    uri: &axum::http::Uri,
    predicate: P,
) -> IpAddr
where
    P: Fn(usize) -> bool,
{
    for octet in 1..=254 {
        let ip = IpAddr::from([198, 51, 100, octet]);
        let digest = super::hosted_http_request_hash(
            "soracloud:hosted-http-replica:v1",
            service_name,
            Some(service_version),
            Some(ip),
            method,
            uri,
        );
        let bucket = usize::from(u16::from_le_bytes([digest[0], digest[1]]));
        if predicate(bucket) {
            return ip;
        }
    }
    panic!("failed to find a replica bucket match for hosted-http routing test");
}
#[derive(Clone, Copy)]
enum PublicLocalReadRouteCase {
    Direct,
    TairaMonHost,
}
struct PublicLocalReadRouteSpec {
    state_dir: &'static str,
    response_bytes: &'static [u8],
    content_type: &'static str,
    cache_control: Option<&'static str>,
    artifact_hash_seed: Option<&'static [u8]>,
    result_seed: &'static [u8],
    certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1,
    bound_alias: Option<&'static str>,
    request_uri: &'static str,
    request_host: &'static str,
}
fn public_local_read_route_spec(case: PublicLocalReadRouteCase) -> PublicLocalReadRouteSpec {
    use iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1;
    let mut spec = PublicLocalReadRouteSpec {
        state_dir: "/tmp/test-soracloud-runtime",
        response_bytes: b"asset-body",
        content_type: "text/plain; charset=utf-8",
        cache_control: Some("public, max-age=60"),
        artifact_hash_seed: Some(b"asset-hash"),
        result_seed: b"result",
        certified_by: SoraCertifiedResponsePolicyV1::StateCommitment,
        bound_alias: None,
        request_uri: "/app/assets",
        request_host: "portal.sora",
    };
    match case {
        PublicLocalReadRouteCase::Direct => {}
        PublicLocalReadRouteCase::TairaMonHost => {
            spec.state_dir = "/tmp/test-taira-mon-public-runtime";
            spec.response_bytes = b"mon-asset-body";
            spec.artifact_hash_seed = Some(b"mon-asset-hash");
            spec.result_seed = b"mon-result";
            spec.bound_alias = Some("portal.sora");
            spec.request_uri = "/app/assets?fresh=1";
            spec.request_host = "portal.sora.mon.taira.sora.net:443";
        }
    }
    assert_eq!(
        spec.bound_alias.is_some(),
        !matches!(case, PublicLocalReadRouteCase::Direct),
        "public local-read fixture must keep direct and aliased ingress distinct",
    );
    spec
}
async fn run_public_local_read_route_case(case: PublicLocalReadRouteCase) {
    use tower::ServiceExt as _;
    let spec = public_local_read_route_spec(case);
    let world = seed_public_soracloud_world();
    let bindings = match spec.artifact_hash_seed {
        Some(seed) => vec![iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
            binding_name: None,
            state_key: None,
            payload_commitment: None,
            artifact_hash: Some(Hash::new(seed)),
        }],
        None => Vec::new(),
    };
    let cache_control = match spec.cache_control {
        Some(value) => Some(value.to_owned()),
        None => None,
    };
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestLocalReadRuntime::with_result(
        iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
        PathBuf::from(spec.state_dir),
        None,
        Ok(iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
            response_bytes: spec.response_bytes.to_vec(),
            content_type: Some(spec.content_type.to_owned()),
            content_encoding: None,
            cache_control,
            bindings,
            result_commitment: Hash::new(spec.result_seed),
            certified_by: spec.certified_by,
            runtime_receipt: None,
        }),
    )
    .capturing_requests(Arc::clone(&captured_requests));
    let mut app = mk_app_state_for_tests_with_world(world);
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(runtime));
    if let Some(alias) = spec.bound_alias {
        bind_domain_name_for_test(&app, alias);
    }
    let router = axum::Router::new()
        .fallback(any(handler_soracloud_public_local_read))
        .with_state(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri(spec.request_uri)
                .header(axum::http::header::HOST, spec.request_host)
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    if matches!(case, PublicLocalReadRouteCase::Direct) {
        assert_eq!(
            torii_response_header(&response, "x-iroha-soracloud-certified-by"),
            Some("state_commitment")
        );
    }
    let body = torii_body_bytes(response, "body").await;
    assert_eq!(body.as_ref(), spec.response_bytes);
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    match case {
        PublicLocalReadRouteCase::Direct => {
            assert_eq!(captured[0].service_name, "web_portal");
            assert_eq!(captured[0].handler_name, "assets");
            assert_eq!(captured[0].request_path, "/app/assets");
            assert_eq!(captured[0].handler_path, "/");
        }
        PublicLocalReadRouteCase::TairaMonHost => {
            assert_eq!(captured[0].request_path, "/app/assets");
            assert_eq!(captured[0].request_query.as_deref(), Some("fresh=1"));
            assert_eq!(
                captured[0].request_headers.get("host").map(String::as_str),
                Some("portal.sora")
            );
        }
    }
}
#[tokio::test]
async fn soracloud_public_local_read_route_invokes_runtime_with_authoritative_context() {
    run_public_local_read_route_case(PublicLocalReadRouteCase::Direct).await;
}
#[tokio::test]
async fn taira_mon_gateway_host_routes_local_read_requests() {
    run_public_local_read_route_case(PublicLocalReadRouteCase::TairaMonHost).await;
}
#[tokio::test]
async fn path_encoded_soradns_alias_is_not_routed() {
    use tower::ServiceExt as _;
    let app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    bind_domain_name_for_test(&app, "portal.sora");
    let router = axum::Router::new()
        .fallback(any(handler_soracloud_public_local_read))
        .with_state(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/soradns/portal.sora/app/assets")
                .header(axum::http::header::HOST, "taira.sora.org")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
#[derive(Clone, Copy)]
enum TravelSplitVaultMode {
    LocalRead,
    OrderedMailbox,
}
struct TravelSplitTopologyFixture {
    world: World,
    snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot,
    temp: tempfile::TempDir,
    live_peer_id: PeerId,
    upstream_task: tokio::task::JoinHandle<()>,
}
async fn travel_split_topology_fixture(mode: TravelSplitVaultMode) -> TravelSplitTopologyFixture {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let listen_base_url = format!("http://{}", listener.local_addr().expect("upstream addr"));
    let upstream = axum::Router::new().route(
        "/search",
        get(|| async {
            Response::builder()
                .status(StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(br#"{"source":"live"}"#.to_vec()))
                .expect("upstream response")
        }),
    );
    let upstream_task = tokio::spawn(async move {
        axum::serve(listener, upstream.into_make_service())
            .await
            .expect("serve upstream");
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let temp = tempfile::tempdir().expect("tempdir");
    let live_materialization_dir = temp.path().join("travel-ops-live");
    let mut world = seed_public_soracloud_world();
    let seed_bundle = world
        .view()
        .soracloud_service_revisions()
        .get(&("web_portal".to_owned(), "2026.02.0".to_owned()))
        .cloned()
        .expect("seed bundle");
    let (live_hash, live_path, vault_hash, vault_path): (&[u8], _, &[u8], _) = match mode {
        TravelSplitVaultMode::LocalRead => (
            b"travel-ops-live-bundle",
            "/bundles/travel-ops-live.to",
            b"travel-ops-vault-bundle",
            "/bundles/travel-ops-vault.to",
        ),
        TravelSplitVaultMode::OrderedMailbox => (
            b"travel-ops-live-update-bundle",
            "/bundles/travel-ops-live-update.to",
            b"travel-ops-vault-update-bundle",
            "/bundles/travel-ops-vault-update.to",
        ),
    };
    let mut live_bundle = seed_bundle.clone();
    live_bundle.service.service_name = "travel_ops_live".parse().expect("service");
    live_bundle.service.service_version = "2026.04.0".to_owned();
    live_bundle.container.runtime = iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou;
    live_bundle.container.bundle_hash = Hash::new(live_hash);
    live_bundle.container.bundle_path = live_path.to_owned();
    live_bundle.container.entrypoint = "/runtime/bin/launch.sh".to_owned();
    live_bundle.container.inrou = Some(test_inrou_manifest());
    live_bundle.service.execution_plane =
        iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
    live_bundle.service.route = Some(iroha_data_model::soracloud::SoraRouteTargetV1 {
        host: "travel.sora".to_owned(),
        path_prefix: "/api/v1".to_owned(),
        service_port: std::num::NonZeroU16::new(8787).expect("port"),
        visibility: iroha_data_model::soracloud::SoraRouteVisibilityV1::Public,
        tls_mode: iroha_data_model::soracloud::SoraTlsModeV1::Required,
    });
    live_bundle.service.handlers.clear();
    live_bundle.service.state_bindings.clear();
    live_bundle.service.artifacts.clear();
    live_bundle.service.lease_volumes = vec![
        iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("volume"),
            kind: iroha_data_model::soracloud::SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: std::num::NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("bytes"),
        },
        iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
            volume_name: "service_state".parse().expect("volume"),
            kind: iroha_data_model::soracloud::SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
            mount_path: "/var/lib/soracloud/service".to_owned(),
            max_total_bytes: std::num::NonZeroU64::new(1024 * 1024).expect("bytes"),
        },
    ];
    live_bundle.service.container.manifest_hash = live_bundle.container_manifest_hash();
    live_bundle
        .validate_for_admission()
        .expect("hosted live Inrou fixture must pass production validation");

    let mut vault_bundle = seed_bundle;
    vault_bundle.service.service_name = "travel_ops_vault".parse().expect("service");
    vault_bundle.service.service_version = "2026.04.0".to_owned();
    vault_bundle.container.bundle_hash = Hash::new(vault_hash);
    vault_bundle.container.bundle_path = vault_path.to_owned();
    vault_bundle.service.route = Some(iroha_data_model::soracloud::SoraRouteTargetV1 {
        host: "travel.sora".to_owned(),
        path_prefix: "/api".to_owned(),
        service_port: std::num::NonZeroU16::new(8788).expect("port"),
        visibility: iroha_data_model::soracloud::SoraRouteVisibilityV1::Public,
        tls_mode: iroha_data_model::soracloud::SoraTlsModeV1::Required,
    });
    vault_bundle.service.handlers = vec![match mode {
        TravelSplitVaultMode::LocalRead => iroha_data_model::soracloud::SoraServiceHandlerV1 {
            handler_name: "auth_me".parse().expect("handler"),
            class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query,
            entrypoint: "serve_auth_me".to_owned(),
            route_path: Some("/auth/me".to_owned()),
            certified_response:
                iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::AuditReceipt,
            mailbox: None,
        },
        TravelSplitVaultMode::OrderedMailbox => iroha_data_model::soracloud::SoraServiceHandlerV1 {
            handler_name: "preferences_put".parse().expect("handler"),
            class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update,
            entrypoint: "store_user_preferences".to_owned(),
            route_path: Some("/v1/user/preferences".to_owned()),
            certified_response: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
            mailbox: Some(iroha_data_model::soracloud::SoraMailboxContractV1 {
                queue_name: "user_updates".parse().expect("queue"),
                max_pending_messages: std::num::NonZeroU32::new(128).expect("pending"),
                max_message_bytes: std::num::NonZeroU64::new(131_072).expect("bytes"),
                retention_blocks: std::num::NonZeroU32::new(64).expect("retention"),
            }),
        },
    }];
    vault_bundle.service.artifacts.clear();
    vault_bundle.service.container.manifest_hash = vault_bundle.container_manifest_hash();
    vault_bundle
        .validate_for_admission()
        .expect("deterministic vault fixture must pass production validation");

    for bundle in [live_bundle.clone(), vault_bundle] {
        let service_name = bundle.service.service_name.clone();
        world.soracloud_service_revisions_mut_for_testing().insert(
            (
                bundle.service.service_name.to_string(),
                bundle.service.service_version.clone(),
            ),
            bundle.clone(),
        );
        let service_lease = (bundle.service.execution_plane
            == iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService)
            .then(|| {
                hosted_http_service_lease_state(
                    iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
                    "50".parse().expect("runtime balance"),
                    100,
                )
            });
        let lease_volume_states = hosted_http_lease_volume_states(&bundle, service_lease.as_ref());
        let deployment = iroha_data_model::soracloud::SoraServiceDeploymentStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
            service_name: bundle.service.service_name.clone(),
            current_service_version: bundle.service.service_version.clone(),
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
            service_lease,
            lease_volume_states,
        };
        deployment
            .validate()
            .expect("travel split deployment must be production-valid");
        iroha_core::soracloud_runtime::validate_soracloud_deployment_lease_volume_bindings(
            &deployment,
            &bundle,
        )
        .expect("travel split deployment must exactly match admitted lease-volume economics");
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(service_name, deployment);
    }

    let (host_seed, host_context) = match mode {
        TravelSplitVaultMode::LocalRead => (
            0x45,
            "derive canonical hosted live/local split host fixture key",
        ),
        TravelSplitVaultMode::OrderedMailbox => (
            0x47,
            "derive canonical hosted live/mailbox split host fixture key",
        ),
    };
    let (live_validator_account_id, live_peer_id) =
        checked_torii_test_inrou_host_identity(host_seed, host_context);
    seed_authoritative_hosted_http_revision(
        &mut world,
        &live_bundle,
        live_bundle.service.replicas.get(),
        &[(
            1,
            live_validator_account_id,
            live_peer_id.to_string(),
            iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        )],
    );
    let mut snapshot = iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default();
    snapshot.local_peer_id = Some(live_peer_id.to_string());
    snapshot.services.insert(
        "travel_ops_live".to_owned(),
        BTreeMap::from([(
            "2026.04.0".to_owned(),
            hosted_http_runtime_plan(
                &live_materialization_dir,
                "travel_ops_live",
                "2026.04.0",
                live_bundle.container.bundle_hash.to_string(),
                iroha_core::soracloud_runtime::SoracloudRuntimeRevisionRole::Active,
                100,
                iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
                vec![hosted_http_runtime_replica_plan(
                    &live_materialization_dir,
                    1,
                    iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
                    Some(&listen_base_url),
                    Some(1),
                )],
            ),
        )]),
    );
    TravelSplitTopologyFixture {
        world,
        snapshot,
        temp,
        live_peer_id,
        upstream_task,
    }
}
#[tokio::test]
async fn soracloud_public_split_app_routes_hosted_live_and_local_vault_on_one_node() {
    use tower::ServiceExt as _;
    let TravelSplitTopologyFixture {
        world,
        snapshot,
        temp,
        live_peer_id,
        upstream_task,
    } = travel_split_topology_fixture(TravelSplitVaultMode::LocalRead).await;
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestLocalReadRuntime {
        snapshot,
        state_dir: temp.path().to_path_buf(),
        local_peer_id: Some(live_peer_id.to_string()),
        result: Ok(iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
            response_bytes: br#"{"wallet":"alice"}"#.to_vec(),
            content_type: Some("application/json".to_owned()),
            content_encoding: None,
            cache_control: None,
            bindings: Vec::new(),
            result_commitment: Hash::new(b"vault-result"),
            certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
            runtime_receipt: None,
        }),
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
        "hosted live routes must bypass deterministic local-read execution"
    );
    let vault_response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/auth/me")
                .header(axum::http::header::HOST, "travel.sora")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("vault request"),
        )
        .await
        .expect("vault response");
    assert_eq!(vault_response.status(), StatusCode::OK);
    let vault_body = torii_body_bytes(vault_response, "vault body").await;
    assert_eq!(vault_body.as_ref(), br#"{"wallet":"alice"}"#);
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].service_name, "travel_ops_vault");
    assert_eq!(captured[0].service_version, "2026.04.0");
    assert_eq!(captured[0].handler_name, "auth_me");
    assert_eq!(
        captured[0].handler_class,
        iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query
    );
    assert_eq!(captured[0].request_method, "GET");
    assert_eq!(captured[0].request_path, "/api/auth/me");
    assert_eq!(captured[0].handler_path, "/");
    upstream_task.abort();
}
