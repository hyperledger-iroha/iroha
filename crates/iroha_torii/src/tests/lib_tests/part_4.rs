#[tokio::test]
async fn configured_proof_body_layer_accepts_above_axum_default_and_rejects_limit_plus_one() {
    let app = mk_app_state_for_tests();
    let router = proof_post_router_with_body_limits(
        Router::<SharedAppState>::new().route(
            "/probe",
            post(|body: Bytes| async move {
                assert!(body.len() > 2 * 1024 * 1024);
                StatusCode::NO_CONTENT
            }),
        ),
        app.clone(),
    )
    .with_state::<()>(app.clone());
    let above_axum_default = axum::http::Request::builder()
        .method("POST")
        .uri("/probe")
        .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(vec![0_u8; 2 * 1024 * 1024 + 1]))
        .expect("request");
    let response = router
        .clone()
        .oneshot(above_axum_default)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let over_configured_limit = axum::http::Request::builder()
        .method("POST")
        .uri("/probe")
        .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(vec![0_u8; app.proof_limits.max_body_bytes + 1]))
        .expect("request");
    let response = router
        .oneshot(over_configured_limit)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
#[tokio::test]
async fn proof_body_middleware_deadline_rejects_stall_and_releases_admission() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.proof_body_inflight = Arc::new(tokio::sync::Semaphore::new(1));
        state.proof_limits.body_read_timeout = Duration::from_millis(30);
    }
    let router = proof_post_router_with_body_limits(
        Router::<SharedAppState>::new().route(
            "/probe",
            post(|_body: Bytes| async move { StatusCode::NO_CONTENT }),
        ),
        app.clone(),
    )
    .with_state::<()>(app.clone());
    let stalled =
        futures_util::stream::pending::<std::result::Result<Bytes, std::convert::Infallible>>();
    let first_request = axum::http::Request::builder()
        .method("POST")
        .uri("/probe")
        .body(Body::from_stream(stalled))
        .expect("stalled request");
    let first_router = router.clone();
    let first = tokio::spawn(async move {
        first_router
            .oneshot(first_request)
            .await
            .expect("first response")
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while app.proof_body_inflight.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("stalled request must acquire admission");
    let second = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/probe")
                .body(Body::from(Bytes::from_static(b"second")))
                .expect("second request"),
        )
        .await
        .expect("second response");
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    let first = first.await.expect("first task");
    assert_eq!(first.status(), StatusCode::REQUEST_TIMEOUT);
    let third = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/probe")
                .body(Body::from(Bytes::from_static(b"third")))
                .expect("third request"),
        )
        .await
        .expect("third response");
    assert_eq!(
        third.status(),
        StatusCode::NO_CONTENT,
        "deadline completion must release middleware admission"
    );
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn verified_source_admission_precedes_body_polling_and_transfers_one_slot() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.verified_source_compile_inflight = Arc::new(tokio::sync::Semaphore::new(1));
        state.verified_source_body_read_timeout = Duration::from_millis(30);
    }
    let router =
        Router::<SharedAppState>::new()
            .route(
                "/verified-source",
                post(
                    |Extension(admission): Extension<VerifiedSourceCompileAdmission>,
                     _body: Bytes| async move {
                        let _permit = admission.take().expect("compiler admission handoff");
                        StatusCode::NO_CONTENT
                    },
                ),
            )
            .layer(axum::middleware::from_fn_with_state(
                app.clone(),
                verified_source_body_admission_middleware,
            ))
            .with_state::<()>(app.clone());
    let stalled =
        futures_util::stream::pending::<std::result::Result<Bytes, std::convert::Infallible>>();
    let first_router = router.clone();
    let first = tokio::spawn(async move {
        first_router
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/verified-source")
                    .body(Body::from_stream(stalled))
                    .expect("stalled request"),
            )
            .await
            .expect("first response")
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while app.verified_source_compile_inflight.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the first request must own compiler capacity before polling its body");
    let rejected = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/verified-source")
                .body(Body::from(Bytes::from_static(b"second")))
                .expect("second request"),
        )
        .await
        .expect("second response");
    assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        first.await.expect("first task").status(),
        StatusCode::REQUEST_TIMEOUT
    );
    let accepted = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/verified-source")
                .body(Body::from(Bytes::from_static(b"third")))
                .expect("third request"),
        )
        .await
        .expect("third response");
    assert_eq!(accepted.status(), StatusCode::NO_CONTENT);
}
#[tokio::test]
async fn proof_body_absolute_deadline_rejects_continuous_trickle() {
    let trickle = futures_util::stream::unfold((), |_| async {
        tokio::time::sleep(Duration::from_millis(5)).await;
        Some((
            Ok::<_, std::convert::Infallible>(Bytes::from_static(b"x")),
            (),
        ))
    });
    let request = axum::http::Request::new(Body::from_stream(trickle));
    let response = collect_proof_body_with_deadline(request, 1024, Duration::from_millis(25))
        .await
        .expect_err("trickle must not reset the absolute deadline");
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}
#[tokio::test]
async fn proof_json_egress_charges_the_exact_serialized_response_bytes() {
    let payload = norito::json!({"status": "exact-json-egress"});
    let expected = norito::json::to_vec(&payload).expect("encode expected response");
    assert!(expected.len() > 1);
    let mut limited_app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut limited_app).expect("unique app state");
        state.proof_limits.retry_after = std::time::Duration::from_secs(7);
        state.proof_egress_limiter =
            limits::RateLimiter::new_u64(Some(1), Some(expected.len() as u64 - 1));
    }
    let err = proof_json_response_with_egress(
        &limited_app,
        &HeaderMap::new(),
        Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        "v1/zk/verify-batch",
        payload.clone(),
        true,
    )
    .await
    .expect_err("one byte below the encoded response must be throttled");
    let limited_response = err.into_response();
    assert_eq!(limited_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        limited_response
            .headers()
            .get(axum::http::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
        Some("7")
    );
    let mut exact_app = mk_app_state_for_tests();
    Arc::get_mut(&mut exact_app)
        .expect("unique app state")
        .proof_egress_limiter = limits::RateLimiter::new_u64(Some(1), Some(expected.len() as u64));
    let response = proof_json_response_with_egress(
        &exact_app,
        &HeaderMap::new(),
        Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        "v1/zk/verify-batch",
        payload,
        true,
    )
    .await
    .expect("an exact-byte budget should pass");
    let actual = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("response body")
        .to_bytes();
    assert_eq!(actual.as_ref(), expected.as_slice());
}
#[tokio::test]
async fn buffered_proof_response_egress_charges_exact_bytes_and_preserves_body() {
    let expected = Bytes::from_static(b"exact-finality-proof-response");
    let response = || {
        let mut response = AxResponse::new(Body::from(expected.clone()));
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    };
    let remote = Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    let mut limited_app = mk_app_state_for_tests();
    Arc::get_mut(&mut limited_app)
        .expect("unique limited app state")
        .proof_egress_limiter = limits::RateLimiter::new_u64(
        Some(1),
        Some(u64::try_from(expected.len()).expect("small body") - 1),
    );
    let error = proof_response_with_exact_egress(
        limited_app.as_ref(),
        &HeaderMap::new(),
        remote,
        "v1/bridge/finality",
        response(),
        true,
    )
    .await
    .expect_err("one byte below the buffered proof response must reject");
    assert!(matches!(
        error,
        Error::ProofRateLimited {
            endpoint: "v1/bridge/finality",
            ..
        }
    ));
    let mut exact_app = mk_app_state_for_tests();
    Arc::get_mut(&mut exact_app)
        .expect("unique exact app state")
        .proof_egress_limiter = limits::RateLimiter::new_u64(
        Some(1),
        Some(u64::try_from(expected.len()).expect("small body")),
    );
    let admitted = proof_response_with_exact_egress(
        exact_app.as_ref(),
        &HeaderMap::new(),
        remote,
        "v1/bridge/finality",
        response(),
        true,
    )
    .await
    .expect("exact buffered proof response budget must pass");
    assert_eq!(
        admitted
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let actual = axum::body::to_bytes(admitted.into_body(), usize::MAX)
        .await
        .expect("collect admitted response");
    assert_eq!(actual, expected);
}

#[test]
fn query_validation_message_preserves_conversion_source() {
    let err = iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(
            "AccountId must use a canonical I105 literal".to_owned(),
        ),
    );
    assert_eq!(
        validation_fail_message(&err),
        "AccountId must use a canonical I105 literal"
    );
}
include!("part_4b_alias_multisig_auth.rs");
