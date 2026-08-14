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
    let recent_before = axum::body::to_bytes(recent_response.into_body(), usize::MAX)
        .await
        .expect("recent body");
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
    let recent_after = axum::body::to_bytes(recent_response.into_body(), usize::MAX)
        .await
        .expect("recent body after sidecar corruption");
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
    use iroha_data_model::query::{QueryRequest, SingularQueryBox, runtime::prelude::FindAbiVersion};
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
    captured_proxy_failures: Arc<
        std::sync::Mutex<
            Vec<(
                SoracloudLocalReadRequest,
                String,
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
            )>,
        >,
    >,
    captured_reconcile_requests: Arc<
        std::sync::Mutex<
            Vec<(
                SoracloudLocalReadRequest,
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
            )>,
        >,
    >,
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
    fn report_generated_hf_proxy_failure(
        &self,
        request: &SoracloudLocalReadRequest,
        target_peer_id: &str,
        error: &iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    ) {
        self.captured_proxy_failures
            .lock()
            .expect("proxy failure capture lock")
            .push((request.clone(), target_peer_id.to_owned(), error.clone()));
    }
    fn request_generated_hf_reconcile(
        &self,
        request: &SoracloudLocalReadRequest,
        error: &iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError,
    ) {
        self.captured_reconcile_requests
            .lock()
            .expect("reconcile capture lock")
            .push((request.clone(), error.clone()));
    }
    fn request_generated_hf_proxy_responder_reconcile(
        &self,
        request: &SoracloudLocalReadRequest,
        responder_peer_id: &str,
        expected_peer_id: &str,
    ) {
        self.captured_reconcile_requests
                .lock()
                .expect("reconcile capture lock")
                .push((
                    request.clone(),
                    iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                        SoracloudRuntimeExecutionErrorKind::Unavailable,
                        format!(
                            "unexpected proxy responder `{responder_peer_id}` answered request intended for authoritative primary `{expected_peer_id}`"
                        ),
                    ),
                ));
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
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test runtime does not implement mailbox execution",
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
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test runtime does not implement apartment execution",
            ),
        )
    }
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
#[derive(Clone)]
struct TestSoracloudRuntimeHandle {
    snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot,
    state_dir: PathBuf,
    local_peer_id: Option<String>,
}
impl iroha_core::soracloud_runtime::SoracloudRuntimeReadHandle for TestSoracloudRuntimeHandle {
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
impl iroha_core::soracloud_runtime::SoracloudRuntime for TestSoracloudRuntimeHandle {
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
                "test runtime handle does not implement local reads",
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
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test runtime handle does not implement mailbox execution",
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
        Err(
            iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::Unavailable,
                "test runtime handle does not implement apartment execution",
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
                reported_pending_mailbox_messages: 2,
                authoritative_pending_mailbox_messages: 3,
                rollout_handle: None,
                config_generation: 0,
                secret_generation: 0,
                quota_class: None,
                service_lease_status: None,
                lease_expires_sequence: None,
                remaining_runtime_balance: None,
                config_entry_count: 0,
                secret_entry_count: 0,
                config_exports: vec![],
                supports_host_read_config: true,
                supports_host_read_secret_envelope: true,
                supports_private_secret_payload_reads: false,
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
                secret_payload_materialization_dir:
                    "/tmp/soracloud/runtime/secrets/web_portal/1.0.0".to_string(),
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
            lease_expires_sequence: 90,
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
        hf_sources: std::collections::BTreeMap::new(),
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
                allow_wallet_signing: false,
                allow_state_writes: true,
                allow_model_inference: false,
                allow_model_training: false,
            },
            resources: iroha_data_model::soracloud::SoraResourceLimitsV1 {
                cpu_millis: std::num::NonZeroU32::new(500).expect("cpu"),
                memory_bytes: std::num::NonZeroU64::new(16 * 1024 * 1024).expect("memory"),
                ephemeral_storage_bytes: std::num::NonZeroU64::new(16 * 1024 * 1024)
                    .expect("storage"),
                max_open_files: std::num::NonZeroU32::new(256).expect("files"),
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
                    handler_name: "private_update".parse().expect("handler"),
                    class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::PrivateUpdate,
                    entrypoint: "apply_private_update".to_owned(),
                    route_path: Some("/private/update".to_owned()),
                    certified_response:
                        iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                    mailbox: Some(iroha_data_model::soracloud::SoraMailboxContractV1 {
                        queue_name: "private_updates".parse().expect("queue"),
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
fn seed_generated_hf_public_world(primary_peer_id: &str) -> (World, String, String) {
    use iroha_data_model::{
        asset::AssetDefinitionId,
        soracloud::{
            SORA_HF_PLACEMENT_RECORD_VERSION_V1, SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
            SORA_HF_SHARED_LEASE_POOL_VERSION_V1, SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
            SoraHfBackendFamilyV1, SoraHfModelFormatV1, SoraHfPlacementHostAssignmentV1,
            SoraHfPlacementHostRoleV1, SoraHfPlacementHostStatusV1, SoraHfPlacementRecordV1,
            SoraHfPlacementStatusV1, SoraHfResourceProfileV1, SoraHfSharedLeaseMemberStatusV1,
            SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1, SoraHfSharedLeaseStatusV1,
            SoraServiceDeploymentStateV1,
        },
        sorafs::pin_registry::StorageClass,
    };
    let mut world = World::new();
    let service_name: Name = "hf_router".parse().expect("service");
    let service_name_string = service_name.as_ref().to_owned();
    let source_id = Hash::new(b"generated-hf-source");
    let pool_id = Hash::new(b"generated-hf-pool");
    let bundle = iroha_core::soracloud_runtime::build_soracloud_hf_generated_service_bundle(
        service_name.clone(),
        &source_id.to_string(),
        "openai/gpt-oss",
        "main",
        "gpt-oss",
    );
    let service_version = bundle.service.service_version.clone();
    let member_account =
        checked_torii_test_account_id(0x39, "derive generated-HF member fixture key");
    let primary_validator =
        checked_torii_test_account_id(0x3a, "derive generated-HF primary validator fixture key");
    let replica_validator =
        checked_torii_test_account_id(0x3b, "derive generated-HF replica validator fixture key");
    let replica_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x3c, "derive generated-HF replica peer fixture key")
            .public_key()
            .clone(),
    )
    .to_string();
    world.soracloud_service_revisions_mut_for_testing().insert(
        (service_name_string.clone(), service_version.clone()),
        bundle.clone(),
    );
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(
            service_name,
            SoraServiceDeploymentStateV1 {
                schema_version: SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                service_name: bundle.service.service_name.clone(),
                current_service_version: service_version.clone(),
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
        .soracloud_hf_shared_lease_pools_mut_for_testing()
        .insert(
            pool_id,
            SoraHfSharedLeasePoolV1 {
                schema_version: SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
                pool_id,
                source_id,
                storage_class: StorageClass::Warm,
                lease_asset_definition_id: AssetDefinitionId::derive_from_components(
                    DomainId::try_new("wonderland", "universal").expect("domain"),
                    "xor".parse().expect("asset"),
                ),
                base_fee: "0.00001".parse().expect("base fee"),
                lease_term_ms: 60_000,
                window_started_at_ms: 1,
                window_expires_at_ms: 60_001,
                active_member_count: 1,
                status: SoraHfSharedLeaseStatusV1::Active,
                queued_next_window: None,
            },
        );
    world
        .soracloud_hf_shared_lease_members_mut_for_testing()
        .insert(
            (pool_id.to_string(), member_account.to_string()),
            SoraHfSharedLeaseMemberV1 {
                schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
                pool_id,
                source_id,
                account_id: member_account,
                status: SoraHfSharedLeaseMemberStatusV1::Active,
                joined_at_ms: 1,
                updated_at_ms: 1,
                total_paid: "0.00001".parse().expect("total paid"),
                total_refunded: Quantity::zero(),
                last_charge: "0.00001".parse().expect("last charge"),
                total_compute_paid: "0.000005".parse().expect("total compute paid"),
                total_compute_refunded: Quantity::zero(),
                last_compute_charge: "0.000005".parse().expect("last compute charge"),
                service_bindings: std::collections::BTreeSet::from([service_name_string.clone()]),
                apartment_bindings: std::collections::BTreeSet::new(),
            },
        );
    world.soracloud_hf_placements_mut_for_testing().insert(
        pool_id,
        SoraHfPlacementRecordV1 {
            schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
            placement_id: Hash::new(b"generated-hf-placement"),
            source_id,
            pool_id,
            status: SoraHfPlacementStatusV1::Ready,
            selection_seed_hash: Hash::new(b"generated-hf-seed"),
            resource_profile: SoraHfResourceProfileV1 {
                required_model_bytes: 1_024,
                backend_family: SoraHfBackendFamilyV1::Transformers,
                model_format: SoraHfModelFormatV1::Safetensors,
                disk_cache_bytes_floor: 2_048,
                ram_bytes_floor: 2_048,
                vram_bytes_floor: 0,
            },
            eligible_validator_count: 2,
            adaptive_target_host_count: 2,
            assigned_hosts: vec![
                SoraHfPlacementHostAssignmentV1 {
                    validator_account_id: primary_validator,
                    peer_id: primary_peer_id.to_owned(),
                    role: SoraHfPlacementHostRoleV1::Primary,
                    status: SoraHfPlacementHostStatusV1::Warm,
                    host_class: "gpu.large".to_owned(),
                },
                SoraHfPlacementHostAssignmentV1 {
                    validator_account_id: replica_validator,
                    peer_id: replica_peer_id,
                    role: SoraHfPlacementHostRoleV1::Replica,
                    status: SoraHfPlacementHostStatusV1::Warm,
                    host_class: "gpu.large".to_owned(),
                },
            ],
            total_reservation_fee: "0.000005".parse().expect("total reservation fee"),
            last_rebalance_at_ms: 1,
            last_error: None,
        },
    );
    (world, service_name_string, service_version)
}
fn hosted_http_runtime_plan(
    materialization_dir: &Path,
    service_name: &str,
    service_version: &str,
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
        bundle_hash: Hash::new(service_version.as_bytes()).to_string(),
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
        reported_pending_mailbox_messages: 0,
        authoritative_pending_mailbox_messages: 0,
        rollout_handle: Some("rollout-2026-03".to_owned()),
        config_generation: 0,
        secret_generation: 0,
        quota_class: Some("taira-open".to_owned()),
        service_lease_status: Some(iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active),
        lease_expires_sequence: Some(100),
        remaining_runtime_balance: Some("50".parse().expect("runtime balance")),
        config_entry_count: 0,
        secret_entry_count: 0,
        config_exports: Vec::new(),
        supports_host_read_config: true,
        supports_host_read_secret_envelope: true,
        supports_private_secret_payload_reads: false,
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
        secret_payload_materialization_dir: materialization_dir
            .join("secret_payloads")
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
        guest_os: iroha_data_model::soracloud::SoraInrouGuestOsV1::DebianSlim,
        guest_images: std::collections::BTreeMap::from([
            (
                iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                    rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                    initrd_image_path: None,
                    distribution: Default::default(),
                    published_artifact: None,
                },
            ),
            (
                iroha_data_model::soracloud::SoraInrouGuestIsaV1::Aarch64,
                iroha_data_model::soracloud::SoraInrouGuestImageV1 {
                    kernel_image_path: "/inrou/aarch64/vmlinux".to_owned(),
                    rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_owned(),
                    initrd_image_path: None,
                    distribution: Default::default(),
                    published_artifact: None,
                },
            ),
        ]),
        bootstrap_user_data_path: None,
        ssh_authorized_keys: vec!["ssh-ed25519 test-key torii-tests".to_owned()],
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
    let placements = assignments
        .iter()
        .map(
            |(replica_slot, validator_account_id, peer_id, _health_status)| {
                iroha_data_model::soracloud::SoraInrouReplicaPlacementV1 {
                    replica_slot: *replica_slot,
                    validator_account_id: validator_account_id.clone(),
                    peer_id: peer_id.clone(),
                    selected_backend:
                        iroha_data_model::soracloud::SoraInrouRuntimeBackendV1::PortableVm,
                    selected_guest_isa: iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                    selected_geography_tag: None,
                    selection_latency_ms: None,
                }
            },
        )
        .collect::<Vec<_>>();
    world
        .soracloud_inrou_service_placements_mut_for_testing()
        .insert(
            (
                bundle.service.service_name.to_string(),
                bundle.service.service_version.clone(),
            ),
            iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                service_name: bundle.service.service_name.clone(),
                service_version: bundle.service.service_version.clone(),
                desired_replica_count,
                eligible_validator_count: u32::try_from(assignments.len()).unwrap_or(u32::MAX),
                placements: placements.clone(),
                reconciled_at_ms: 1,
                last_error: None,
            },
        );
    for ((replica_slot, validator_account_id, peer_id, health_status), placement) in
        assignments.iter().zip(placements.iter())
    {
        world
            .soracloud_inrou_replica_runtime_mut_for_testing()
            .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                    replica_slot.to_string(),
                ),
                iroha_data_model::soracloud::SoraInrouReplicaRuntimeStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
                    service_name: bundle.service.service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    replica_slot: *replica_slot,
                    validator_account_id: validator_account_id.clone(),
                    peer_id: peer_id.clone(),
                    selected_backend: placement.selected_backend,
                    selected_guest_isa: placement.selected_guest_isa,
                    health_status: *health_status,
                    load_factor_bps: 0,
                    materialized_bundle_hash: bundle.container.bundle_hash,
                    accounted_egress_bytes: 0,
                    pending_mailbox_message_count: 0,
                    last_receipt_id: None,
                    updated_at_ms: 1,
                    last_error: None,
                },
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
    seed_public_hosted_http_rollout_app_with_local_replicas_and_snapshot_peer_id(
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
            Some("http://127.0.0.1:18081"),
            Some(201),
        )],
        None,
        service_lease,
    )
}
fn seed_public_hosted_http_rollout_app_with_local_replicas(
    temp: &tempfile::TempDir,
    baseline_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    candidate_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    baseline_local_replicas: Vec<SoracloudRuntimeReplicaPlan>,
    candidate_local_replicas: Vec<SoracloudRuntimeReplicaPlan>,
) -> SharedAppState {
    seed_public_hosted_http_rollout_app_with_local_replicas_and_snapshot_peer_id(
        temp,
        baseline_health,
        candidate_health,
        baseline_local_replicas,
        candidate_local_replicas,
        None,
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
    lease_expires_sequence: u64,
) -> iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
    iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
        status,
        quota_class: "taira-open".to_owned(),
        deployment_deposit: "1".parse().expect("deployment deposit"),
        prepaid_runtime_balance,
        runtime_price_per_sequence: "0.00025".parse().expect("runtime price"),
        storage_price_per_gib_sequence: "0.000025".parse().expect("storage price"),
        egress_price_per_mib: "0.000005".parse().expect("egress price"),
        lease_started_sequence: 0,
        lease_expires_sequence,
        last_billed_sequence: 0,
        accounted_egress_bytes: 0,
        last_status_reason: None,
    }
}
fn seed_public_hosted_http_rollout_app_with_local_replicas_and_snapshot_peer_id(
    temp: &tempfile::TempDir,
    baseline_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    candidate_health: iroha_data_model::soracloud::SoraServiceHealthStatusV1,
    baseline_local_replicas: Vec<SoracloudRuntimeReplicaPlan>,
    candidate_local_replicas: Vec<SoracloudRuntimeReplicaPlan>,
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
    baseline_bundle.service.execution_plane =
        iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService;
    baseline_bundle.service.replicas = std::num::NonZeroU16::new(2).expect("replicas");
    baseline_bundle.service.state_bindings.clear();
    baseline_bundle.service.handlers.clear();
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
            mount_path: "/var/lib/ton-indexer".to_owned(),
            max_total_bytes: std::num::NonZeroU64::new(1024 * 1024).expect("bytes"),
        },
    ];
    baseline_bundle.service.container.manifest_hash = baseline_bundle.container_manifest_hash();
    world.soracloud_service_revisions_mut_for_testing().insert(
        (service_name.to_owned(), baseline_version.to_owned()),
        baseline_bundle.clone(),
    );
    let mut candidate_bundle = baseline_bundle.clone();
    candidate_bundle.service.service_version = candidate_version.to_owned();
    candidate_bundle.container.bundle_hash = Hash::new(b"hosted-http-canary-bundle");
    candidate_bundle.container.bundle_path = "/bundles/public-canary.to".to_owned();
    candidate_bundle.service.container.manifest_hash = candidate_bundle.container_manifest_hash();
    world.soracloud_service_revisions_mut_for_testing().insert(
        (service_name.to_owned(), candidate_version.to_owned()),
        candidate_bundle.clone(),
    );
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(
            service_name.parse().expect("service"),
            iroha_data_model::soracloud::SoraServiceDeploymentStateV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                service_name: service_name.parse().expect("service"),
                current_service_version: baseline_version.to_owned(),
                current_service_manifest_hash: baseline_bundle.service_manifest_hash(),
                current_container_manifest_hash: baseline_bundle.container_manifest_hash(),
                revision_count: 2,
                process_generation: 1,
                process_started_sequence: 1,
                active_rollout: Some(iroha_data_model::soracloud::SoraServiceRolloutStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
                    rollout_handle: "rollout-2026-03".to_owned(),
                    baseline_version: Some(baseline_version.to_owned()),
                    candidate_version: candidate_version.to_owned(),
                    canary_percent: 20,
                    traffic_percent: 20,
                    stage: iroha_data_model::soracloud::SoraRolloutStageV1::Canary,
                    health_failures: 0,
                    max_health_failures: 3,
                    health_window_secs: 60,
                    created_sequence: 1,
                    updated_sequence: 1,
                }),
                last_rollout: None,
                config_generation: 0,
                secret_generation: 0,
                service_configs: BTreeMap::new(),
                service_secrets: BTreeMap::new(),
                fhe_policy_records: BTreeMap::new(),
                service_lease,
                lease_volume_states: Vec::new(),
            },
        );
    let local_validator_account_id =
        checked_torii_test_account_id(0x3d, "derive hosted-http rollout validator fixture key");
    let local_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x3e, "derive hosted-http rollout peer fixture key")
            .public_key()
            .clone(),
    );
    let baseline_assignments = baseline_local_replicas
        .iter()
        .map(|replica| {
            (
                replica.replica_slot,
                local_validator_account_id.clone(),
                local_peer_id.to_string(),
                replica.health_status,
            )
        })
        .collect::<Vec<_>>();
    seed_authoritative_hosted_http_revision(
        &mut world,
        &baseline_bundle,
        baseline_bundle.service.replicas.get(),
        &baseline_assignments,
    );
    let candidate_assignments = candidate_local_replicas
        .iter()
        .map(|replica| {
            (
                replica.replica_slot,
                local_validator_account_id.clone(),
                local_peer_id.to_string(),
                replica.health_status,
            )
        })
        .collect::<Vec<_>>();
    seed_authoritative_hosted_http_revision(
        &mut world,
        &candidate_bundle,
        candidate_bundle.service.replicas.get(),
        &candidate_assignments,
    );
    let baseline_dir = temp.path().join("service-baseline");
    let candidate_dir = temp.path().join("service-canary");
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
        captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
        captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut app = mk_app_state_for_tests_with_world(world);
    let app_mut = Arc::get_mut(&mut app).expect("unique app state");
    app_mut.local_peer_id = Some(local_peer_id);
    app_mut.soracloud_runtime = Some(Arc::new(runtime));
    app
}
fn hosted_http_rollout_test_ip<P>(
    service_name: &str,
    method: &HttpMethod,
    uri: &axum::http::Uri,
    predicate: P,
) -> IpAddr
where
    P: Fn(u8) -> bool,
{
    for octet in 1..=254 {
        let ip = IpAddr::from([203, 0, 113, octet]);
        let bucket = super::hosted_http_rollout_bucket(service_name, Some(ip), method, uri);
        if predicate(bucket) {
            return ip;
        }
    }
    panic!("failed to find a rollout bucket match for hosted-http routing test");
}
fn hosted_http_replica_test_ip<P>(
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
#[tokio::test]
async fn soracloud_public_local_read_route_invokes_runtime_with_authoritative_context() {
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestLocalReadRuntime {
        snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
        state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
        local_peer_id: None,
        result: Ok(iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
            response_bytes: b"asset-body".to_vec(),
            content_type: Some("text/plain; charset=utf-8".to_owned()),
            content_encoding: None,
            cache_control: Some("public, max-age=60".to_owned()),
            bindings: vec![iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
                binding_name: None,
                state_key: None,
                payload_commitment: None,
                artifact_hash: Some(Hash::new(b"asset-hash")),
            }],
            result_commitment: Hash::new(b"result"),
            certified_by:
                iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::StateCommitment,
            runtime_receipt: None,
        }),
        captured_requests: Arc::clone(&captured_requests),
        captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
        captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
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
                .uri("/app/assets")
                .header(axum::http::header::HOST, "portal.sora")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-soracloud-certified-by")
            .and_then(|value| value.to_str().ok()),
        Some("state_commitment")
    );
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert_eq!(body.as_ref(), b"asset-body");
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].service_name, "web_portal");
    assert_eq!(captured[0].handler_name, "assets");
    assert_eq!(captured[0].request_path, "/app/assets");
    assert_eq!(captured[0].handler_path, "/");
}
#[tokio::test]
async fn soradns_public_alias_gateway_routes_local_read_requests() {
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestLocalReadRuntime {
        snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
        state_dir: PathBuf::from("/tmp/test-soradns-public-runtime"),
        local_peer_id: None,
        result: Ok(iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
            response_bytes: b"asset-body".to_vec(),
            content_type: Some("text/plain; charset=utf-8".to_owned()),
            content_encoding: None,
            cache_control: Some("public, max-age=60".to_owned()),
            bindings: vec![iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
                binding_name: None,
                state_key: None,
                payload_commitment: None,
                artifact_hash: Some(Hash::new(b"asset-hash")),
            }],
            result_commitment: Hash::new(b"result"),
            certified_by:
                iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::StateCommitment,
            runtime_receipt: None,
        }),
        captured_requests: Arc::clone(&captured_requests),
        captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
        captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(runtime));
    bind_domain_name_for_test(&app, "portal.sora");
    let router = soradns_public_alias_router(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/soradns/portal.sora/app/assets?fresh=1")
                .header(axum::http::header::HOST, "taira.sora.org")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert_eq!(body.as_ref(), b"asset-body");
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].request_path, "/app/assets");
    assert_eq!(captured[0].request_query.as_deref(), Some("fresh=1"));
    assert_eq!(
        captured[0].request_headers.get("host").map(String::as_str),
        Some("portal.sora")
    );
}
#[tokio::test]
async fn taira_mon_gateway_host_routes_local_read_requests() {
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestLocalReadRuntime {
        snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
        state_dir: PathBuf::from("/tmp/test-taira-mon-public-runtime"),
        local_peer_id: None,
        result: Ok(iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
            response_bytes: b"mon-asset-body".to_vec(),
            content_type: Some("text/plain; charset=utf-8".to_owned()),
            content_encoding: None,
            cache_control: Some("public, max-age=60".to_owned()),
            bindings: vec![iroha_core::soracloud_runtime::SoracloudLocalReadBinding {
                binding_name: None,
                state_key: None,
                payload_commitment: None,
                artifact_hash: Some(Hash::new(b"mon-asset-hash")),
            }],
            result_commitment: Hash::new(b"mon-result"),
            certified_by:
                iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::StateCommitment,
            runtime_receipt: None,
        }),
        captured_requests: Arc::clone(&captured_requests),
        captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
        captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(runtime));
    bind_domain_name_for_test(&app, "portal.sora");
    let router = soradns_public_alias_router(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/app/assets?fresh=1")
                .header(
                    axum::http::header::HOST,
                    "portal.sora.mon.taira.sora.net:443",
                )
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert_eq!(body.as_ref(), b"mon-asset-body");
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].request_path, "/app/assets");
    assert_eq!(captured[0].request_query.as_deref(), Some("fresh=1"));
    assert_eq!(
        captured[0].request_headers.get("host").map(String::as_str),
        Some("portal.sora")
    );
}
#[tokio::test]
async fn soradns_public_alias_gateway_maps_empty_tail_to_root_path() {
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    let mut world = seed_public_soracloud_world();
    let mut bundle = world
        .view()
        .soracloud_service_revisions()
        .get(&("web_portal".to_owned(), "2026.02.0".to_owned()))
        .cloned()
        .expect("seed bundle");
    bundle.service.route.as_mut().expect("public route").host = "docs.sora".to_owned();
    bundle
        .service
        .route
        .as_mut()
        .expect("public route")
        .path_prefix = "/".to_owned();
    bundle.service.handlers = vec![iroha_data_model::soracloud::SoraServiceHandlerV1 {
        handler_name: "root".parse().expect("handler"),
        class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Asset,
        entrypoint: "serve_root".to_owned(),
        route_path: Some("/".to_owned()),
        certified_response:
            iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::StateCommitment,
        mailbox: None,
    }];
    bundle.service.container.manifest_hash = bundle.container_manifest_hash();
    world.soracloud_service_revisions_mut_for_testing().insert(
        ("web_portal".to_owned(), "2026.02.0".to_owned()),
        bundle.clone(),
    );
    let deployment_service_name: iroha_data_model::name::Name =
        "web_portal".parse().expect("service");
    let mut deployment = world
        .view()
        .soracloud_service_deployments()
        .get(&deployment_service_name)
        .cloned()
        .expect("deployment");
    deployment.current_service_manifest_hash = bundle.service_manifest_hash();
    deployment.current_container_manifest_hash = bundle.container_manifest_hash();
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(deployment_service_name, deployment);
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestLocalReadRuntime {
        snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
        state_dir: PathBuf::from("/tmp/test-soradns-public-root"),
        local_peer_id: None,
        result: Ok(iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
            response_bytes: b"docs-root".to_vec(),
            content_type: Some("text/plain".to_owned()),
            content_encoding: None,
            cache_control: None,
            bindings: Vec::new(),
            result_commitment: Hash::new(b"docs-root-result"),
            certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
            runtime_receipt: None,
        }),
        captured_requests: Arc::clone(&captured_requests),
        captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
        captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut app = mk_app_state_for_tests_with_world(world);
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(runtime));
    bind_domain_name_for_test(&app, "docs.sora");
    let router = soradns_public_alias_router(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/soradns/docs.sora")
                .header(axum::http::header::HOST, "taira.sora.org")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert_eq!(body.as_ref(), b"docs-root");
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].request_path, "/");
    assert_eq!(
        captured[0].request_headers.get("host").map(String::as_str),
        Some("docs.sora")
    );
}
#[tokio::test]
async fn soradns_public_alias_gateway_rejects_invalid_aliases() {
    use tower::ServiceExt as _;
    let app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    let router = soradns_public_alias_router(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/soradns/bad%20name.sora/app/assets")
                .header(axum::http::header::HOST, "taira.sora.org")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
#[tokio::test]
async fn soradns_public_alias_gateway_rejects_inactive_aliases() {
    use tower::ServiceExt as _;
    let app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    bind_domain_name_for_test_with_status(
        &app,
        "portal.sora",
        iroha_data_model::sns::NameStatus::Frozen(iroha_data_model::sns::NameFrozenStateV1 {
            reason: "guardian hold".to_owned(),
            until_ms: u64::MAX,
        }),
    );
    let router = soradns_public_alias_router(app);
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
#[tokio::test]
async fn soradns_public_alias_gateway_accepts_non_sora_alias_hosts() {
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    let mut world = seed_public_soracloud_world();
    let mut bundle = world
        .view()
        .soracloud_service_revisions()
        .get(&("web_portal".to_owned(), "2026.02.0".to_owned()))
        .cloned()
        .expect("seed bundle");
    bundle.service.route.as_mut().expect("public route").host = "portal.dao".to_owned();
    bundle.service.container.manifest_hash = bundle.container_manifest_hash();
    world.soracloud_service_revisions_mut_for_testing().insert(
        ("web_portal".to_owned(), "2026.02.0".to_owned()),
        bundle.clone(),
    );
    let deployment_service_name: iroha_data_model::name::Name =
        "web_portal".parse().expect("service");
    let mut deployment = world
        .view()
        .soracloud_service_deployments()
        .get(&deployment_service_name)
        .cloned()
        .expect("deployment");
    deployment.current_service_manifest_hash = bundle.service_manifest_hash();
    deployment.current_container_manifest_hash = bundle.container_manifest_hash();
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(deployment_service_name, deployment);
    let captured_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let runtime = TestLocalReadRuntime {
        snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
        state_dir: PathBuf::from("/tmp/test-soradns-public-dao"),
        local_peer_id: None,
        result: Ok(iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
            response_bytes: b"dao-alias".to_vec(),
            content_type: Some("text/plain".to_owned()),
            content_encoding: None,
            cache_control: None,
            bindings: Vec::new(),
            result_commitment: Hash::new(b"dao-alias-result"),
            certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
            runtime_receipt: None,
        }),
        captured_requests: Arc::clone(&captured_requests),
        captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
        captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut app = mk_app_state_for_tests_with_world(world);
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(runtime));
    bind_domain_name_for_test(&app, "portal.dao");
    let router = soradns_public_alias_router(app);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/soradns/portal.dao/app/assets")
                .header(axum::http::header::HOST, "taira.sora.org")
                .extension(crate::loopback_connect_info())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert_eq!(body.as_ref(), b"dao-alias");
    let captured = captured_requests.lock().expect("capture lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].request_headers.get("host").map(String::as_str),
        Some("portal.dao")
    );
}
#[tokio::test]
async fn soracloud_public_split_app_routes_hosted_live_and_local_vault_on_one_node() {
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream listener");
    let addr = listener.local_addr().expect("upstream addr");
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
    let listen_base_url = format!("http://{addr}");
    let live_materialization_dir = temp.path().join("travel-ops-live");
    let mut world = seed_public_soracloud_world();
    let seed_bundle = world
        .view()
        .soracloud_service_revisions()
        .get(&("web_portal".to_owned(), "2026.02.0".to_owned()))
        .cloned()
        .expect("seed bundle");
    let mut live_bundle = seed_bundle.clone();
    live_bundle.service.service_name = "travel_ops_live".parse().expect("service");
    live_bundle.service.service_version = "2026.04.0".to_owned();
    live_bundle.container.runtime = iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou;
    live_bundle.container.bundle_hash = Hash::new(b"travel-ops-live-bundle");
    live_bundle.container.bundle_path = "/bundles/travel-ops-live.to".to_owned();
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
    live_bundle.service.container.manifest_hash = live_bundle.container_manifest_hash();
    let mut vault_bundle = seed_bundle;
    vault_bundle.service.service_name = "travel_ops_vault".parse().expect("service");
    vault_bundle.service.service_version = "2026.04.0".to_owned();
    vault_bundle.container.bundle_hash = Hash::new(b"travel-ops-vault-bundle");
    vault_bundle.container.bundle_path = "/bundles/travel-ops-vault.to".to_owned();
    vault_bundle.service.route = Some(iroha_data_model::soracloud::SoraRouteTargetV1 {
        host: "travel.sora".to_owned(),
        path_prefix: "/api".to_owned(),
        service_port: std::num::NonZeroU16::new(8788).expect("port"),
        visibility: iroha_data_model::soracloud::SoraRouteVisibilityV1::Public,
        tls_mode: iroha_data_model::soracloud::SoraTlsModeV1::Required,
    });
    vault_bundle.service.handlers = vec![iroha_data_model::soracloud::SoraServiceHandlerV1 {
        handler_name: "auth_me".parse().expect("handler"),
        class: iroha_data_model::soracloud::SoraServiceHandlerClassV1::Query,
        entrypoint: "serve_auth_me".to_owned(),
        route_path: Some("/auth/me".to_owned()),
        certified_response:
            iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::AuditReceipt,
        mailbox: None,
    }];
    vault_bundle.service.container.manifest_hash = vault_bundle.container_manifest_hash();
    for bundle in [live_bundle.clone(), vault_bundle.clone()] {
        let service_name = bundle.service.service_name.clone();
        world.soracloud_service_revisions_mut_for_testing().insert(
            (
                bundle.service.service_name.to_string(),
                bundle.service.service_version.clone(),
            ),
            bundle.clone(),
        );
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(
                service_name,
                iroha_data_model::soracloud::SoraServiceDeploymentStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
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
                    service_lease: if bundle.service.execution_plane
                        == iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService
                    {
                        Some(iroha_data_model::soracloud::SoraServiceLeaseStateV1 {
                            schema_version:
                                iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
                            status: iroha_data_model::soracloud::SoraServiceLeaseStatusV1::Active,
                            quota_class: "taira-open".to_owned(),
                            deployment_deposit: "1".parse().expect("deployment deposit"),
                            prepaid_runtime_balance: "50".parse().expect("runtime balance"),
                            runtime_price_per_sequence: "0.00025".parse().expect("runtime price"),
                            storage_price_per_gib_sequence: "0.000025"
                                .parse()
                                .expect("storage price"),
                            egress_price_per_mib: "0.000005".parse().expect("egress price"),
                            lease_started_sequence: 0,
                            lease_expires_sequence: 100,
                            last_billed_sequence: 0,
                            accounted_egress_bytes: 0,
                            last_status_reason: None,
                        })
                    } else {
                        None
                    },
                    lease_volume_states: Vec::new(),
                },
            );
    }
    let live_validator_account_id =
        checked_torii_test_account_id(0x45, "derive hosted live/local split validator fixture key");
    let live_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x46, "derive hosted live/local split peer fixture key")
            .public_key()
            .clone(),
    );
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
                iroha_core::soracloud_runtime::SoracloudRuntimeRevisionRole::Active,
                100,
                iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
                vec![hosted_http_runtime_replica_plan(
                    &live_materialization_dir,
                    1,
                    iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
                    Some(listen_base_url.as_str()),
                    Some(1),
                )],
            ),
        )]),
    );
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
        captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
        captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
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
    let live_body = live_response
        .into_body()
        .collect()
        .await
        .expect("live body")
        .to_bytes();
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
    let vault_body = vault_response
        .into_body()
        .collect()
        .await
        .expect("vault body")
        .to_bytes();
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
