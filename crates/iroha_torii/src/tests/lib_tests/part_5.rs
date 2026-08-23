#[test]
fn handler_local_api_token_checks_use_the_canonical_evaluator() {
    let source = include_str!("../../lib.rs");
    let fail_open_empty_set_check = ["!app.api_tokens_", "set.is_empty()"].concat();
    let direct_membership_check = ["app.api_tokens_set.", "contains"].concat();
    assert!(
        !source.contains(&fail_open_empty_set_check),
        "handler-local API-token checks must not make an empty required token set fail open"
    );
    assert!(
        !source.contains(&direct_membership_check),
        "handler-local API-token checks must use evaluate_api_token"
    );
}
#[tokio::test]
#[cfg(feature = "telemetry")]
async fn norito_rpc_gate_records_metrics() {
    let cfg = actual::NoritoRpcTransport {
            enabled: true,
            require_mtls: false,
            stage: actual::NoritoRpcStage::Canary,
            allowed_clients: vec!["ok".into()],
            mtls_trusted_proxy_cidrs:
                iroha_config::parameters::defaults::torii::transport::norito_rpc::mtls_trusted_proxy_cidrs(),
        };
    let (app, metrics) = mk_norito_rpc_test_harness(cfg.clone()).await;
    let trusted_remote = Some("127.0.0.1".parse().expect("trusted proxy"));
    let untrusted_remote = Some("198.51.100.10".parse().expect("untrusted proxy"));
    let mut headers = HeaderMap::new();
    headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("ok"));
    app.check_norito_rpc_allowed(&headers, trusted_remote)
        .expect("canary token should be allowed");
    assert_eq!(
        metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&[cfg.stage.label(), "allowed"])
            .get(),
        1
    );
    let missing_token_headers = HeaderMap::new();
    assert!(
        app.check_norito_rpc_allowed(&missing_token_headers, trusted_remote)
            .is_err()
    );
    assert_eq!(
        metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&[cfg.stage.label(), "canary_missing_token"])
            .get(),
        1
    );
    let mut wrong_token_headers = HeaderMap::new();
    wrong_token_headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("wrong"));
    assert!(
        app.check_norito_rpc_allowed(&wrong_token_headers, trusted_remote)
            .is_err()
    );
    assert_eq!(
        metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&[cfg.stage.label(), "canary_denied"])
            .get(),
        1
    );
    let mtls_cfg = actual::NoritoRpcTransport {
            enabled: true,
            require_mtls: true,
            stage: actual::NoritoRpcStage::Ga,
            allowed_clients: Vec::new(),
            mtls_trusted_proxy_cidrs:
                iroha_config::parameters::defaults::torii::transport::norito_rpc::mtls_trusted_proxy_cidrs(),
        };
    let (mtls_app, mtls_metrics) = mk_norito_rpc_test_harness(mtls_cfg.clone()).await;
    assert!(
        mtls_app
            .check_norito_rpc_allowed(&HeaderMap::new(), trusted_remote)
            .is_err()
    );
    assert_eq!(
        mtls_metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&[mtls_cfg.stage.label(), "mtls_required"])
            .get(),
        1
    );
    let mut mtls_headers = HeaderMap::new();
    mtls_headers.insert(HEADER_MTLS_FORWARD, HeaderValue::from_static("present"));
    mtls_app
        .check_norito_rpc_allowed(&mtls_headers, trusted_remote)
        .expect("mtls header should allow RPC");
    assert_eq!(
        mtls_metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&[mtls_cfg.stage.label(), "allowed"])
            .get(),
        1
    );
    assert!(
        mtls_app
            .check_norito_rpc_allowed(&mtls_headers, untrusted_remote)
            .is_err()
    );
    assert_eq!(
        mtls_metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&[mtls_cfg.stage.label(), "mtls_required"])
            .get(),
        2
    );
    let disabled_cfg = actual::NoritoRpcTransport::default();
    let (disabled_app, disabled_metrics) = mk_norito_rpc_test_harness(disabled_cfg.clone()).await;
    assert!(
        disabled_app
            .check_norito_rpc_allowed(&HeaderMap::new(), trusted_remote)
            .is_err()
    );
    assert_eq!(
        disabled_metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&[actual::NoritoRpcStage::Disabled.label(), "disabled"])
            .get(),
        1
    );
}
#[tokio::test]
async fn soracloud_signed_mutation_middleware_strips_internal_identity_and_rejects_replay() {
    use axum::{Router, body::Bytes, routing::post};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    async fn probe(headers: HeaderMap, body: Bytes) -> axum::response::Response {
        let verified_account = headers
            .get(axum::http::HeaderName::from_static(
                soracloud::VERIFIED_ACCOUNT_HEADER,
            ))
            .and_then(|value| std::str::from_utf8(value.as_bytes()).ok());
        let verified_signer = headers
            .get(axum::http::HeaderName::from_static(
                soracloud::VERIFIED_SIGNER_HEADER,
            ))
            .and_then(|value| value.to_str().ok());
        let verified_signers = headers
            .get(axum::http::HeaderName::from_static(
                soracloud::VERIFIED_SIGNERS_HEADER,
            ))
            .and_then(|value| value.to_str().ok());
        let client_identity_was_removed = [verified_account, verified_signer, verified_signers]
            .into_iter()
            .flatten()
            .all(|value| value != "attacker");
        axum::response::Response::builder()
            .status(
                if verified_account.is_some()
                    && verified_signer.is_some()
                    && verified_signers.is_some()
                    && client_identity_was_removed
                {
                    StatusCode::OK
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                },
            )
            .body(Body::from(body))
            .expect("response")
    }
    let _guard = crate::tests_runtime_handlers::app_auth_test_guard(
        crate::app_auth::CanonicalRequestAuthConfig::default(),
    );
    let key_pair = checked_torii_test_ed25519_keypair(
        0x23,
        "derive Soracloud signed mutation replay fixture key",
    );
    let account_id = AccountId::new(key_pair.public_key().clone());
    let app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(
        crate::tests_runtime_handlers::world_with_account(&account_id),
    );
    let router = Router::new()
        .route("/v1/soracloud/test", post(probe))
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            enforce_soracloud_signed_mutation_request,
        ))
        .with_state(app);
    let unsigned = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .uri("/v1/soracloud/test")
        .body(Body::from(br#"{"op":"unsigned"}"#.to_vec()))
        .expect("unsigned request");
    let unsigned_response = router.clone().oneshot(unsigned).await.expect("response");
    assert_eq!(unsigned_response.status(), StatusCode::FORBIDDEN);
    let unsigned_body = unsigned_response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert!(
        String::from_utf8_lossy(&unsigned_body).contains("signed account headers are required")
    );
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/soracloud/test".parse().expect("uri");
    let body = br#"{"op":"signed"}"#;
    let headers = crate::tests_runtime_handlers::signed_app_headers(
        &account_id,
        &key_pair,
        &method,
        &uri,
        body,
    );
    let mut signed_builder = axum::http::Request::builder()
        .method(method.clone())
        .uri(uri.to_string())
        .header(soracloud::VERIFIED_ACCOUNT_HEADER, "attacker")
        .header(soracloud::VERIFIED_SIGNER_HEADER, "attacker")
        .header(soracloud::VERIFIED_SIGNERS_HEADER, "attacker");
    for (name, value) in &headers {
        signed_builder = signed_builder.header(name, value);
    }
    let signed = signed_builder
        .body(Body::from(body.to_vec()))
        .expect("signed request");
    let signed_response = router.clone().oneshot(signed).await.expect("response");
    assert_eq!(signed_response.status(), StatusCode::OK);
    let mut replay_builder = axum::http::Request::builder()
        .method(method)
        .uri(uri.to_string());
    for (name, value) in &headers {
        replay_builder = replay_builder.header(name, value);
    }
    let replay = replay_builder
        .body(Body::from(body.to_vec()))
        .expect("replay request");
    let replay_response = router.oneshot(replay).await.expect("response");
    assert_eq!(replay_response.status(), StatusCode::FORBIDDEN);
    let replay_body = replay_response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert!(String::from_utf8_lossy(&replay_body).contains("nonce already used"));
}
#[tokio::test]
async fn soracloud_signed_mutation_middleware_applies_route_body_caps_before_auth() {
    use axum::{Router, routing::post};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    async fn probe() -> axum::response::Response {
        StatusCode::OK.into_response()
    }
    let account_id = checked_torii_test_account_id(
        0x40,
        "derive Soracloud signed mutation body-cap fixture key",
    );
    let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(
        crate::tests_runtime_handlers::world_with_account(&account_id),
    );
    let app_state = Arc::get_mut(&mut app).expect("test owns app state");
    app_state.soracloud_mutation_max_body_bytes = 8;
    app_state.soracloud_upload_max_body_bytes = 64;
    let router = Router::new()
        .route("/v1/soracloud/test", post(probe))
        .route("/v1/soracloud/model/upload/register", post(probe))
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            enforce_soracloud_signed_mutation_request,
        ))
        .with_state(app);
    let body = vec![b'x'; 32];
    let oversized = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .uri("/v1/soracloud/test")
        .body(Body::from(body.clone()))
        .expect("request");
    let oversized_response = router.clone().oneshot(oversized).await.expect("response");
    assert_eq!(oversized_response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let oversized_body = oversized_response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert!(String::from_utf8_lossy(&oversized_body).contains("mutation request body exceeds"));
    let upload = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .uri("/v1/soracloud/model/upload/register")
        .body(Body::from(body))
        .expect("request");
    let upload_response = router.oneshot(upload).await.expect("response");
    assert_eq!(upload_response.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn soracloud_signed_mutation_middleware_rate_limits_account_across_caller_origins() {
    use axum::{Router, routing::post};
    use tower::ServiceExt as _;
    async fn probe() -> axum::response::Response {
        StatusCode::OK.into_response()
    }
    let _guard = crate::tests_runtime_handlers::app_auth_test_guard(
        crate::app_auth::CanonicalRequestAuthConfig::default(),
    );
    let key_pair = checked_torii_test_ed25519_keypair(
        0x41,
        "derive Soracloud signed mutation rate-limit fixture key",
    );
    let account_id = AccountId::new(key_pair.public_key().clone());
    let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(
        crate::tests_runtime_handlers::world_with_account(&account_id),
    );
    Arc::get_mut(&mut app)
        .expect("test owns app state")
        .soracloud_mutation_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
    let router = Router::new()
        .route("/v1/soracloud/hf/deploy", post(probe))
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            enforce_soracloud_signed_mutation_request,
        ))
        .with_state(app);
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/soracloud/hf/deploy".parse().expect("uri");
    let body = br#"{"model":"test"}"#;
    let mut statuses = Vec::new();
    for origin in ["https://wallet-a.test", "https://wallet-b.test"] {
        let headers = crate::tests_runtime_handlers::signed_app_headers(
            &account_id,
            &key_pair,
            &method,
            &uri,
            body,
        );
        let mut builder = axum::http::Request::builder()
            .method(method.clone())
            .uri(uri.to_string())
            .header(axum::http::header::ORIGIN, origin);
        for (name, value) in &headers {
            builder = builder.header(name, value);
        }
        let response = router
            .clone()
            .oneshot(
                builder
                    .body(Body::from(body.to_vec()))
                    .expect("signed request"),
            )
            .await
            .expect("response");
        statuses.push(response.status());
    }
    assert_eq!(statuses, [StatusCode::OK, StatusCode::TOO_MANY_REQUESTS]);
}
#[test]
fn soracloud_signed_mutation_route_groups_cover_load_gate_paths() {
    let account_id = checked_torii_test_account_id(
        0x42,
        "derive Soracloud signed mutation route-group fixture key",
    );
    assert_eq!(
        super::soracloud_signed_mutation_route_group("/v1/soracloud/deploy"),
        "mutation"
    );
    assert_eq!(
        super::soracloud_signed_mutation_route_group("/v1/soracloud/model/session/start"),
        "model"
    );
    assert_eq!(
        super::soracloud_signed_mutation_route_group("/v1/soracloud/model/upload/register"),
        "upload"
    );
    assert_eq!(
        super::soracloud_signed_mutation_route_group("/v1/soracloud/hf/deploy"),
        "hf"
    );
    let model_key = super::soracloud_signed_mutation_rate_key(&account_id, "model");
    let hf_key = super::soracloud_signed_mutation_rate_key(&account_id, "hf");
    assert_ne!(model_key, hf_key);
    assert!(!model_key.contains("origin:"));
    assert!(hf_key.contains("soracloud:hf:account:"));
}
#[tokio::test]
async fn soracloud_signed_mutation_middleware_enforces_global_inflight_limit() {
    use axum::{Router, routing::post};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    async fn probe() -> axum::response::Response {
        StatusCode::OK.into_response()
    }
    let _guard = crate::tests_runtime_handlers::app_auth_test_guard(
        crate::app_auth::CanonicalRequestAuthConfig::default(),
    );
    let key_pair = checked_torii_test_ed25519_keypair(
        0x43,
        "derive Soracloud signed mutation inflight fixture key",
    );
    let account_id = AccountId::new(key_pair.public_key().clone());
    let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(
        crate::tests_runtime_handlers::world_with_account(&account_id),
    );
    Arc::get_mut(&mut app)
        .expect("test owns app state")
        .soracloud_mutation_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    let router = Router::new()
        .route("/v1/soracloud/model/session/start", post(probe))
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            enforce_soracloud_signed_mutation_request,
        ))
        .with_state(app);
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/soracloud/model/session/start".parse().expect("uri");
    let body = br#"{"session":"test"}"#;
    let headers = crate::tests_runtime_handlers::signed_app_headers(
        &account_id,
        &key_pair,
        &method,
        &uri,
        body,
    );
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(uri.to_string());
    for (name, value) in &headers {
        builder = builder.header(name, value);
    }
    let response = router
        .oneshot(
            builder
                .body(Body::from(body.to_vec()))
                .expect("signed request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let response_body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert!(
        String::from_utf8_lossy(&response_body)
            .contains("Soracloud model request concurrency limit exceeded")
    );
}
#[tokio::test]
async fn soracloud_public_runtime_rate_and_inflight_limits_fail_closed() {
    use std::net::{IpAddr, Ipv4Addr};
    let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
    let app_state = Arc::get_mut(&mut app).expect("test owns app state");
    app_state.soracloud_public_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
    let method = axum::http::Method::GET;
    let uri: axum::http::Uri = "/healthz".parse().expect("uri");
    let remote_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9));
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::HOST,
        HeaderValue::from_static("portal.sora.test"),
    );
    let first = super::execute_soracloud_public_runtime_request(
        app.clone(),
        method.clone(),
        uri.clone(),
        headers.clone(),
        remote_ip,
        Bytes::new(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::NOT_FOUND);
    let second = super::execute_soracloud_public_runtime_request(
        app.clone(),
        method.clone(),
        uri.clone(),
        headers.clone(),
        remote_ip,
        Bytes::new(),
    )
    .await;
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    let mut busy_app = crate::tests_runtime_handlers::mk_app_state_for_tests();
    let busy_state = Arc::get_mut(&mut busy_app).expect("test owns app state");
    busy_state.soracloud_public_rate_limiter = limits::RateLimiter::new(None, None);
    busy_state.soracloud_public_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    let busy = super::execute_soracloud_public_runtime_request(
        busy_app,
        method,
        uri,
        headers,
        remote_ip,
        Bytes::new(),
    )
    .await;
    assert_eq!(busy.status(), StatusCode::SERVICE_UNAVAILABLE);
}
#[tokio::test]
async fn zk_attachment_route_authenticates_before_decode_and_rejects_replay() {
    use axum::{Extension, Router, routing::post};
    use tower::ServiceExt as _;
    async fn probe(
        Extension(_verified): Extension<crate::app_auth::VerifiedCanonicalRequest>,
        crate::utils::extractors::JsonOnly(_body): crate::utils::extractors::JsonOnly<
            norito::json::Value,
        >,
    ) -> StatusCode {
        StatusCode::NO_CONTENT
    }
    let _guard = crate::tests_runtime_handlers::app_auth_test_guard(
        crate::app_auth::CanonicalRequestAuthConfig::default(),
    );
    let key_pair =
        checked_torii_test_ed25519_keypair(0x44, "derive ZK attachment tenant fixture key");
    let account_id = AccountId::new(key_pair.public_key().clone());
    let app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(
        crate::tests_runtime_handlers::world_with_account(&account_id),
    );
    let router = Router::new()
        .route("/v1/zk/attachments", post(probe))
        .layer(axum::middleware::from_fn_with_state(
            CanonicalAccountBodyAuthState {
                app: app.clone(),
                max_body_bytes: 1024,
            },
            enforce_canonical_account_body_authentication,
        ));
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/zk/attachments".parse().expect("uri");
    let body = br#"{"attachment":"test"}"#;
    let headers = crate::tests_runtime_handlers::signed_app_headers(
        &account_id,
        &key_pair,
        &method,
        &uri,
        body,
    );
    let unsigned = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method(method.clone())
                .uri(uri.clone())
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{"))
                .expect("unsigned malformed attachment request"),
        )
        .await
        .expect("unsigned attachment response");
    assert_eq!(unsigned.status(), StatusCode::UNAUTHORIZED);
    let unsigned_body = unsigned
        .into_body()
        .collect()
        .await
        .expect("unsigned response body")
        .to_bytes();
    let unsigned_error = norito::decode_from_bytes::<super::ErrorEnvelope>(&unsigned_body)
        .expect("unsigned response error envelope");
    assert_eq!(unsigned_error.code(), "canonical_authentication_required");
    let signed_request = || {
        let mut request = axum::http::Request::builder()
            .method(method.clone())
            .uri(uri.clone())
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_vec()))
            .expect("signed attachment request");
        request.headers_mut().extend(headers.clone());
        request
    };
    let mut admitted_request = signed_request();
    let body_polls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed_body_polls = Arc::clone(&body_polls);
    *admitted_request.body_mut() = Body::from_stream(futures::stream::poll_fn(
        move |_| -> std::task::Poll<Option<Result<Bytes, std::convert::Infallible>>> {
            observed_body_polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            panic!("admitted canonical body must not be polled a second time")
        },
    ));
    admitted_request
        .extensions_mut()
        .insert(AdmittedAppRoutedReadBody {
            bytes: Bytes::copy_from_slice(body),
            destination_bytes: body.len(),
        });
    let reservation = try_acquire_new_query_fanout_memory(&app).expect("test fanout reservation");
    let admission = AppRoutedReadHttpAdmission {
        reservation: reservation.clone(),
        decode_plan: torii_routed_read_request_decode_plan(&app).expect("test decode plan"),
    };
    let accepted = APP_ROUTED_READ_HTTP_ADMISSION
        .scope(admission, router.clone().oneshot(admitted_request))
        .await
        .expect("admitted attachment response");
    assert_eq!(body_polls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(accepted.status(), StatusCode::NO_CONTENT);
    drop(accepted);
    drop(reservation);
    let replayed = router
        .oneshot(signed_request())
        .await
        .expect("replayed attachment response");
    assert_eq!(replayed.status(), StatusCode::FORBIDDEN);
    let replayed_body = replayed
        .into_body()
        .collect()
        .await
        .expect("replayed response body")
        .to_bytes();
    let replayed_error = norito::decode_from_bytes::<super::ErrorEnvelope>(&replayed_body)
        .expect("replayed response error envelope");
    assert_eq!(replayed_error.code(), "query_validation_failed");
}
#[tokio::test]
async fn rpc_capabilities_reflect_transport_config() {
    let cfg = actual::NoritoRpcTransport {
            enabled: true,
            require_mtls: true,
            stage: actual::NoritoRpcStage::Canary,
            allowed_clients: vec!["alpha".into(), "beta".into()],
            mtls_trusted_proxy_cidrs:
                iroha_config::parameters::defaults::torii::transport::norito_rpc::mtls_trusted_proxy_cidrs(),
        };
    let app = mk_app_state_for_tests_with_options(None, None, Some(cfg.clone()), None);
    let response = app.rpc_capabilities();
    assert!(response.norito_rpc.enabled);
    assert!(response.norito_rpc.require_mtls);
    assert_eq!(
        response.norito_rpc.stage,
        cfg.stage.label(),
        "stage label should match config"
    );
    assert_eq!(
        response.norito_rpc.canary_allowlist_size,
        cfg.allowed_clients.len()
    );
}
#[tokio::test]
async fn rpc_ping_reuses_capabilities_advert() {
    let cfg = actual::NoritoRpcTransport {
            enabled: false,
            require_mtls: false,
            stage: actual::NoritoRpcStage::Ga,
            allowed_clients: Vec::new(),
            mtls_trusted_proxy_cidrs:
                iroha_config::parameters::defaults::torii::transport::norito_rpc::mtls_trusted_proxy_cidrs(),
        };
    let app = mk_app_state_for_tests_with_options(None, None, Some(cfg.clone()), None);
    let response = app.rpc_ping();
    assert!(response.ok);
    assert!(response.unix_time_ms > 0);
    assert_eq!(
        response.norito_rpc.stage,
        cfg.stage.label(),
        "ping should advertise the same stage"
    );
    assert_eq!(response.norito_rpc.canary_allowlist_size, 0);
}
#[tokio::test]
async fn error_response_contains_details() {
    let mismatch = iroha_data_model::isi::error::Mismatch {
        expected: iroha_data_model::transaction::TransactionDomain::Network(
            crate::test_utils::signed_query_network_id(),
        ),
        actual: iroha_data_model::transaction::TransactionDomain::Genesis,
    };
    let expected_message = format!("failed to accept transaction: {mismatch}");
    let err = Error::AcceptTransaction(
        iroha_core::tx::AcceptTransactionFail::TransactionDomainMismatch(mismatch),
    );
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap())
        .expect("content-type header present");
    assert_eq!(content_type, super::utils::NORITO_MIME_TYPE);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let payload = norito::decode_from_bytes::<super::ErrorEnvelope>(&body).unwrap();
    assert_eq!(payload.code(), "transaction_rejected");
    assert_eq!(payload.message(), expected_message);
}
#[test]
fn start_server_maps_to_internal_server_error() {
    assert_eq!(
        Error::StartServer.status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
#[test]
fn failed_exit_maps_to_internal_server_error() {
    assert_eq!(
        Error::FailedExit.status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
#[test]
fn contract_rejection_maps_to_unprocessable_entity() {
    let rejection = iroha_data_model::executor::ContractRejection {
        contract: "BoiFiLiquidity".to_owned(),
        namespace: "FiLiquidityError".to_owned(),
        name: "BelowMinimum".to_owned(),
        code: 18,
    };
    assert_eq!(
        Error::Query(ValidationFail::ContractRejected(rejection)).status_code(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
