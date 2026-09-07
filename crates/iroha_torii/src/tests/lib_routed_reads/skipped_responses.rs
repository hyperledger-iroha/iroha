// Skipped fanout replies must release transport owners before another route.
#[cfg(feature = "connect")]
fn skipped_response_test_fetch(
    pool: &Arc<tokio::sync::Semaphore>,
    route: RoutingDecision,
    skipped: StatusCode,
) -> Response {
    let permit = pool
        .clone()
        .try_acquire_owned()
        .expect("the previous route must release its proxy reservation");
    let response = if route.dataspace_id.as_u64() == 1 {
        let code = match skipped {
            StatusCode::NOT_FOUND => "not_found",
            StatusCode::FORBIDDEN => "permission_denied",
            _ => "route_unavailable",
        };
        torii_proxy_error_response(skipped, code, "route detail")
    } else {
        crate::utils::respond_value_with_format(
            norito::json!({"items": [], "total": 0, "has_more": false, "count_mode": "exact"}),
            ResponseFormat::Json,
        )
    };
    hold_torii_proxy_memory_in_response_body(response, ToriiProxyMemoryReservation::new(permit))
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn skipped_route_summary_releases_body_and_extension_reservations() {
    for (status, code) in [
        (StatusCode::NOT_FOUND, "not_found"),
        (StatusCode::SERVICE_UNAVAILABLE, "route_unavailable"),
        (StatusCode::FORBIDDEN, "permission_denied"),
    ] {
        let pool = Arc::new(tokio::sync::Semaphore::new(1));
        let reservation = ToriiProxyMemoryReservation::new(
            pool.clone().try_acquire_owned().expect("initial slot"),
        );
        let mut upstream = torii_proxy_error_response(status, code, "x".repeat(64 * 1024));
        upstream.extensions_mut().insert(reservation.clone());
        upstream
            .headers_mut()
            .insert("x-upstream-detail", HeaderValue::from_static("discard"));
        upstream = hold_torii_proxy_memory_in_response_body(upstream, reservation);
        assert_eq!(pool.available_permits(), 0);
        let summary = summarize_skipped_torii_route_response(upstream);
        assert_eq!(pool.available_permits(), 1);
        assert_eq!(summary.status(), status);
        assert!(
            summary
                .extensions()
                .get::<ToriiProxyMemoryReservation>()
                .is_none()
        );
        assert!(!summary.headers().contains_key("x-upstream-detail"));
        assert!(torii_response_has_reject_code(&summary, code));
        let body = axum::body::to_bytes(summary.into_body(), 4096)
            .await
            .expect("fixed small summary");
        let envelope: ErrorEnvelope =
            norito::decode_from_bytes(&body).expect("canonical error envelope");
        assert_eq!(envelope.code(), code);
    }
}

#[tokio::test]
async fn skipped_route_summary_keeps_non_skippable_errors_exact() {
    let response = torii_proxy_error_response(
        StatusCode::TOO_MANY_REQUESTS,
        "proxy_capacity_exceeded",
        "retry later",
    );
    let headers = response.headers().clone();
    let response = summarize_skipped_torii_route_response(response);
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers(), &headers);
    let error = response_error(response).await;
    assert_eq!(error.code(), "proxy_capacity_exceeded");
    assert_eq!(error.message(), "retry later");
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn fanout_collectors_release_skipped_reservations_before_next_route() {
    let routes = [
        RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
        RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
    ];
    for skipped in [StatusCode::NOT_FOUND, StatusCode::SERVICE_UNAVAILABLE] {
        for collector in 0..4 {
            let pool = Arc::new(tokio::sync::Semaphore::new(1));
            let fetch =
                |route| std::future::ready(skipped_response_test_fetch(&pool, route, skipped));
            let (payload_count, diagnostics) = match collector {
                0 => {
                    let result = collect_torii_singleton_json_payloads(
                        &routes,
                        routed_read_test_working_set_bytes(),
                        ROUTED_READ_TEST_BODY_BYTES,
                        fetch,
                    )
                    .await
                    .expect("singleton should reach the second route");
                    (result.payloads.len(), result.diagnostics)
                }
                1 => {
                    let result = collect_torii_list_json_payloads(
                        &routes,
                        routed_read_test_working_set_bytes(),
                        ROUTED_READ_TEST_BODY_BYTES,
                        fetch,
                    )
                    .await
                    .expect("list should reach the second route");
                    (result.payloads.len(), result.diagnostics)
                }
                2 => {
                    let result = collect_torii_routed_list_json_payloads(
                        &routes,
                        routed_read_test_working_set_bytes(),
                        ROUTED_READ_TEST_BODY_BYTES,
                        fetch,
                    )
                    .await
                    .expect("routed list should reach the second route");
                    (result.payloads.len(), result.diagnostics)
                }
                _ => {
                    let result = collect_torii_paginated_list_json_payloads(
                        &routes,
                        1,
                        routed_read_test_working_set_bytes(),
                        ROUTED_READ_TEST_BODY_BYTES,
                        |route, _, _| fetch(route),
                    )
                    .await
                    .expect("paginated list should reach the second route");
                    (result.payloads.len(), result.diagnostics)
                }
            };
            assert_eq!(payload_count, 1);
            assert_eq!(diagnostics.attempted_routes, 2);
            assert_eq!(diagnostics.succeeded_routes, 1);
            assert_eq!(pool.available_permits(), 1);
        }
    }
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn alias_fanout_releases_skipped_denial_before_next_route() {
    let routes = [
        RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
        RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
    ];
    let pool = Arc::new(tokio::sync::Semaphore::new(1));
    let result = collect_torii_alias_json_payloads(
        &routes,
        0,
        "lookup denied",
        routed_read_test_working_set_bytes(),
        ROUTED_READ_TEST_BODY_BYTES,
        |route| {
            std::future::ready(skipped_response_test_fetch(
                &pool,
                route,
                StatusCode::FORBIDDEN,
            ))
        },
    )
    .await
    .expect("authorized route remains reachable after a denied route");
    assert_eq!(result.payloads.len(), 1);
    assert_eq!(result.diagnostics.denied_routes, 1);
    assert_eq!(result.diagnostics.succeeded_routes, 1);
    assert_eq!(pool.available_permits(), 1);
}
