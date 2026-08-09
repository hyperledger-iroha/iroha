#[test]
fn sccp_submit_ingress_has_closed_endpoint_specific_limits() {
    let destination = super::sccp_submit_ingress_policy("/v1/bridge/proofs/submit")
        .expect("destination endpoint policy");
    let native =
        super::sccp_submit_ingress_policy("/v1/bridge/messages").expect("native endpoint policy");

    assert_eq!(destination.telemetry_label, "bridge_proof");
    assert_eq!(native.telemetry_label, "bridge_message");
    assert_eq!(
        destination.rate_limit_cost,
        super::FINALITY_HEAVY_QUERY_RATE_COST
    );
    assert_eq!(native.rate_limit_cost, destination.rate_limit_cost);
    let shared_fields =
        super::canonical_base64_max_len(super::SCCP_SUBMIT_MAX_TRANSACTION_PAYLOAD_BYTES_V1);
    let shared_fields = shared_fields
        + super::canonical_base64_max_len(super::SCCP_SUBMIT_MAX_DETACHED_SIGNATURE_BYTES_V1)
        + super::SCCP_SUBMIT_JSON_ENVELOPE_ALLOWANCE_BYTES_V1;
    assert_eq!(
        destination.max_body_bytes,
        iroha_sccp::SCCP_GROTH16_BN254_MAX_BASE64_ARTIFACT_BYTES_V1 + shared_fields
    );
    assert_eq!(
        native.max_body_bytes,
        iroha_sccp::SCCP_NATIVE_ADMISSION_MAX_BASE64_BYTES_V1 + shared_fields
    );
    assert!(destination.max_body_bytes > native.max_body_bytes);
    let default_operator_cap =
        usize::try_from(iroha_config::parameters::defaults::torii::MAX_CONTENT_LEN.0)
            .expect("default Torii content limit fits usize");
    assert!(destination.max_body_bytes <= default_operator_cap);
    assert!(native.max_body_bytes <= default_operator_cap);
    assert!(super::sccp_submit_ingress_policy("/v1/bridge/proofs").is_none());
    assert!(super::sccp_submit_ingress_policy("/v1/bridge/messages/").is_none());
}

#[tokio::test]
async fn sccp_submit_body_caps_chunked_streams_without_content_length() {
    let stream = futures::stream::iter([
        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"1234")),
        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"5678")),
    ]);
    let error = super::collect_sccp_submit_body(axum::body::Body::from_stream(stream), 7)
        .await
        .expect_err("chunked body must not exceed the explicit cap");

    assert_eq!(error, super::SccpSubmitBodyReadError::TooLarge);
}

#[tokio::test]
async fn sccp_submit_body_accepts_exact_cap_and_rejects_one_byte_over() {
    let exact = super::collect_sccp_submit_body(axum::body::Body::from("12345678"), 8)
        .await
        .expect("body at the exact endpoint cap must be accepted");
    assert_eq!(exact.as_ref(), b"12345678");

    let error = super::collect_sccp_submit_body(axum::body::Body::from("123456789"), 8)
        .await
        .expect_err("one byte over the endpoint cap must reject");
    assert_eq!(error, super::SccpSubmitBodyReadError::TooLarge);
}

#[tokio::test]
async fn sccp_submit_body_distinguishes_transport_failure_from_size_rejection() {
    let stream = futures::stream::iter([Err::<axum::body::Bytes, _>(std::io::Error::other(
        "adversarial stream failure",
    ))]);
    let error = super::collect_sccp_submit_body(axum::body::Body::from_stream(stream), 64)
        .await
        .expect_err("transport error must fail closed");

    assert_eq!(error, super::SccpSubmitBodyReadError::Read);
}

#[tokio::test]
async fn sccp_submit_ingress_rejects_missing_token_without_polling_body() {
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::from(["expected-token".to_owned()]));
    }
    let router = sccp_ingress_test_router(app);

    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let mut request = sccp_ingress_request(path, sccp_body_that_must_not_be_polled());
        request.headers_mut().insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from_static("not-a-length"),
        );
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("middleware response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "path {path}");
    }
}

#[tokio::test]
async fn sccp_submit_ingress_fails_closed_when_required_tokens_are_unconfigured() {
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests_with_options(None, Some((1, 8)), None, None);
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::new());
        state.query_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        state.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        state.query_queue_timeout = Duration::ZERO;
    }
    let router = sccp_ingress_test_router(Arc::clone(&app));

    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let mut request = sccp_ingress_request(path, sccp_body_that_must_not_be_polled());
        request.headers_mut().insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from_static("not-a-length"),
        );
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("middleware response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "path {path}");
    }
    assert!(
        app.deploy_rate_limiter.allow_cost("127.0.0.1", 8).await,
        "unconfigured authentication must reject before consuming rate budget"
    );
}

#[tokio::test]
async fn sccp_submit_ingress_rejects_duplicate_tokens_before_rate_body_and_admission() {
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests_with_options(None, Some((1, 8)), None, None);
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::from(["expected-token".to_owned()]));
        state.query_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        state.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        state.query_queue_timeout = Duration::ZERO;
    }
    let router = sccp_ingress_test_router(Arc::clone(&app));
    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let mut request = sccp_ingress_request(path, sccp_body_that_must_not_be_polled());
        request
            .headers_mut()
            .append(HEADER_API_TOKEN, HeaderValue::from_static("expected-token"));
        request
            .headers_mut()
            .append(HEADER_API_TOKEN, HeaderValue::from_static("expected-token"));

        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("middleware response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "path {path}");
    }
    assert!(
        app.deploy_rate_limiter
            .allow_cost("expected-token", 8)
            .await,
        "duplicate authentication must reject before consuming rate budget"
    );
}

#[test]
fn sccp_submit_content_type_accepts_only_unambiguous_utf8_application_json() {
    for value in [
        "application/json",
        "APPLICATION/JSON",
        "application/json; charset=utf-8",
        "application/json; charset=\"UTF-8\"",
    ] {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_str(value).expect("valid header fixture"),
        );
        assert_eq!(super::validate_sccp_submit_content_type(&headers), Ok(()));
    }
}

#[tokio::test]
async fn sccp_submit_content_type_rejects_before_rate_body_and_admission() {
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests_with_options(None, Some((1, 8)), None, None);
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.query_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        state.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        state.query_queue_timeout = Duration::ZERO;
    }
    let router = sccp_ingress_test_router(Arc::clone(&app));

    let cases = [
        (None, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (
            Some("application/octet-stream"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            Some("application/problem+json"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (Some("   "), StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (
            Some("application/json; charset="),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            Some("application/json; charset=\"\""),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            Some("application/json; charset=latin1"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            Some("application/json; profile=x"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (Some("application/json;"), StatusCode::BAD_REQUEST),
        (
            Some("application/json;;charset=utf-8"),
            StatusCode::BAD_REQUEST,
        ),
        (
            Some("application/json; charset=utf-8; charset=utf-8"),
            StatusCode::BAD_REQUEST,
        ),
        (
            Some("application/json; charset=\"utf-8"),
            StatusCode::BAD_REQUEST,
        ),
        (
            Some("application/json;\tcharset=utf-8"),
            StatusCode::BAD_REQUEST,
        ),
    ];
    for (content_type, expected) in cases {
        let mut request = sccp_ingress_request(
            "/v1/bridge/proofs/submit",
            sccp_body_that_must_not_be_polled(),
        );
        request
            .headers_mut()
            .remove(axum::http::header::CONTENT_TYPE);
        if let Some(content_type) = content_type {
            request.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_str(content_type).expect("representable adversarial header"),
            );
        }
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("content-type response");
        assert_eq!(response.status(), expected, "content type {content_type:?}");
    }

    let mut duplicate = sccp_ingress_request(
        "/v1/bridge/proofs/submit",
        sccp_body_that_must_not_be_polled(),
    );
    duplicate.headers_mut().append(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let response = router
        .oneshot(duplicate)
        .await
        .expect("duplicate content-type response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        app.deploy_rate_limiter
            .allow_cost("127.0.0.1", super::FINALITY_HEAVY_QUERY_RATE_COST)
            .await,
        "invalid content types must reject before consuming rate budget"
    );
}

#[tokio::test]
async fn sccp_submit_wrong_methods_bypass_proof_admission_and_body_polling() {
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests_with_options(None, Some((1, 8)), None, None);
    let app_mut = Arc::get_mut(&mut app).expect("unique app state");
    app_mut.query_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    app_mut.query_queue_timeout = Duration::ZERO;
    let headers = HeaderMap::new();
    let remote_ip = std::net::IpAddr::from([127, 0, 0, 1]);
    let policy =
        super::sccp_submit_ingress_policy("/v1/bridge/proofs/submit").expect("known policy");
    let key = super::rate_limit_key(
        &headers,
        Some(remote_ip),
        policy.rate_limit_hint,
        app.api_token_enforced(),
    );
    let router = sccp_ingress_test_router(Arc::clone(&app));

    for method in [axum::http::Method::GET, axum::http::Method::PUT] {
        let mut request = sccp_ingress_request(
            "/v1/bridge/proofs/submit",
            sccp_body_that_must_not_be_polled(),
        );
        *request.method_mut() = method;
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("wrong-method response");
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    assert!(
        app.deploy_rate_limiter
            .allow_cost(&key, policy.rate_limit_cost)
            .await,
        "wrong methods must not consume the SCCP proof-rate budget"
    );
    assert!(
        !app.deploy_rate_limiter.allow(&key).await,
        "the exact weighted request must consume the full enabled burst"
    );
}

#[tokio::test]
async fn sccp_submit_ingress_rejects_exhausted_rate_limit_without_polling_body() {
    use tower::ServiceExt as _;

    let app = mk_app_state_for_tests_with_options(None, Some((1, 1)), None, None);
    let headers = HeaderMap::new();
    let remote_ip = std::net::IpAddr::from([127, 0, 0, 1]);
    let policies = ["/v1/bridge/proofs/submit", "/v1/bridge/messages"]
        .map(|path| super::sccp_submit_ingress_policy(path).expect("known policy"));
    let rate_limit_keys = policies.map(|policy| {
        super::rate_limit_key(
            &headers,
            Some(remote_ip),
            policy.rate_limit_hint,
            app.api_token_enforced(),
        )
    });
    assert_eq!(
        rate_limit_keys[0], rate_limit_keys[1],
        "identified callers must share one deploy-rate bucket across SCCP submit routes"
    );
    assert!(
        app.deploy_rate_limiter
            .allow_cost_capped_to_burst(&rate_limit_keys[0], policies[0].rate_limit_cost)
            .await
    );
    let router = sccp_ingress_test_router(app);

    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let mut request = sccp_ingress_request(path, sccp_body_that_must_not_be_polled());
        request.headers_mut().insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from_static("not-a-length"),
        );
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("middleware response");
        assert_eq!(
            response.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "path {path}"
        );
    }
}

#[tokio::test]
async fn sccp_submit_ingress_rejects_exhausted_body_gate_without_polling() {
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .proof_body_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    let router = sccp_ingress_test_router(app);

    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let response = router
            .clone()
            .oneshot(sccp_ingress_request(
                path,
                sccp_body_that_must_not_be_polled(),
            ))
            .await
            .expect("body-gate response");
        assert_eq!(
            response.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "path {path}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_sccp_body_times_out_without_reserving_query_capacity() {
    use std::task::Poll;
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique app state");
    app_mut.proof_body_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app_mut.proof_limits.body_read_timeout = Duration::from_millis(250);
    app_mut.query_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app_mut.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(1));
    app_mut.query_queue_timeout = Duration::ZERO;
    let body_gate = Arc::clone(&app_mut.proof_body_inflight);
    let query = Arc::clone(&app_mut.query_inflight);
    let heavy = Arc::clone(&app_mut.query_heavy_inflight);
    let router = sccp_ingress_test_router(app);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let mut started_tx = Some(started_tx);
    let stream = futures::stream::poll_fn(
        move |_context| -> Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            if let Some(started_tx) = started_tx.take() {
                let _ = started_tx.send(());
            }
            Poll::Pending
        },
    );
    let request = sccp_ingress_request(
        "/v1/bridge/proofs/submit",
        axum::body::Body::from_stream(stream),
    );
    let response_task = tokio::spawn(router.oneshot(request));
    started_rx.await.expect("stalled body was polled");

    assert_eq!(body_gate.available_permits(), 0);
    let query_permit = query
        .clone()
        .try_acquire_owned()
        .expect("network body read must not reserve general query capacity");
    let heavy_permit = heavy
        .clone()
        .try_acquire_owned()
        .expect("network body read must not reserve heavy query capacity");
    drop((query_permit, heavy_permit));

    let response = response_task
        .await
        .expect("body timeout task")
        .expect("middleware response");
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    assert_eq!(body_gate.available_permits(), 1);
    assert_eq!(query.available_permits(), 1);
    assert_eq!(heavy.available_permits(), 1);
}

#[tokio::test]
async fn sccp_submit_ingress_checks_heavy_admission_after_bounded_body_collection() {
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests();
    let state = Arc::get_mut(&mut app).expect("unique app state");
    state.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    state.query_queue_timeout = Duration::ZERO;
    let router = sccp_ingress_test_router(app);

    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let yielded = Arc::new(AtomicUsize::new(0));
        let yielded_for_stream = Arc::clone(&yielded);
        let stream = futures::stream::once(async move {
            yielded_for_stream.fetch_add(1, Ordering::SeqCst);
            Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"{}"))
        });
        let request = sccp_ingress_request(path, axum::body::Body::from_stream(stream));
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("middleware response");
        assert_eq!(
            response.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "path {path}"
        );
        assert_eq!(
            yielded.load(Ordering::SeqCst),
            1,
            "heavy admission must be acquired only after the bounded body is complete"
        );
    }
}

#[tokio::test]
async fn sccp_submit_weight_is_capped_to_burst_and_consumes_the_full_budget() {
    use tower::ServiceExt as _;

    let mut app = mk_app_state_for_tests_with_options(None, Some((1, 7)), None, None);
    let state = Arc::get_mut(&mut app).expect("unique app state");
    state.query_heavy_inflight = Arc::new(tokio::sync::Semaphore::new(0));
    state.query_queue_timeout = Duration::ZERO;
    let router = sccp_ingress_test_router(app);
    let yielded = Arc::new(AtomicUsize::new(0));
    let yielded_for_stream = Arc::clone(&yielded);
    let stream = futures::stream::once(async move {
        yielded_for_stream.fetch_add(1, Ordering::SeqCst);
        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"{}"))
    });
    let first = router
        .clone()
        .oneshot(sccp_ingress_request(
            "/v1/bridge/proofs/submit",
            axum::body::Body::from_stream(stream),
        ))
        .await
        .expect("capped-cost response");
    assert_eq!(first.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(yielded.load(Ordering::SeqCst), 1);

    let second = router
        .oneshot(sccp_ingress_request(
            "/v1/bridge/messages",
            sccp_body_that_must_not_be_polled(),
        ))
        .await
        .expect("exhausted-cost response");
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn sccp_submit_ingress_rejects_oversized_content_length_without_polling_body() {
    use tower::ServiceExt as _;

    let router = sccp_ingress_test_router(mk_app_state_for_tests());
    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let policy = super::sccp_submit_ingress_policy(path).expect("known policy");
        let mut request = sccp_ingress_request(path, sccp_body_that_must_not_be_polled());
        request.headers_mut().insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from_str(&policy.max_body_bytes.saturating_add(1).to_string())
                .expect("valid oversized content length"),
        );
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("middleware response");
        assert_eq!(
            response.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "path {path}"
        );
    }
}

#[tokio::test]
async fn sccp_submit_ingress_rejects_malformed_or_ambiguous_length_without_polling_body() {
    use tower::ServiceExt as _;

    let router = sccp_ingress_test_router(mk_app_state_for_tests());
    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        for case in 0..3 {
            let mut request = sccp_ingress_request(path, sccp_body_that_must_not_be_polled());
            match case {
                0 => {
                    request.headers_mut().insert(
                        axum::http::header::CONTENT_LENGTH,
                        HeaderValue::from_static("1x"),
                    );
                }
                1 => {
                    request.headers_mut().append(
                        axum::http::header::CONTENT_LENGTH,
                        HeaderValue::from_static("1"),
                    );
                    request.headers_mut().append(
                        axum::http::header::CONTENT_LENGTH,
                        HeaderValue::from_static("1"),
                    );
                }
                2 => {
                    request.headers_mut().insert(
                        axum::http::header::CONTENT_LENGTH,
                        HeaderValue::from_static("1"),
                    );
                    request.headers_mut().insert(
                        axum::http::header::TRANSFER_ENCODING,
                        HeaderValue::from_static("chunked"),
                    );
                }
                _ => unreachable!(),
            }
            let response = router
                .clone()
                .oneshot(request)
                .await
                .expect("middleware response");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "path {path}");
        }
    }
}

#[tokio::test]
async fn sccp_submit_ingress_treats_overflowing_numeric_length_as_too_large() {
    use tower::ServiceExt as _;

    let router = sccp_ingress_test_router(mk_app_state_for_tests());
    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let mut request = sccp_ingress_request(path, sccp_body_that_must_not_be_polled());
        request.headers_mut().insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from_static("9999999999999999999999999999999999999999"),
        );
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("middleware response");
        assert_eq!(
            response.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "path {path}"
        );
    }
}

#[tokio::test]
async fn sccp_submit_ingress_honors_stricter_operator_body_limit() {
    use tower::ServiceExt as _;

    let router = sccp_ingress_test_router_with_limit(mk_app_state_for_tests(), 8);
    let mut request =
        sccp_ingress_request("/v1/bridge/messages", sccp_body_that_must_not_be_polled());
    request.headers_mut().insert(
        axum::http::header::CONTENT_LENGTH,
        HeaderValue::from_static("9"),
    );
    let response = router.oneshot(request).await.expect("middleware response");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn sccp_submit_ingress_accepts_exact_effective_cap_and_rejects_actual_overage() {
    use tower::ServiceExt as _;

    let router = sccp_ingress_test_router_with_limit(mk_app_state_for_tests(), 8);
    for path in ["/v1/bridge/proofs/submit", "/v1/bridge/messages"] {
        let mut exact = sccp_ingress_request(path, axum::body::Body::from("12345678"));
        exact.headers_mut().insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from_static("8"),
        );
        let response = router
            .clone()
            .oneshot(exact)
            .await
            .expect("exact-cap middleware response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT, "path {path}");

        let over = sccp_ingress_request(path, axum::body::Body::from("123456789"));
        let response = router
            .clone()
            .oneshot(over)
            .await
            .expect("over-cap middleware response");
        assert_eq!(
            response.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "path {path}"
        );
    }
}

#[tokio::test]
async fn sccp_submit_ingress_rejects_lying_content_length_with_correct_status() {
    use tower::ServiceExt as _;

    let router = sccp_ingress_test_router_with_limit(mk_app_state_for_tests(), 8);
    for (body, declared, expected) in [
        ("12345", "4", StatusCode::BAD_REQUEST),
        ("12345", "6", StatusCode::BAD_REQUEST),
        ("123456789", "4", StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        let mut request = sccp_ingress_request("/v1/bridge/messages", axum::body::Body::from(body));
        request.headers_mut().insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from_str(declared).expect("valid adversarial length"),
        );
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("lying-length middleware response");
        assert_eq!(response.status(), expected, "declared {declared}");
    }
}

#[tokio::test]
async fn sccp_submit_ingress_preserves_missing_and_chunked_bodies_after_one_consumption() {
    use tower::ServiceExt as _;

    let router = sccp_ingress_echo_router_with_limit(mk_app_state_for_tests(), 16);
    for (path, chunked) in [
        ("/v1/bridge/proofs/submit", false),
        ("/v1/bridge/messages", true),
    ] {
        let yielded = Arc::new(AtomicUsize::new(0));
        let yielded_for_stream = Arc::clone(&yielded);
        let chunks = [
            axum::body::Bytes::from_static(b"abc"),
            axum::body::Bytes::from_static(b"defgh"),
        ];
        let mut index = 0_usize;
        let stream = futures::stream::poll_fn(move |_context| {
            if let Some(chunk) = chunks.get(index).cloned() {
                index += 1;
                yielded_for_stream.fetch_add(1, Ordering::SeqCst);
                std::task::Poll::Ready(Some(Ok::<_, std::io::Error>(chunk)))
            } else {
                std::task::Poll::Ready(None)
            }
        });
        let mut request = sccp_ingress_request(path, axum::body::Body::from_stream(stream));
        if chunked {
            request.headers_mut().insert(
                axum::http::header::TRANSFER_ENCODING,
                HeaderValue::from_static("chunked"),
            );
        }
        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("body-preservation middleware response");
        assert_eq!(response.status(), StatusCode::OK, "path {path}");
        let body = axum::body::to_bytes(response.into_body(), 16)
            .await
            .expect("echo response body");
        assert_eq!(body.as_ref(), b"abcdefgh", "path {path}");
        assert_eq!(yielded.load(Ordering::SeqCst), 2, "path {path}");
    }
}

#[tokio::test]
async fn sccp_submit_ingress_maps_transport_failure_to_bad_request() {
    use tower::ServiceExt as _;

    let stream = futures::stream::iter([Err::<axum::body::Bytes, _>(std::io::Error::other(
        "adversarial stream failure",
    ))]);
    let request =
        sccp_ingress_request("/v1/bridge/messages", axum::body::Body::from_stream(stream));
    let response = sccp_ingress_test_router_with_limit(mk_app_state_for_tests(), 64)
        .oneshot(request)
        .await
        .expect("transport-failure middleware response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn openapi_enforces_token_policy() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        let mut tokens = HashSet::new();
        tokens.insert("token-123".to_owned());
        state.api_tokens_set = Arc::new(tokens);
    }

    let headers = HeaderMap::new();
    let missing = super::handler_openapi(
        State(app.clone()),
        headers.clone(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
    )
    .await;
    assert!(matches!(
        missing,
        Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted(_)
        ))
    ));

    let mut headers_with_token = HeaderMap::new();
    headers_with_token.insert("x-api-token", HeaderValue::from_static("token-123"));
    let ok = super::handler_openapi(
        State(app),
        headers_with_token,
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
    )
    .await
    .expect("token accepted");
    assert_eq!(ok.status(), axum::http::StatusCode::OK);
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn connect_session_handler_rejects_missing_remote_addr_header() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};

    let app = mk_app_state_for_tests();
    let req = routing::ConnectSessionRequest {
        sid: Some(B64.encode([0x42_u8; 32])),
        node: None,
    };
    let err =
        match super::handler_connect_session(State(app), HeaderMap::new(), NoritoJson(req)).await {
            Ok(_) => panic!("missing remote addr should fail closed"),
            Err(err) => err,
        };
    assert!(matches!(
        err,
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message)
        )) if message == "connect: remote addr unavailable"
    ));
}

#[tokio::test]
async fn soracloud_status_handler_returns_snapshot_sections() {
    let mut app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(TestSoracloudRuntimeHandle {
        snapshot: sample_soracloud_runtime_snapshot(
            iroha_data_model::soracloud::SoraServiceHealthStatusV1::Healthy,
        ),
        state_dir: PathBuf::from("/tmp/soracloud/runtime"),
        local_peer_id: None,
    }));

    let response = super::handler_soracloud_status(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("soracloud status should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("decode JSON response");

    assert_eq!(
        payload
            .get("schema_version")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    assert!(
        payload
            .get("service_health")
            .and_then(norito::json::Value::as_object)
            .is_some(),
        "service_health section should be present"
    );
    assert!(
        payload
            .get("routing")
            .and_then(norito::json::Value::as_object)
            .is_some(),
        "routing section should be present"
    );
    assert!(
        payload
            .get("hosted_http_topology")
            .and_then(norito::json::Value::as_object)
            .is_some(),
        "hosted_http_topology section should be present"
    );
    assert!(
        payload
            .get("resource_pressure")
            .and_then(norito::json::Value::as_object)
            .is_some(),
        "resource_pressure section should be present"
    );
    assert!(
        payload
            .get("failed_admissions")
            .and_then(norito::json::Value::as_object)
            .is_some(),
        "failed_admissions section should be present"
    );
    assert_eq!(
        payload
            .get("failed_admissions")
            .and_then(norito::json::Value::as_object)
            .and_then(|value| value.get("available"))
            .and_then(norito::json::Value::as_bool),
        Some(cfg!(feature = "telemetry"))
    );
    assert!(
        payload
            .get("control_plane")
            .and_then(norito::json::Value::as_object)
            .is_some(),
        "control_plane section should be present"
    );
    assert_eq!(
        payload
            .get("service_health")
            .and_then(norito::json::Value::as_object)
            .and_then(|value| value.get("mode"))
            .and_then(norito::json::Value::as_str),
        Some("embedded_runtime_manager")
    );
    assert!(
        payload
            .get("runtime_manager")
            .and_then(norito::json::Value::as_object)
            .is_some(),
        "runtime_manager section should be present"
    );
}

#[tokio::test]
async fn soracloud_status_routing_counts_only_active_autoscale_capacity_lanes() {
    let mut app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    let future_lane = LaneId::new(1);
    let mut future_autoscale_lane = iroha_data_model::nexus::LaneConfig {
        id: future_lane,
        alias: "elastic-lane-1".to_owned(),
        visibility: iroha_data_model::nexus::LaneVisibility::Public,
        ..iroha_data_model::nexus::LaneConfig::default()
    };
    future_autoscale_lane.metadata.insert(
        iroha_data_model::nexus::AUTOSCALE_META_MANAGED.to_owned(),
        "true".to_owned(),
    );
    future_autoscale_lane.metadata.insert(
        iroha_data_model::nexus::AUTOSCALE_META_CREATED_HEIGHT.to_owned(),
        "7".to_owned(),
    );
    let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
        NonZeroU32::new(2).expect("nonzero lane count"),
        vec![
            iroha_data_model::nexus::LaneConfig::default(),
            future_autoscale_lane,
        ],
    )
    .expect("future-created autoscale lane catalog");
    let mut nexus = iroha_config::parameters::actual::Nexus {
        enabled: true,
        lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog),
        lane_catalog,
        ..iroha_config::parameters::actual::Nexus::default()
    };
    nexus.autoscale.enabled = true;
    nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min lanes");
    nexus.autoscale.max_lanes = NonZeroU32::new(2).expect("nonzero max lanes");
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        {
            let mut current = state.nexus.write();
            *current = nexus;
        }
        state.update_latest_block_header_cache_for_tests(BlockHeader::new(
            NonZeroU64::new(1).expect("nonzero height"),
            None,
            None,
            None,
            0,
            0,
        ));
    }

    let response = super::handler_soracloud_status(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("soracloud status should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("decode JSON response");
    let routing = payload
        .get("routing")
        .and_then(norito::json::Value::as_object)
        .expect("routing section");

    assert_eq!(
        routing
            .get("configured_lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        routing
            .get("lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(2),
        "legacy lane_count remains the configured lane count"
    );
    assert_eq!(
        routing
            .get("declared_lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(2),
        "declared lane count reports catalog metadata entries"
    );
    assert_eq!(
        routing
            .get("active_lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(1),
        "future-created autoscale lanes must not count as active"
    );
    assert_eq!(
        routing
            .get("active_lane_ids")
            .and_then(norito::json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(norito::json::Value::as_u64)
                    .collect::<Vec<_>>()
            }),
        Some(vec![0])
    );
    assert_eq!(
        routing
            .get("autoscale_capacity_lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(0),
        "future-created autoscale lanes must not count as live capacity"
    );
    assert!(
        routing
            .get("autoscale_capacity_lane_ids")
            .and_then(norito::json::Value::as_array)
            .is_some_and(Vec::is_empty),
        "future-created autoscale lanes must be absent from capacity ids"
    );
}

#[tokio::test]
async fn soracloud_status_routing_reports_sparse_configured_lane_namespace() {
    let mut app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
    let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
        NonZeroU32::new(4).expect("nonzero lane namespace"),
        vec![iroha_data_model::nexus::LaneConfig::default()],
    )
    .expect("sparse lane catalog");
    let nexus = iroha_config::parameters::actual::Nexus {
        enabled: true,
        lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog),
        lane_catalog,
        ..iroha_config::parameters::actual::Nexus::default()
    };
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        {
            let mut current = state.nexus.write();
            *current = nexus;
        }
        state.update_latest_block_header_cache_for_tests(BlockHeader::new(
            NonZeroU64::new(1).expect("nonzero height"),
            None,
            None,
            None,
            0,
            0,
        ));
    }

    let response = super::handler_soracloud_status(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("soracloud status should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let payload: norito::json::Value =
        norito::json::from_slice(&body).expect("decode JSON response");
    let routing = payload
        .get("routing")
        .and_then(norito::json::Value::as_object)
        .expect("routing section");

    assert_eq!(
        routing
            .get("configured_lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(4),
        "configured count must report the lane namespace size"
    );
    assert_eq!(
        routing
            .get("lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(4),
        "legacy lane_count remains the configured namespace count"
    );
    assert_eq!(
        routing
            .get("declared_lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(1),
        "declared count reports only catalog metadata entries"
    );
    assert_eq!(
        routing
            .get("active_lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        routing
            .get("active_lane_ids")
            .and_then(norito::json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(norito::json::Value::as_u64)
                    .collect::<Vec<_>>()
            }),
        Some(vec![0])
    );
    assert_eq!(
        routing
            .get("autoscale_capacity_lane_count")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert!(
        routing
            .get("autoscale_capacity_lane_ids")
            .and_then(norito::json::Value::as_array)
            .is_some_and(Vec::is_empty)
    );
}

#[test]
fn soracloud_hosted_http_topology_section_reports_authoritative_counts() {
    let mut world = seed_public_soracloud_world();
    let validator_two = checked_torii_test_account_id(
        0x7d,
        "derive hosted HTTP topology second validator fixture key",
    );
    let validator_three = checked_torii_test_account_id(
        0x7e,
        "derive hosted HTTP topology third validator fixture key",
    );

    world
        .soracloud_inrou_host_capabilities_mut_for_testing()
        .insert(
            ALICE_ID.clone(),
            iroha_data_model::soracloud::SoraInrouHostCapabilityRecordV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
                validator_account_id: ALICE_ID.clone(),
                peer_id: "12D3KooWTopologyHostPortable".to_owned(),
                supported_backends: std::collections::BTreeSet::from([
                    iroha_data_model::soracloud::SoraInrouRuntimeBackendV1::PortableVm,
                ]),
                supported_guest_isas: std::collections::BTreeSet::from([
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                ]),
                max_hosted_replica_capacity: 2,
                max_cpu_millis: 2_000,
                max_memory_bytes: 2 * 1024 * 1024 * 1024,
                max_storage_bytes: 16 * 1024 * 1024 * 1024,
                proxy_only: false,
                geography_tags: Default::default(),
                observed_latency_ms: None,
                advertised_at_ms: 1,
                heartbeat_expires_at_ms: u64::MAX,
            },
        );
    world
        .soracloud_inrou_host_capabilities_mut_for_testing()
        .insert(
            validator_two.clone(),
            iroha_data_model::soracloud::SoraInrouHostCapabilityRecordV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
                validator_account_id: validator_two.clone(),
                peer_id: "12D3KooWTopologyHostKvm".to_owned(),
                supported_backends: std::collections::BTreeSet::from([
                    iroha_data_model::soracloud::SoraInrouRuntimeBackendV1::FirecrackerKvm,
                ]),
                supported_guest_isas: std::collections::BTreeSet::from([
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                ]),
                max_hosted_replica_capacity: 1,
                max_cpu_millis: 1_000,
                max_memory_bytes: 1024 * 1024 * 1024,
                max_storage_bytes: 8 * 1024 * 1024 * 1024,
                proxy_only: false,
                geography_tags: Default::default(),
                observed_latency_ms: None,
                advertised_at_ms: 1,
                heartbeat_expires_at_ms: u64::MAX,
            },
        );
    world
        .soracloud_inrou_host_capabilities_mut_for_testing()
        .insert(
            validator_three.clone(),
            iroha_data_model::soracloud::SoraInrouHostCapabilityRecordV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
                validator_account_id: validator_three,
                peer_id: "12D3KooWTopologyProxyOnly".to_owned(),
                supported_backends: std::collections::BTreeSet::from([
                    iroha_data_model::soracloud::SoraInrouRuntimeBackendV1::PortableVm,
                ]),
                supported_guest_isas: std::collections::BTreeSet::from([
                    iroha_data_model::soracloud::SoraInrouGuestIsaV1::Aarch64,
                ]),
                max_hosted_replica_capacity: 0,
                max_cpu_millis: 0,
                max_memory_bytes: 0,
                max_storage_bytes: 0,
                proxy_only: true,
                geography_tags: Default::default(),
                observed_latency_ms: None,
                advertised_at_ms: 1,
                heartbeat_expires_at_ms: u64::MAX,
            },
        );
    world
        .soracloud_inrou_service_placements_mut_for_testing()
        .insert(
            ("web_portal".to_owned(), "2026.02.0".to_owned()),
            iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                service_name: "web_portal".parse().expect("service"),
                service_version: "2026.02.0".to_owned(),
                desired_replica_count: 2,
                eligible_validator_count: 2,
                placements: vec![
                    iroha_data_model::soracloud::SoraInrouReplicaPlacementV1 {
                        replica_slot: 1,
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: "12D3KooWTopologyHostPortable".to_owned(),
                        selected_backend:
                            iroha_data_model::soracloud::SoraInrouRuntimeBackendV1::PortableVm,
                        selected_guest_isa: iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                        selected_geography_tag: None,
                        selection_latency_ms: None,
                    },
                    iroha_data_model::soracloud::SoraInrouReplicaPlacementV1 {
                        replica_slot: 2,
                        validator_account_id: validator_two,
                        peer_id: "12D3KooWTopologyHostKvm".to_owned(),
                        selected_backend:
                            iroha_data_model::soracloud::SoraInrouRuntimeBackendV1::FirecrackerKvm,
                        selected_guest_isa: iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                        selected_geography_tag: None,
                        selection_latency_ms: None,
                    },
                ],
                reconciled_at_ms: 1,
                last_error: None,
            },
        );

    let app = mk_app_state_for_tests_with_world(world);
    let topology = super::soracloud_hosted_http_topology_section(&app);

    assert_eq!(
        topology
            .get("active_capability_adverts")
            .and_then(norito::json::Value::as_u64),
        Some(3)
    );
    assert_eq!(
        topology
            .get("placed_host_count")
            .and_then(norito::json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        topology
            .get("hosted_replica_count")
            .and_then(norito::json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        topology
            .get("proxy_only_validator_count")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    let backend_mix = topology
        .get("backend_mix")
        .and_then(norito::json::Value::as_object)
        .expect("backend mix object");
    assert_eq!(
        backend_mix
            .get("portable_vm")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        backend_mix
            .get("firecracker_kvm")
            .and_then(norito::json::Value::as_u64),
        Some(1)
    );
}

#[tokio::test]
async fn soracloud_runtime_status_sections_report_degraded_for_hydrating_snapshots() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .soracloud_runtime = Some(Arc::new(TestSoracloudRuntimeHandle {
        snapshot: sample_soracloud_runtime_snapshot(
            iroha_data_model::soracloud::SoraServiceHealthStatusV1::Hydrating,
        ),
        state_dir: PathBuf::from("/tmp/soracloud/runtime"),
        local_peer_id: None,
    }));

    let (service_health, _runtime_pressure, runtime_manager) =
        super::soracloud_runtime_status_sections(&app);

    assert_eq!(
        service_health
            .get("status")
            .and_then(norito::json::Value::as_str),
        Some("degraded")
    );
    assert_eq!(
        runtime_manager
            .get("available")
            .and_then(norito::json::Value::as_bool),
        Some(true)
    );
}

#[tokio::test]
async fn soracloud_runtime_status_sections_report_unavailable_without_runtime() {
    let app = mk_app_state_for_tests();
    let (service_health, runtime_pressure, runtime_manager) =
        super::soracloud_runtime_status_sections(&app);

    assert_eq!(
        service_health
            .get("status")
            .and_then(norito::json::Value::as_str),
        Some("unavailable")
    );
    assert_eq!(
        runtime_pressure
            .get("enabled")
            .and_then(norito::json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        runtime_manager
            .get("available")
            .and_then(norito::json::Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn proof_rate_limit_sets_retry_after_header() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.proof_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        state.proof_limits = routing::ProofApiLimits::new(
            10,
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(3),
            iroha_config::parameters::defaults::torii::PROOF_MAX_BODY_BYTES.get(),
            std::time::Duration::from_millis(
                iroha_config::parameters::defaults::torii::PROOF_BODY_READ_TIMEOUT_MS,
            ),
        );
    }

    let headers = HeaderMap::new();
    let q = crate::NoritoQuery(routing::ProofListQuery {
        limit: Some(1),
        ..Default::default()
    });

    let first = super::handler_list_proofs(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
        q.clone(),
    )
    .await
    .expect("first request allowed")
    .into_response();
    assert_eq!(first.status(), axum::http::StatusCode::OK);

    let resp = match super::handler_list_proofs(
        State(app.clone()),
        headers,
        crate::loopback_connect_info(),
        q,
    )
    .await
    {
        Ok(_) => panic!("second request should be throttled"),
        Err(err) => err.into_response(),
    };
    assert_eq!(resp.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
    let retry_after = resp.headers().get(axum::http::header::RETRY_AFTER);
    assert_eq!(retry_after.and_then(|h| h.to_str().ok()), Some("3"));
}

#[tokio::test]
async fn policy_rate_limit_keys_transport_remote_when_internal_header_missing() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .rate_limiter = limits::RateLimiter::new(Some(1), Some(1));

    let first = super::handler_policy(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([198, 51, 100, 10], 0))),
    )
    .await
    .expect("first policy request should be allowed")
    .into_response();
    assert_eq!(first.status(), StatusCode::OK);

    let second = super::handler_policy(
        State(app),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([198, 51, 100, 11], 0))),
    )
    .await
    .expect("second policy request from a different remote should use a separate bucket")
    .into_response();
    assert_eq!(second.status(), StatusCode::OK);
}

#[cfg(feature = "profiling")]
#[tokio::test]
async fn profiling_enforces_token_policy() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        let mut tokens = HashSet::new();
        tokens.insert("token-456".to_owned());
        state.api_tokens_set = Arc::new(tokens);
    }

    let params: routing::profiling::ProfileParams =
        norito::json::from_value(norito::json!({})).expect("defaults");

    let headers = HeaderMap::new();
    let missing = super::handler_profile(
        State(app.clone()),
        headers.clone(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        AxQuery(params),
    )
    .await;
    assert!(matches!(
        missing,
        Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
        )))
    ));

    let mut headers_with_token = HeaderMap::new();
    headers_with_token.insert("x-api-token", HeaderValue::from_static("token-456"));
    let ok = super::handler_profile(
        State(app),
        headers_with_token,
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        AxQuery(norito::json::from_value(norito::json!({})).expect("defaults for second run")),
    )
    .await
    .expect("token accepted");
    assert!(!ok.is_empty());
}

#[cfg(feature = "schema")]
#[tokio::test]
async fn schema_handler_ok() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();
    let resp = super::handler_schema(
        State(app),
        headers,
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn telemetry_handlers_ok() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();

    // Status root
    let resp = super::handler_status_root(
        State(app.clone()),
        headers.clone(),
        None,
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    // Status tail
    let resp = super::handler_status_tail(
        State(app.clone()),
        headers.clone(),
        None,
        axum::extract::Path("peers".to_string()),
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    // Metrics
    let text = super::handler_metrics(
        State(app.clone()),
        headers.clone(),
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
    )
    .await
    .expect("ok");
    assert!(!text.is_empty());

    let resp = super::handler_sumeragi_phases(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    // QC and leader endpoints
    let resp = super::handler_sumeragi_qc(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    app.state.metrics().set_axt_proof_cache_state(
        iroha_data_model::nexus::DataSpaceId::new(1),
        "miss",
        [0x11; 32],
        2,
        Some(5),
    );
    app.state.metrics().set_axt_reject_hint(
        DataSpaceId::new(1),
        LaneId::new(7),
        10,
        2,
        AxtRejectReason::HandleEra,
    );
    let resp = super::handler_debug_axt_cache(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read body");
    let body = String::from_utf8(body.to_vec()).expect("utf8 body");
    let body_json: norito::json::Value = norito::json::from_str(&body).expect("cache status json");
    let entries = body_json
        .get("entries")
        .and_then(norito::json::Value::as_array)
        .expect("entries array");
    assert!(
        entries.iter().any(|entry| {
            entry.get("status").and_then(norito::json::Value::as_str) == Some("miss")
        }),
        "cache status response should include miss entry"
    );
    assert!(
        body_json
            .get("snapshot_version")
            .and_then(norito::json::Value::as_u64)
            .is_some(),
        "cache status response should include snapshot version"
    );
    let hints = body_json
        .get("reject_hints")
        .and_then(norito::json::Value::as_array)
        .expect("reject_hints array");
    assert_eq!(hints.len(), 1, "reject hints should include dsid=1");
    let hint = &hints[0];
    assert_eq!(
        hint.get("dataspace").and_then(norito::json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        hint.get("target_lane")
            .and_then(norito::json::Value::as_u64),
        Some(7)
    );
    assert_eq!(
        hint.get("active_handle_era")
            .and_then(norito::json::Value::as_u64),
        Some(10)
    );
    assert_eq!(
        hint.get("next_handle_counter")
            .and_then(norito::json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        hint.get("reason").and_then(norito::json::Value::as_str),
        Some(AxtRejectReason::HandleEra.label())
    );

    let resp = super::handler_sumeragi_leader(State(app), headers, crate::loopback_connect_info())
        .await
        .expect("ok")
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn telemetry_commit_qc_null_on_empty() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();
    let sample_hash = format!("{}", iroha_crypto::Hash::new(b"torii-telemetry-test"));

    let resp = super::handler_commit_qc(
        State(app.clone()),
        headers,
        crate::loopback_connect_info(),
        None,
        axum::extract::Path(sample_hash),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert!(v.get("subject_block_hash").is_some());
    assert!(v.get("commit_qc").is_some());
    let resp = super::handler_commit_qc(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        )),
        axum::extract::Path(format!(
            "{}",
            iroha_crypto::Hash::new(b"torii-telemetry-test")
        )),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let decoded_opt: Option<Qc> =
        norito::decode_from_bytes(&bytes).expect("decode commit_qc norito");
    assert!(decoded_opt.is_none());
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn telemetry_params_and_bls_keys_ok() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();

    let resp = super::handler_sumeragi_params(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    let resp =
        super::handler_sumeragi_bls_keys(State(app), headers, crate::loopback_connect_info())
            .await
            .expect("ok")
            .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
async fn app_api_vk_and_proofs_lists_ok() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();

    // List VKs
    let resp = super::handler_list_vk(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
        AxQuery(crate::routing::VkListQuery::default()),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    // List proofs
    let resp = super::handler_list_proofs(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
        AxQuery(crate::routing::ProofListQuery::default()),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    // Count proofs
    let resp = super::handler_count_proofs(
        State(app.clone()),
        headers,
        crate::loopback_connect_info(),
        AxQuery(crate::routing::ProofListQuery::default()),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
async fn app_api_get_by_id_not_found_returns_404() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();

    // Contract code by hash (non-existent)
    let resp = super::handler_get_contract_code(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
        axum::extract::Path(
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        ),
    )
    .await
    .expect("ok mapping")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);

    // VK by backend/name (non-existent)
    let resp = super::handler_get_vk_by_backend_name(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
        axum::extract::Path(("halo2/ipa".to_string(), "demo".to_string())),
    )
    .await
    .expect("ok mapping")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);

    // Proof by backend/hash (non-existent)
    let resp = super::handler_get_proof_by_backend_hash(
        State(app.clone()),
        headers,
        crate::loopback_connect_info(),
        axum::extract::Path((
            "halo2/ipa".to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        )),
    )
    .await
    .expect("ok mapping")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn retired_server_contract_deploy_routes_are_absent() {
    use axum::{
        body::Body,
        extract::ConnectInfo,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt as _;

    let cfg = crate::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(IrohaState::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
        capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };
    let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let torii = Torii::new_with_handle(
        ChainId::from("contracts-router-test"),
        signed_query_test_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        routing::MaybeTelemetry::disabled(),
    );

    for (method, path) in [
        (Method::POST, "/v1/contracts/deploy"),
        (Method::POST, "/v1/contracts/deploy-bundle"),
        (
            Method::GET,
            "/v1/contracts/deploy-bundles/retired-bundle-digest",
        ),
    ] {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::empty())
            .expect("request");
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));

        let response = torii
            .api_router_for_tests()
            .oneshot(request)
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND, "path {path}");
    }
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn retired_storage_pin_route_cannot_mutate_chain_or_local_storage() {
    use axum::{
        body::Body,
        extract::ConnectInfo,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt as _;

    let cfg = crate::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(IrohaState::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
        capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };
    let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let storage_dir = tempfile::tempdir().expect("storage tempdir");
    let sorafs_node = sorafs_node::NodeHandle::new(
        sorafs_node::config::StorageConfig::builder()
            .enabled(true)
            .data_dir(storage_dir.path().join("storage"))
            .build(),
    );
    let storage = sorafs_node.storage().expect("enabled storage");
    assert_eq!(storage.manifest_count(), 0);
    assert_eq!(state.view().world().pin_manifests().len(), 0);

    let runtime_deps =
        ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled()).with_sorafs_node(sorafs_node);
    let torii = Torii::new_with_handle(
        ChainId::from("sorafs-retired-storage-pin-router-test"),
        signed_query_test_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state.clone(),
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        runtime_deps,
    );

    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/v1/sorafs/storage/pin")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"manifest_b64":"AA==","payload_b64":"AA=="}"#,
        ))
        .expect("retired-route probe");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));

    let response = torii
        .api_router_for_tests()
        .oneshot(request)
        .await
        .expect("retired-route response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        storage.manifest_count(),
        0,
        "an HTTP request must not mutate provider-local storage"
    );
    assert_eq!(
        state.view().world().pin_manifests().len(),
        0,
        "an HTTP request must not create pre-commit pin-registry state"
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn appeal_finance_publication_routes_are_read_only() {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use axum::{
        body::Body,
        extract::ConnectInfo,
        http::{Method, Request, StatusCode},
    };
    use sorafs_node::{
        GovernanceDagRuntimeProviderQualificationV1, GovernanceDagRuntimeSigner,
        GovernanceDagSealedCheckpointStore, GovernanceDagSealedStateRecord,
        GovernanceDagSealedStateSlot, NodeHandle, NodeRuntimeDeps,
    };
    use tower::ServiceExt as _;

    #[derive(Debug)]
    struct RouterGovernanceDagSigner {
        key_pair: KeyPair,
    }

    impl RouterGovernanceDagSigner {
        const HANDLE: &'static str = "pkcs11:governance-dag:retired-appeal-route-primary";
        const PEER_ID: &'static [u8] = b"12D3KooWRetiredAppealRoutePublisher";

        fn new() -> Self {
            Self {
                key_pair: KeyPair::try_from_seed(vec![0x4D; 32], Algorithm::Ed25519)
                    .expect("derive retired-route Governance DAG signer"),
            }
        }

        fn public_key_bytes(&self) -> [u8; 32] {
            let (algorithm, bytes) = self
                .key_pair
                .public_key()
                .try_to_bytes()
                .expect("serialize retired-route Governance DAG public key");
            assert_eq!(algorithm, Algorithm::Ed25519);
            bytes.try_into().expect("Ed25519 public key width")
        }

        fn expected_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0x85; 32])
        }
    }

    impl GovernanceDagRuntimeSigner for RouterGovernanceDagSigner {
        fn handle(&self) -> &str {
            Self::HANDLE
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(Self::expected_qualification())
        }

        fn publisher_peer_id(&self) -> &[u8] {
            Self::PEER_ID
        }

        fn public_key(&self) -> [u8; 32] {
            self.public_key_bytes()
        }

        fn sign(&self, payload: &[u8]) -> Result<[u8; 64], String> {
            Signature::try_new(self.key_pair.private_key(), payload)
                .map_err(|_| "retired-route Governance DAG signing failed".to_owned())?
                .payload()
                .try_into()
                .map_err(|_| "retired-route Governance DAG signature width changed".to_owned())
        }
    }

    #[derive(Debug)]
    struct RouterGovernanceDagCheckpointState {
        records: [Option<GovernanceDagSealedStateRecord>; 6],
        generation_floors: [u64; 6],
    }

    impl Default for RouterGovernanceDagCheckpointState {
        fn default() -> Self {
            Self {
                records: std::array::from_fn(|_| None),
                generation_floors: [0; 6],
            }
        }
    }

    #[derive(Debug, Default)]
    struct RouterGovernanceDagCheckpointStore {
        state: std::sync::Mutex<RouterGovernanceDagCheckpointState>,
    }

    impl RouterGovernanceDagCheckpointStore {
        const HANDLE: &'static str = "sealed:governance-dag:retired-appeal-route-primary";
        const POLICY_DIGEST: [u8; 32] = [0x86; 32];

        const fn slot_index(slot: GovernanceDagSealedStateSlot) -> usize {
            match slot {
                GovernanceDagSealedStateSlot::Checkpoint => 0,
                GovernanceDagSealedStateSlot::PublishIntent => 1,
                GovernanceDagSealedStateSlot::ProducerCheckpoint => 2,
                GovernanceDagSealedStateSlot::ProducerPublishIntent => 3,
                GovernanceDagSealedStateSlot::IpfsRequestReplay => 4,
                GovernanceDagSealedStateSlot::SignedHeadRequestReplay => 5,
            }
        }

        fn expected_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
            GovernanceDagRuntimeProviderQualificationV1::new(1, Self::POLICY_DIGEST)
        }
    }

    impl GovernanceDagSealedCheckpointStore for RouterGovernanceDagCheckpointStore {
        fn handle(&self) -> &str {
            Self::HANDLE
        }

        fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
            Ok(Self::expected_qualification())
        }

        fn load(
            &self,
            slot: GovernanceDagSealedStateSlot,
        ) -> Result<Option<GovernanceDagSealedStateRecord>, String> {
            let state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            Ok(state.records[Self::slot_index(slot)].clone())
        }

        fn compare_and_swap(
            &self,
            slot: GovernanceDagSealedStateSlot,
            expected_revision: Option<[u8; 32]>,
            next: GovernanceDagSealedStateRecord,
        ) -> Result<(), String> {
            let index = Self::slot_index(slot);
            let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            if state.records[index].as_ref().map(|record| record.revision) != expected_revision {
                return Err("compare-and-swap conflict".to_owned());
            }
            if next.generation <= state.generation_floors[index]
                || next.payload.is_empty()
                || !next.has_valid_revision(slot)
            {
                return Err("invalid or non-monotonic record".to_owned());
            }
            state.generation_floors[index] = next.generation;
            state.records[index] = Some(next);
            Ok(())
        }

        fn delete(
            &self,
            slot: GovernanceDagSealedStateSlot,
            expected_revision: [u8; 32],
        ) -> Result<(), String> {
            let index = Self::slot_index(slot);
            let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
            if state.records[index].as_ref().map(|record| record.revision)
                != Some(expected_revision)
            {
                return Err("delete conflict".to_owned());
            }
            state.records[index] = None;
            Ok(())
        }
    }

    fn snapshot_files(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        fn visit(root: &Path, directory: &Path, snapshot: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
            let mut entries = fs::read_dir(directory)
                .unwrap_or_else(|error| {
                    panic!("read snapshot directory {}: {error}", directory.display())
                })
                .map(|entry| entry.expect("read snapshot entry"))
                .collect::<Vec<_>>();
            entries.sort_by_key(std::fs::DirEntry::path);
            for entry in entries {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .expect("snapshot path is rooted")
                    .to_path_buf();
                if entry
                    .file_type()
                    .expect("read snapshot entry type")
                    .is_dir()
                {
                    snapshot.push((relative, None));
                    visit(root, &path, snapshot);
                } else {
                    snapshot.push((
                        relative,
                        Some(fs::read(&path).unwrap_or_else(|error| {
                            panic!("read snapshot file {}: {error}", path.display())
                        })),
                    ));
                }
            }
        }

        let mut snapshot = Vec::new();
        visit(root, root, &mut snapshot);
        snapshot
    }

    let cfg = crate::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(IrohaState::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
        capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };
    let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let storage_dir = tempfile::tempdir().expect("appeal publication storage tempdir");
    let storage_root = storage_dir
        .path()
        .canonicalize()
        .expect("canonical appeal publication storage root");
    let signer = Arc::new(RouterGovernanceDagSigner::new());
    let checkpoint_store = Arc::new(RouterGovernanceDagCheckpointStore::default());
    let sorafs_node = NodeHandle::try_new_with_runtime_deps(
        sorafs_node::config::StorageConfig::builder()
            .enabled(true)
            .data_dir(storage_root.join("storage"))
            .governance_dir(Some(storage_root.join("governance")))
            .governance_dag_publisher_peer_id(Some(
                String::from_utf8(RouterGovernanceDagSigner::PEER_ID.to_vec())
                    .expect("retired-route publisher peer id is UTF-8"),
            ))
            .governance_dag_signer_handle(Some(RouterGovernanceDagSigner::HANDLE.to_owned()))
            .governance_dag_signer_qualification(Some(
                RouterGovernanceDagSigner::expected_qualification(),
            ))
            .governance_dag_checkpoint_store_handle(Some(
                RouterGovernanceDagCheckpointStore::HANDLE.to_owned(),
            ))
            .governance_dag_checkpoint_store_qualification(Some(
                RouterGovernanceDagCheckpointStore::expected_qualification(),
            ))
            .governance_dag_publisher_public_key_hex(Some(hex::encode(signer.public_key_bytes())))
            .build(),
        NodeRuntimeDeps::default()
            .with_governance_dag_signer(signer)
            .with_governance_dag_checkpoint_store(checkpoint_store),
    )
    .expect("initialise runtime-signed Governance DAG publisher");
    assert!(sorafs_node.has_governance_publisher());

    let runtime_deps = ToriiRuntimeDeps::new(routing::MaybeTelemetry::disabled())
        .with_sorafs_node(sorafs_node.clone());
    let torii = Torii::new_with_handle(
        ChainId::from("sorafs-retired-appeal-publication-router-test"),
        signed_query_test_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        runtime_deps,
    );
    let router = torii.api_router_for_tests();
    let files_before = snapshot_files(&storage_root);
    let pending_before = sorafs_node.pending_governance_publication_count();

    for path in [
        "/v1/sorafs/appeals/finance/reports",
        "/v1/sorafs/appeals/finance/weekly-rollups",
    ] {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .expect("retired appeal-finance publication route probe");
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));

        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("retired appeal-finance publication route response");
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED, "{path}");
    }

    assert_eq!(
        sorafs_node.pending_governance_publication_count(),
        pending_before,
        "retired publication routes must not enqueue durable Governance work"
    );
    assert_eq!(
        snapshot_files(&storage_root),
        files_before,
        "retired publication routes must not mutate the Governance DAG, publish index, or durable outbox"
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn contracts_aliases_route_is_mounted_in_api_router() {
    use axum::{
        body::Body,
        extract::ConnectInfo,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt as _;

    let cfg = crate::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(IrohaState::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
        capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };
    let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let torii = Torii::new_with_handle(
        ChainId::from("contracts-aliases-router-test"),
        signed_query_test_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        routing::MaybeTelemetry::disabled(),
    );

    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/v1/contracts/aliases")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{"))
        .expect("request");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));

    let response = torii
        .api_router_for_tests()
        .oneshot(request)
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn sorafs_capacity_declare_route_is_mounted_in_api_router() {
    use axum::{
        body::Body,
        extract::ConnectInfo,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt as _;

    let cfg = crate::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(IrohaState::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
        capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };
    let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let torii = Torii::new_with_handle(
        ChainId::from("sorafs-capacity-declare-router-test"),
        signed_query_test_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        routing::MaybeTelemetry::disabled(),
    );

    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/v1/sorafs/capacity/declare")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{"))
        .expect("request");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));

    let response = torii
        .api_router_for_tests()
        .oneshot(request)
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn retired_sorafs_mutation_routes_are_absent() {
    use axum::{
        body::Body,
        extract::ConnectInfo,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt as _;

    let cfg = crate::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(IrohaState::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
        capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };
    let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let torii = Torii::new_with_handle(
        ChainId::from("sorafs-retired-por-router-test"),
        signed_query_test_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        routing::MaybeTelemetry::disabled(),
    );
    let router = torii.api_router_for_tests();

    for path in [
        "/v1/sorafs/capacity/dispute",
        "/v1/sorafs/capacity/schedule",
        "/v1/sorafs/capacity/complete",
        "/v1/sorafs/capacity/uptime",
        "/v1/sorafs/capacity/failure",
        "/v1/sorafs/por/trigger",
        "/v1/sorafs/capacity/por-challenge",
        "/v1/sorafs/capacity/por",
        "/v1/sorafs/storage/por-challenge",
        "/v1/sorafs/storage/por-proof",
        "/v1/sorafs/storage/por-verdict",
        "/v1/sorafs/moderation/viewer-audit-reports",
        "/v1/sorafs/moderation/viewer-audit-reports/publish-due",
    ] {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .expect("removed-route probe");
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));

        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("removed-route response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }

    for (method, path) in [
        (Method::POST, "/v1/sorafs/capacity/por-proof"),
        (Method::POST, "/v1/sorafs/capacity/por-verdict"),
        (Method::POST, "/v1/sorafs/por/vrf"),
        (Method::GET, "/v1/sorafs/por/status"),
        (Method::GET, "/v1/sorafs/por/export"),
        (Method::GET, "/v1/sorafs/por/report/2026-W01"),
        (Method::GET, "/v1/evidence/audit"),
        (Method::GET, "/v1/evidence/status"),
    ] {
        let mut request = Request::builder()
            .method(method.clone())
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .expect("live-route probe");
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));

        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("live-route response");
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "live SoraFS route was removed accidentally: {method} {path}"
        );
    }
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn sccp_recent_messages_route_survives_soracloud_fallback() {
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    let cfg = crate::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(IrohaState::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
        capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };
    let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let torii = Torii::new_with_handle(
        ChainId::from("sccp-recent-route-test"),
        signed_query_test_network_id(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        routing::MaybeTelemetry::disabled(),
    );

    let mut request = Request::builder()
        .method(Method::GET)
        .uri("/v1/sccp/messages/recent")
        .header(axum::http::header::ACCEPT, "application/json")
        .body(Body::empty())
        .expect("request");
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));

    let response = torii
        .api_router_for_tests()
        .oneshot(request)
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let text = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(
        text.contains("\"items\""),
        "expected SCCP recent-messages JSON payload, got: {text}"
    );
}

#[test]
fn iso_error_mapping_returns_expected_variants() {
    use iroha_data_model::ValidationFail::{InternalError, NotPermitted};

    assert!(matches!(
        super::map_iso_error(MsgError::InvalidFormat),
        NotPermitted(_)
    ));
    assert!(matches!(
        super::map_iso_error(MsgError::NoActiveMessage),
        InternalError(_)
    ));
}

#[test]
fn push_into_queue_error_sets_backpressure_headers() {
    use nonzero_ext::nonzero;

    let err = super::Error::PushIntoQueue {
        source: Box::new(queue::Error::Full),
        backpressure: queue::BackpressureState::Saturated {
            queued: 5,
            capacity: nonzero!(5_usize),
        },
    };

    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let headers = response.headers();
    assert_eq!(
        headers
            .get("X-Iroha-Queue-State")
            .map(|v| v.to_str().unwrap()),
        Some("saturated")
    );
    assert_eq!(
        headers.get("Retry-After").map(|v| v.to_str().unwrap()),
        Some("1")
    );
}

#[test]
fn push_into_queue_error_sets_reject_code_header() {
    use nonzero_ext::nonzero;

    let err = super::Error::PushIntoQueue {
        source: Box::new(queue::Error::InBlockchain),
        backpressure: queue::BackpressureState::Healthy {
            queued: 0,
            capacity: nonzero!(1_usize),
        },
    };

    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|v| v.to_str().ok()),
        Some("PRTRY:ALREADY_COMMITTED")
    );
}

#[test]
fn push_into_queue_confidential_policy_rejection_maps_to_forbidden() {
    use nonzero_ext::nonzero;

    let err = super::Error::PushIntoQueue {
        source: Box::new(queue::Error::ConfidentialPolicyAdmissionRejected {
            reason: TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                "shield not permitted by policy".to_owned(),
            )),
            detail: "shield not permitted by policy".to_owned(),
        }),
        backpressure: queue::BackpressureState::Healthy {
            queued: 0,
            capacity: nonzero!(1_usize),
        },
    };

    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("PRTRY:CONFIDENTIAL_POLICY_REJECTED")
    );
    let body = executor::block_on(http_body_util::BodyExt::collect(response.into_body()))
        .expect("response body")
        .to_bytes();
    let payload = norito::decode_from_bytes::<ErrorEnvelope>(&body).expect("queue error envelope");
    assert_eq!(payload.code, "queue_confidential_policy_rejected");
    let details = payload.details.expect("queue error details");
    assert_eq!(details.retry_after_seconds, None);
    assert_eq!(details.queue.expect("queue snapshot").state, "healthy");
}

#[test]
fn push_into_queue_unresolved_route_maps_to_bad_request() {
    use nonzero_ext::nonzero;

    let err = super::Error::PushIntoQueue {
        source: Box::new(queue::Error::UnresolvedRoute {
            reason: "lane 9 is not present in the lane catalog".to_owned(),
        }),
        backpressure: queue::BackpressureState::Healthy {
            queued: 0,
            capacity: nonzero!(1_usize),
        },
    };

    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|v| v.to_str().ok()),
        Some("PRTRY:ROUTE_UNRESOLVED")
    );
}

#[test]
fn push_into_queue_fee_rejections_are_typed_and_redacted() {
    use nonzero_ext::nonzero;

    const PRIVATE_REASON: &str = "private sponsor rule internals";
    for (source, expected_status, expected_code) in [
        (
            queue::Error::NexusFeeAdmissionRejected {
                code: FeeRejectionCode::BeneficiaryNotEligible,
                reason: PRIVATE_REASON.to_owned(),
            },
            StatusCode::UNPROCESSABLE_ENTITY,
            FeeRejectionCode::BeneficiaryNotEligible,
        ),
        (
            queue::Error::NexusFeeAdmissionConfigInvalid {
                code: FeeRejectionCode::InvalidProgramConfiguration,
                reason: PRIVATE_REASON.to_owned(),
            },
            StatusCode::SERVICE_UNAVAILABLE,
            FeeRejectionCode::InvalidProgramConfiguration,
        ),
    ] {
        let (_, public_detail) = queue_rejection_metadata(&source);
        assert!(!public_detail.contains(PRIVATE_REASON));
        assert!(public_detail.contains(expected_code.as_str()));
        let response = super::Error::PushIntoQueue {
            source: Box::new(source),
            backpressure: queue::BackpressureState::Healthy {
                queued: 0,
                capacity: nonzero!(1_usize),
            },
        }
        .into_response();
        assert_eq!(response.status(), expected_status);
        let body = executor::block_on(http_body_util::BodyExt::collect(response.into_body()))
            .expect("fee queue response body")
            .to_bytes();
        assert!(!String::from_utf8_lossy(&body).contains(PRIVATE_REASON));
        let envelope: ErrorEnvelope =
            norito::decode_from_bytes(&body).expect("typed fee queue envelope");
        let fee = envelope
            .details
            .and_then(|details| details.fee)
            .expect("typed fee rejection details");
        assert_eq!(fee.code, expected_code.as_str());
    }
}

#[tokio::test]
async fn serialization_error_emits_redacted_norito_payload() {
    let err = super::Error::SerializationFailure {
        context: "unit_test",
        source: Box::new(norito::json::Error::Message("boom".into())),
    };

    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap())
        .expect("content-type header present");
    assert_eq!(content_type, super::utils::NORITO_MIME_TYPE);

    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload = norito::decode_from_bytes::<super::ErrorEnvelope>(&body).unwrap();
    assert_eq!(payload.code(), "serialization_error");
    assert_eq!(payload.message(), "failed to serialize unit_test payload");
    assert!(
        !payload.message().contains("boom"),
        "redacted message leaked internal context"
    );
}

#[tokio::test]
async fn internal_validation_failures_never_expose_their_source_message() {
    const MARKER: &str = "private_runtime_marker_9a2c";
    for format in [ResponseFormat::Json, ResponseFormat::Norito] {
        let response = error_response_with_format(
            super::Error::Query(ValidationFail::InternalError(MARKER.to_owned())),
            format,
        );
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .expect("collect redacted validation response")
            .to_bytes();
        assert!(!String::from_utf8_lossy(&body).contains(MARKER));
        let envelope: super::ErrorEnvelope = match format {
            ResponseFormat::Json => {
                norito::json::from_slice(&body).expect("decode JSON validation error")
            }
            ResponseFormat::Norito => {
                norito::decode_from_bytes(&body).expect("decode Norito validation error")
            }
        };
        assert_eq!(envelope.code(), "internal_server_error");
        assert_eq!(envelope.message(), "Torii could not complete the request.");
        assert!(envelope.details.is_none());
    }

    let response =
        super::Error::Query(ValidationFail::InternalError(MARKER.to_owned())).into_response();
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect direct validation response")
        .to_bytes();
    let envelope: super::ErrorEnvelope =
        norito::decode_from_bytes(&body).expect("decode direct validation error");
    assert_eq!(envelope.code(), "internal_server_error");
    assert!(!envelope.message().contains(MARKER));
}

#[test]
fn accept_transaction_signature_failure_sets_code_and_header() {
    let kp = checked_torii_test_ed25519_keypair(
        0x7f,
        "derive accept-transaction signature failure fixture key",
    );
    let authority = AccountId::of(kp.public_key().clone());
    let tx = TransactionBuilder::new(
        signed_query_test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(kp.private_key());

    let fail = iroha_core::tx::SignatureVerificationFail::new(
        tx.signature().clone(),
        iroha_core::tx::SignatureRejectionCode::UnsupportedAuthority,
        iroha_data_model::transaction::signed::MULTISIG_SIGNING_UNSUPPORTED_REASON,
    );
    let err = super::Error::AcceptTransaction(
        iroha_core::tx::AcceptTransactionFail::SignatureVerification(fail),
    );

    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let headers = response.headers();
    assert_eq!(
        headers
            .get("x-iroha-reject-code")
            .and_then(|v| v.to_str().ok()),
        Some(iroha_core::tx::SignatureRejectionCode::UnsupportedAuthority.as_str())
    );

    let body = executor::block_on(http_body_util::BodyExt::collect(response.into_body()))
        .expect("collect body")
        .to_bytes();
    let envelope =
        norito::decode_from_bytes::<super::ErrorEnvelope>(&body).expect("decode error envelope");
    assert_eq!(
        envelope.code(),
        iroha_core::tx::SignatureRejectionCode::UnsupportedAuthority.as_str()
    );
    assert!(envelope.message().contains("failed to accept transaction"));
}

#[test]
fn accept_transaction_limit_failure_sets_header_code() {
    let err =
        super::Error::AcceptTransaction(iroha_core::tx::AcceptTransactionFail::TransactionLimit(
            iroha_data_model::transaction::error::TransactionLimitError {
                reason: "too big".into(),
            },
        ));

    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let headers = response.headers();
    assert_eq!(
        headers
            .get("x-iroha-reject-code")
            .and_then(|v| v.to_str().ok()),
        Some("transaction_rejected")
    );

    let body = executor::block_on(http_body_util::BodyExt::collect(response.into_body()))
        .expect("collect body")
        .to_bytes();
    let envelope =
        norito::decode_from_bytes::<super::ErrorEnvelope>(&body).expect("decode error envelope");
    assert_eq!(envelope.code(), "transaction_rejected");
    assert!(envelope.message().contains("too big"));
}

include!("part_9b_error_headers.rs");
