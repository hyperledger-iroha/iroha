    use super::*;
    #[cfg(feature = "app_api")]
    use crate::tests_runtime_handlers::{
        bind_account_alias_for_test, bind_contract_alias_for_test,
        configure_multiple_dataspace_routes_for_test, configure_private_ingress_routes_for_test,
        mk_app_state_for_tests_with_world, world_with_account,
        world_with_account_bound_to_dataspace,
    };
    #[cfg(feature = "app_api")]
    use iroha_data_model::nexus::UniversalAccountId;

    async fn response_json(response: Response) -> Value {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        norito::json::from_slice(&body).expect("response body should decode as JSON")
    }

    #[cfg(feature = "app_api")]
    const ROUTED_READ_TEST_BODY_BYTES: usize = 1024 * 1024;

    #[cfg(feature = "app_api")]
    fn routed_read_test_working_set_bytes() -> usize {
        routed_read_working_set_for_phase(ROUTED_READ_TEST_BODY_BYTES)
    }

    #[cfg(feature = "app_api")]
    fn routed_read_test_budget() -> ToriiRoutedReadMemoryBudget {
        ToriiRoutedReadMemoryBudget::new(
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
        )
        .expect("routed-read test memory envelope should fit")
    }

    #[cfg(feature = "app_api")]
    async fn response_error(response: Response) -> ErrorEnvelope {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        norito::decode_from_bytes(&body).expect("response body should decode as an error envelope")
    }

    pub(super) fn configure_corrupt_inactive_autoscale_range_route_for_test(
        app: &mut SharedAppState,
    ) -> (LaneId, DataSpaceId) {
        let inactive_dataspace = DataSpaceId::new(1);
        let inactive_lane = LaneId::new(1);
        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                iroha_data_model::nexus::LaneConfig {
                    id: inactive_lane,
                    dataspace_id: inactive_dataspace,
                    alias: "manual-elastic-slot".to_owned(),
                    visibility: iroha_data_model::nexus::LaneVisibility::Public,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
            ],
        )
        .expect("corrupt lane catalog");
        let dataspace_catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: inactive_dataspace,
                alias: "inactive".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let mut nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            lane_catalog,
            dataspace_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        };
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min lanes");
        nexus.autoscale.max_lanes = NonZeroU32::new(3).expect("nonzero max lanes");
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);

        let app_state = Arc::get_mut(app).expect("unique app state");
        let state = Arc::get_mut(&mut app_state.state).expect("unique state");
        {
            let mut current = state.nexus.write();
            *current = nexus;
        }
        (inactive_lane, inactive_dataspace)
    }

    pub(super) fn configure_future_created_autoscale_route_for_test(
        app: &mut SharedAppState,
    ) -> (LaneId, DataSpaceId) {
        let future_lane = LaneId::new(1);
        let future_dataspace = DataSpaceId::UNIVERSAL;
        let mut lane = iroha_data_model::nexus::LaneConfig {
            id: future_lane,
            dataspace_id: future_dataspace,
            alias: "elastic-lane-1".to_owned(),
            visibility: iroha_data_model::nexus::LaneVisibility::Public,
            ..iroha_data_model::nexus::LaneConfig::default()
        };
        lane.metadata.insert(
            iroha_data_model::nexus::AUTOSCALE_META_MANAGED.to_owned(),
            "true".to_owned(),
        );
        lane.metadata.insert(
            iroha_data_model::nexus::AUTOSCALE_META_CREATED_HEIGHT.to_owned(),
            "7".to_owned(),
        );
        assert!(
            lane.is_autoscale_managed_elastic(),
            "fixture must be a valid-looking autoscale elastic lane"
        );
        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![iroha_data_model::nexus::LaneConfig::default(), lane],
        )
        .expect("future-created lane catalog");
        let mut nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            lane_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        };
        nexus.autoscale.enabled = true;
        nexus.autoscale.min_lanes = NonZeroU32::new(1).expect("nonzero min lanes");
        nexus.autoscale.max_lanes = NonZeroU32::new(3).expect("nonzero max lanes");
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);

        let app_state = Arc::get_mut(app).expect("unique app state");
        let state = Arc::get_mut(&mut app_state.state).expect("unique state");
        {
            let mut current = state.nexus.write();
            *current = nexus;
        }
        state.update_latest_block_header_cache_for_tests(BlockHeader::new(
            NonZeroU64::new(1).expect("nonzero authority height"),
            None,
            None,
            None,
            0,
            0,
        ));
        assert!(
            !state.is_lane_active_for_authority(future_lane),
            "future-created autoscale fixture must be inactive before creation height"
        );
        (future_lane, future_dataspace)
    }

    #[test]
    fn torii_route_resolution_rejects_inactive_autoscale_range_lane() {
        let authority = routed_read_test_account(0x7c);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let (inactive_lane, inactive_dataspace) =
            configure_corrupt_inactive_autoscale_range_route_for_test(&mut app);

        assert!(
            !app.state.is_lane_active_for_authority(inactive_lane),
            "fixture lane must be inactive for route authority"
        );
        let err = resolve_torii_route_for_dataspace_id(app.as_ref(), inactive_dataspace)
            .expect_err("inactive autoscale-range lane must not resolve as a Torii route");
        assert!(
            matches!(
                err,
                queue::RoutingResolveError::NoLaneForDataspace { .. }
                    | queue::RoutingResolveError::LaneDataspaceMismatch { .. }
            ),
            "inactive lane routing should fail closed, got {err:?}"
        );
    }

    #[test]
    fn torii_explicit_lane_route_rejects_inactive_autoscale_range_lane() {
        let authority = routed_read_test_account(0x7e);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let (inactive_lane, _) =
            configure_corrupt_inactive_autoscale_range_route_for_test(&mut app);

        let err = torii_route_for_lane_id(app.as_ref(), inactive_lane)
            .expect_err("explicit lane routing must not expose inactive autoscale-range lanes");
        let Error::PushIntoQueue { source, .. } = err else {
            panic!("inactive explicit lane routing should report route unavailability");
        };
        assert!(
            matches!(
                source.as_ref(),
                queue::Error::UnresolvedRoute { reason }
                    if reason.contains("inactive at the current authority height")
            ),
            "unexpected inactive lane route error: {source:?}"
        );
    }

    #[test]
    fn torii_fanout_route_discovery_excludes_inactive_autoscale_range_lane() {
        let authority = routed_read_test_account(0x7d);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let (inactive_lane, inactive_dataspace) =
            configure_corrupt_inactive_autoscale_range_route_for_test(&mut app);

        let routes = torii_all_dataspace_routes(app.as_ref());
        assert!(
            routes
                .iter()
                .all(|route| route.lane_id != inactive_lane
                    && route.dataspace_id != inactive_dataspace),
            "fanout route discovery must not expose inactive autoscale-range lanes: {routes:?}"
        );
        assert_eq!(
            routes,
            vec![RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)]
        );
        let public_dataspaces = torii_public_dataspace_ids(app.as_ref());
        assert!(
            !public_dataspaces.contains(&inactive_dataspace),
            "public visibility must not include dataspaces whose only public lane is inactive"
        );
    }

    #[test]
    fn torii_proxy_query_roundtrip_preserves_numeric_and_string_scalars() {
        let params = routing::AccountAssetsGetParams {
            limit: Some(25),
            offset: 3,
            asset: Some("xor#sora".to_owned()),
            scope: Some("dataspace:7".to_owned()),
            count_mode: Some("exact".to_owned()),
        };

        let encoded = encode_torii_proxy_query(&params)
            .expect("query encoding should succeed")
            .expect("non-empty params should produce a query string");
        let plan = routed_read_test_budget()
            .request_decode_plan()
            .expect("request decode plan");
        let decoded =
            decode_torii_proxy_query::<routing::AccountAssetsGetParams>(plan, Some(&encoded))
                .expect("query decoding should succeed");

        assert_eq!(decoded.limit, params.limit);
        assert_eq!(decoded.offset, params.offset);
        assert_eq!(decoded.asset, params.asset);
        assert_eq!(decoded.scope, params.scope);
        assert_eq!(decoded.count_mode, params.count_mode);
    }

    #[test]
    fn torii_proxy_query_roundtrip_preserves_json_filter_literals_as_strings() {
        let params = routing::ListFilterParams {
            filter: Some(r#"{"op":"eq","args":["id","alice.i105.invalid"]}"#.to_owned()),
            limit: Some(8),
            offset: 0,
            sort: Some("id:asc".to_owned()),
            count_mode: Some("exact".to_owned()),
        };

        let encoded = encode_torii_proxy_query(&params)
            .expect("query encoding should succeed")
            .expect("non-empty params should produce a query string");
        let plan = routed_read_test_budget()
            .request_decode_plan()
            .expect("request decode plan");
        let decoded = decode_torii_proxy_query::<routing::ListFilterParams>(plan, Some(&encoded))
            .expect("query decoding should succeed");

        assert_eq!(decoded.filter, params.filter);
        assert_eq!(decoded.limit, params.limit);
        assert_eq!(decoded.offset, params.offset);
        assert_eq!(decoded.sort, params.sort);
        assert_eq!(decoded.count_mode, params.count_mode);
    }

    fn checked_routed_read_test_keypair(
        seed: Vec<u8>,
        algorithm: iroha_crypto::Algorithm,
    ) -> KeyPair {
        KeyPair::try_from_seed(seed, algorithm).expect("derive routed-read fixture key")
    }

    fn routed_read_ed25519_test_keypair(seed: u8) -> KeyPair {
        checked_routed_read_test_keypair(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
    }

    fn routed_read_test_account(seed: u8) -> AccountId {
        AccountId::new(routed_read_ed25519_test_keypair(seed).public_key().clone())
    }

    #[test]
    fn routed_read_fixture_keypair_rejects_all_zero_seed() {
        assert!(
            KeyPair::try_from_seed(vec![0; 32], iroha_crypto::Algorithm::Ed25519).is_err(),
            "checked routed-read fixtures must reject invalid Ed25519 seed material"
        );
    }

    #[test]
    fn routed_read_test_account_uses_stable_checked_seed_material() {
        assert_eq!(
            routed_read_test_account(0x61),
            routed_read_test_account(0x61),
            "routed-read account fixtures must be stable for a fixed seed"
        );
        assert_ne!(
            routed_read_test_account(0x61),
            routed_read_test_account(0x62),
            "routed-read account fixture seeds must produce distinct accounts"
        );
    }

    #[test]
    fn fanout_route_scan_query_request_is_bounded_by_signed_window() {
        let request = authorize_query_for_test(
            iroha_data_model::query::json::IterableQueryJson {
                kind: iroha_data_model::query::json::IterableQueryKind::FindDomains,
                params: iroha_data_model::query::json::IterableQueryParamsJson {
                    limit: Some(3),
                    offset: Some(7),
                    fetch_size: Some(1),
                    sort_by_metadata_key: None,
                    order: None,
                    ids_projection: None,
                    lane_id: None,
                    dsid: None,
                },
                predicate: None,
            }
            .into_request()
            .expect("iterable request should build"),
            iroha_test_samples::ALICE_ID.clone(),
        );

        let request = fanout_route_scan_query_request(&request).expect("pagination should fit");
        let iroha_data_model::query::QueryRequest::Start(start) = request.request else {
            panic!("expected iterable routed query");
        };
        assert_eq!(
            start.params.pagination,
            iroha_data_model::query::parameters::Pagination::new(std::num::NonZeroU64::new(10), 0,)
        );
        assert_eq!(
            start.params.fetch_size.fetch_size,
            std::num::NonZeroU64::new(crate::routing::app_query_limits().max_fetch_size.min(10),)
        );
    }

    #[test]
    fn fanout_route_scan_without_client_limit_uses_configured_fetch_budget() {
        let request = authorize_query_for_test(
            iroha_data_model::query::json::IterableQueryJson {
                kind: iroha_data_model::query::json::IterableQueryKind::FindDomains,
                params: iroha_data_model::query::json::IterableQueryParamsJson {
                    limit: None,
                    offset: Some(7),
                    fetch_size: Some(1),
                    sort_by_metadata_key: None,
                    order: None,
                    ids_projection: None,
                    lane_id: None,
                    dsid: None,
                },
                predicate: None,
            }
            .into_request()
            .expect("iterable request should build"),
            iroha_test_samples::ALICE_ID.clone(),
        );

        let request = fanout_route_scan_query_request(&request).expect("default budget should fit");
        let iroha_data_model::query::QueryRequest::Start(start) = request.request else {
            panic!("expected iterable routed query");
        };
        assert_eq!(
            start.params.pagination,
            iroha_data_model::query::parameters::Pagination::new(
                std::num::NonZeroU64::new(crate::routing::app_query_limits().max_fetch_size.max(1)),
                0,
            ),
        );
        assert_eq!(
            start.params.fetch_size.fetch_size,
            std::num::NonZeroU64::new(crate::routing::app_query_limits().max_fetch_size.max(1)),
        );
    }

    #[test]
    fn unsupported_routed_query_response_is_conflict_not_not_implemented() {
        let response = unsupported_routed_query_response("unsupported routed query shape");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("query_unsupported")
        );
    }

    #[cfg(not(any(feature = "p2p_ws", feature = "connect")))]
    #[test]
    fn app_api_without_proxy_transport_fails_nonlocal_queries_closed() {
        let response = torii_proxy_transport_disabled_response(RoutingDecision::default());

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
    }

    #[cfg(all(not(feature = "app_api"), any(feature = "p2p_ws", feature = "connect")))]
    #[test]
    fn app_api_required_torii_proxy_response_is_route_unavailable() {
        let response = app_api_required_torii_proxy_response("Torii read proxying");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
    }

    #[test]
    fn iterable_routed_query_skips_not_found_and_route_unavailable_errors() {
        assert!(should_skip_iterable_routed_query_route_error(
            &torii_proxy_error_response(StatusCode::NOT_FOUND, "not_found", "missing")
        ));
        assert!(should_skip_iterable_routed_query_route_error(
            &torii_proxy_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "route_unavailable",
                "offline",
            )
        ));
        assert!(!should_skip_iterable_routed_query_route_error(
            &torii_proxy_error_response(StatusCode::BAD_REQUEST, "invalid", "bad request")
        ));
    }

    #[test]
    fn singleton_routed_query_skips_not_found_and_route_unavailable_errors() {
        assert!(should_skip_singleton_routed_query_route_error(
            &torii_proxy_error_response(StatusCode::NOT_FOUND, "not_found", "missing")
        ));
        assert!(should_skip_singleton_routed_query_route_error(
            &torii_proxy_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "route_unavailable",
                "offline",
            )
        ));
        assert!(!should_skip_singleton_routed_query_route_error(
            &torii_proxy_error_response(StatusCode::BAD_REQUEST, "invalid", "bad request")
        ));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_singleton_json_payloads_skips_route_unavailable_until_success() {
        let unavailable_route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
        let healthy_route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2));
        let expected = norito::json!({"status": {"kind": "Committed"}});
        let expected_for_closure = expected.clone();

        let collected = collect_torii_singleton_json_payloads(
            &[unavailable_route, healthy_route],
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            move |route| {
                let expected = expected_for_closure.clone();
                async move {
                    if route == unavailable_route {
                        torii_proxy_error_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "route_unavailable",
                            "authoritative peers offline",
                        )
                    } else {
                        crate::utils::respond_value_with_format(expected, ResponseFormat::Json)
                    }
                }
            },
        )
        .await
        .expect("healthy singleton payload should survive route_unavailable on another route");

        assert_eq!(collected.payloads, vec![expected]);
        assert_eq!(collected.diagnostics.attempted_routes, 2);
        assert_eq!(collected.diagnostics.succeeded_routes, 1);
        assert_eq!(collected.diagnostics.unavailable_routes, 1);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_singleton_json_payloads_prefers_not_found_when_no_route_succeeds() {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];

        let response = collect_torii_singleton_json_payloads(
            &routes,
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            move |route| async move {
                if route == routes[0] {
                    torii_proxy_error_response(StatusCode::NOT_FOUND, "not_found", "missing")
                } else {
                    torii_proxy_error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "route_unavailable",
                        "authoritative peers offline",
                    )
                }
            },
        )
        .await
        .expect_err("all singleton routes failing should surface a definitive not_found");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("not_found")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-attempted")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-first-failure")
                .and_then(|value| value.to_str().ok()),
            Some("not_found")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_singleton_json_payloads_returns_route_unavailable_when_only_unavailable()
    {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];

        let response = collect_torii_singleton_json_payloads(
            &routes,
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            move |_route| async move {
                torii_proxy_error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "route_unavailable",
                    "authoritative peers offline",
                )
            },
        )
        .await
        .expect_err("all unavailable singleton routes should surface route_unavailable");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-unavailable")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_singleton_json_payloads_keeps_one_route_body_live_at_a_time() {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];
        let route_count = routes.len();
        let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let collected = collect_torii_singleton_json_payloads(
            &routes,
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            {
                let active = std::sync::Arc::clone(&active);
                move |route| {
                    let active = std::sync::Arc::clone(&active);
                async move {
                        assert_eq!(
                            active.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                            0,
                            "a second route began while the prior response body was live"
                        );
                        tokio::task::yield_now().await;
                        active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    let payload =
                        crate::json_object(vec![crate::json_entry("route", route.dataspace_id)]);
                    crate::utils::respond_value_with_format(payload, ResponseFormat::Json)
                }
                }
            },
        )
        .await
        .expect("all singleton routes should succeed");
        assert_eq!(collected.payloads.len(), route_count);
        assert_eq!(collected.diagnostics.succeeded_routes, route_count);
        assert_eq!(active.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_list_json_payloads_skips_route_unavailable_until_success() {
        let unavailable_route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
        let healthy_route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2));
        let expected = norito::json!({"items": [{"id": "asset"}], "total": 1});
        let expected_for_closure = expected.clone();

        let collected =
            collect_torii_list_json_payloads(
                &[unavailable_route, healthy_route],
                routed_read_test_working_set_bytes(),
                ROUTED_READ_TEST_BODY_BYTES,
                move |route| {
                    let expected = expected_for_closure.clone();
                    async move {
                        if route == unavailable_route {
                            torii_proxy_error_response(
                                StatusCode::SERVICE_UNAVAILABLE,
                                "route_unavailable",
                                "authoritative peers offline",
                            )
                        } else {
                            crate::utils::respond_value_with_format(expected, ResponseFormat::Json)
                        }
                    }
                },
            )
            .await
            .expect("healthy list payload should survive route_unavailable on another route");

        assert_eq!(collected.payloads, vec![expected]);
        assert_eq!(collected.diagnostics.attempted_routes, 2);
        assert_eq!(collected.diagnostics.succeeded_routes, 1);
        assert_eq!(collected.diagnostics.unavailable_routes, 1);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_list_json_payloads_prefers_not_found_when_no_route_succeeds() {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];

        let response = collect_torii_list_json_payloads(
            &routes,
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            move |route| async move {
                if route == routes[0] {
                    torii_proxy_error_response(StatusCode::NOT_FOUND, "not_found", "missing")
                } else {
                    torii_proxy_error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "route_unavailable",
                        "authoritative peers offline",
                    )
                }
            },
        )
        .await
        .expect_err("all list routes failing should surface a definitive not_found");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("not_found")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-failed")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_list_json_payloads_returns_route_unavailable_when_only_unavailable() {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];

        let response = collect_torii_list_json_payloads(
            &routes,
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            move |_route| async move {
                torii_proxy_error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "route_unavailable",
                    "authoritative peers offline",
                )
            },
        )
        .await
        .expect_err("all unavailable list routes should surface route_unavailable");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-unavailable")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_account_history_json_payloads_fails_on_mid_route_unavailable() {
        let route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
        let params = routing::AccountHistoryGetParams {
            limit: Some(10),
            count_mode: Some("bounded".to_owned()),
            ..Default::default()
        };
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let response =
            collect_torii_account_history_json_payloads(
                &[route],
                &params,
                10,
                "bounded",
                routed_read_test_working_set_bytes(),
                ROUTED_READ_TEST_BODY_BYTES,
                {
                    let calls = std::sync::Arc::clone(&calls);
                    move |_route, _query| {
                    let calls = std::sync::Arc::clone(&calls);
                    async move {
                        match calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                            0 => crate::utils::respond_value_with_format(
                                norito::json!({
                                    "items": [{"id": "first", "timestamp_ms": 100}],
                                    "has_more": true
                                }),
                                ResponseFormat::Json,
                            ),
                            _ => torii_proxy_error_response(
                                StatusCode::SERVICE_UNAVAILABLE,
                                "route_unavailable",
                                "authoritative peers offline",
                            ),
                        }
                    }
                    }
                },
            )
            .await
            .expect_err("mid-route pagination failure must fail the fanout");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-succeeded")
                .and_then(|value| value.to_str().ok()),
            Some("0")
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_account_history_json_payloads_fails_on_mid_route_not_found() {
        let route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
        let params = routing::AccountHistoryGetParams {
            limit: Some(10),
            count_mode: Some("bounded".to_owned()),
            ..Default::default()
        };
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let response =
            collect_torii_account_history_json_payloads(
                &[route],
                &params,
                10,
                "bounded",
                routed_read_test_working_set_bytes(),
                ROUTED_READ_TEST_BODY_BYTES,
                {
                    let calls = std::sync::Arc::clone(&calls);
                    move |_route, _query| {
                    let calls = std::sync::Arc::clone(&calls);
                    async move {
                        match calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                            0 => crate::utils::respond_value_with_format(
                                norito::json!({
                                    "items": [{"id": "first", "timestamp_ms": 100}],
                                    "has_more": true
                                }),
                                ResponseFormat::Json,
                            ),
                            _ => torii_proxy_error_response(
                                StatusCode::NOT_FOUND,
                                "not_found",
                                "account history page missing",
                            ),
                        }
                    }
                    }
                },
            )
            .await
            .expect_err("mid-route not_found must not merge partial history");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("not_found")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-succeeded")
                .and_then(|value| value.to_str().ok()),
            Some("0")
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_account_history_single_route_preserves_index_metadata() {
        let authority = routed_read_test_account(0x91);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let response = execute_torii_account_history_read_for_routes(
            &app,
            vec![route],
            ToriiFanoutRouteScopeV1::AllDataspaces,
            vec![authority.to_string()],
            Some("limit=10&count_mode=exact".to_owned()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(
            json.get("query_source").and_then(Value::as_str),
            Some("account_history_index")
        );
        assert!(json.get("indexed_height").and_then(Value::as_u64).is_some());
        assert!(
            json.as_object()
                .is_some_and(|object| object.contains_key("indexed_block_hash"))
        );
        assert_eq!(
            json.get("count_mode").and_then(Value::as_str),
            Some("exact")
        );
        assert_eq!(json.get("has_more").and_then(Value::as_bool), Some(false));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_list_json_payloads_keeps_one_route_body_live_at_a_time() {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];
        let route_count = routes.len();
        let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let collected = collect_torii_list_json_payloads(
            &routes,
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            {
                let active = std::sync::Arc::clone(&active);
                move |route| {
                    let active = std::sync::Arc::clone(&active);
                async move {
                        assert_eq!(
                            active.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                            0,
                            "a second route began while the prior response body was live"
                        );
                        tokio::task::yield_now().await;
                        active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    let item =
                        crate::json_object(vec![crate::json_entry("route", route.dataspace_id)]);
                    let payload = crate::json_object(vec![
                        crate::json_entry("items", vec![item]),
                        crate::json_entry("total", 1_u64),
                    ]);
                    crate::utils::respond_value_with_format(payload, ResponseFormat::Json)
                }
                }
            },
        )
        .await
        .expect("all list routes should succeed");
        assert_eq!(collected.payloads.len(), route_count);
        assert_eq!(collected.diagnostics.succeeded_routes, route_count);
        assert_eq!(active.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_list_response_accepts_array_payloads_from_legacy_list_handlers() {
        let payloads = vec![
            norito::json!([{"id": "alpha"}]),
            norito::json!([{"id": "alpha"}, {"id": "beta"}]),
        ];

        let response = merged_list_response(payloads, "fanout", routed_read_test_budget())
            .expect("raw array list payloads should merge");
        let body = response_json(response).await;
        let root = body
            .as_object()
            .expect("merged list response should be an object");
        assert_eq!(root.get("total").and_then(Value::as_u64), Some(2));
        let items = root
            .get("items")
            .and_then(Value::as_array)
            .expect("merged list response should include items");
        assert_eq!(items.len(), 2);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn with_torii_fanout_headers_adds_warning_for_successful_alias_responses_with_denied_routes()
     {
        let mut diagnostics = ToriiFanoutDiagnostics::default();
        diagnostics.record_denied();
        diagnostics.record_attempt();
        diagnostics.record_success();

        let response = with_torii_fanout_headers(
            crate::utils::respond_value_with_format(
                norito::json!({"ok": true}),
                ResponseFormat::Json,
            ),
            diagnostics,
        );

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::WARNING)
                .and_then(|value| value.to_str().ok()),
            Some(r#"199 - "one or more alias routes were denied""#)
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-attempted")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-denied")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn with_torii_fanout_headers_does_not_add_warning_for_failed_alias_responses() {
        let mut diagnostics = ToriiFanoutDiagnostics::default();
        diagnostics.record_denied();

        let response = with_torii_fanout_headers(
            torii_alias_permission_denied_response("alias fanout denied"),
            diagnostics,
        );

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(
            response
                .headers()
                .get(axum::http::header::WARNING)
                .is_none(),
            "warning headers should only be added to successful responses"
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_alias_json_payloads_returns_permission_denied_when_only_synthetic_denials_exist()
     {
        let routes: &[RoutingDecision] = &[];
        let response = collect_torii_alias_json_payloads(
            routes,
            2,
            "alias fanout denied",
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            |_route: RoutingDecision| async move {
                crate::utils::respond_value_with_format(norito::json!({}), ResponseFormat::Json)
            },
        )
        .await
        .expect_err("synthetic denied routes without an allowed route should fail closed");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-denied")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-first-failure")
                .and_then(|value| value.to_str().ok()),
            Some("permission_denied")
        );
        let error = response_error(response).await;
        assert_eq!(error.code(), "permission_denied");
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_alias_json_payloads_returns_route_unavailable_when_no_routes_are_configured()
     {
        let routes: &[RoutingDecision] = &[];
        let response = collect_torii_alias_json_payloads(
            routes,
            0,
            "alias fanout denied",
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            |_route: RoutingDecision| async move {
                crate::utils::respond_value_with_format(norito::json!({}), ResponseFormat::Json)
            },
        )
        .await
        .expect_err("missing Nexus routes should surface route_unavailable");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_alias_json_payloads_prefers_explicit_permission_denied_when_no_route_resolves()
     {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];

        let response = collect_torii_alias_json_payloads(
            &routes,
            0,
            "alias fanout denied",
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            move |route| async move {
                if route == routes[0] {
                    torii_alias_permission_denied_response("routed dataspace blocked the lookup")
                } else {
                    torii_proxy_error_response(StatusCode::NOT_FOUND, "not_found", "missing")
                }
            },
        )
        .await
        .expect_err("a routed permission denial should outrank misses when no route succeeds");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-denied")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-not-found")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        let error = response_error(response).await;
        assert_eq!(error.code(), "permission_denied");
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_alias_json_payloads_keeps_success_when_other_routes_are_denied() {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];
        let expected = norito::json!({
            "account_id": "alice.i105.invalid",
            "total": 1,
            "items": [{
                "alias": "merchant@paynet",
                "dataspace": "paynet",
                "domain": null,
                "is_primary": true
            }],
            "source": "on_chain"
        });

        let collected =
            collect_torii_alias_json_payloads(
                &routes,
                0,
                "alias fanout denied",
                routed_read_test_working_set_bytes(),
                ROUTED_READ_TEST_BODY_BYTES,
                move |route| {
                    let expected = expected.clone();
                    async move {
                        if route == routes[0] {
                            torii_alias_permission_denied_response(
                                "routed dataspace blocked the lookup",
                            )
                        } else {
                            crate::utils::respond_value_with_format(expected, ResponseFormat::Json)
                        }
                    }
                },
            )
            .await
            .expect("successful alias payload should survive denied sibling routes");

        assert_eq!(collected.payloads.len(), 1);
        assert_eq!(collected.diagnostics.attempted_routes, 3);
        assert_eq!(collected.diagnostics.succeeded_routes, 1);
        assert_eq!(collected.diagnostics.denied_routes, 1);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_alias_json_payloads_prefers_not_found_over_route_unavailable_when_no_route_succeeds()
     {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];

        let response = collect_torii_alias_json_payloads(
            &routes,
            0,
            "alias fanout denied",
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            move |route| async move {
                if route == routes[0] {
                    torii_proxy_error_response(StatusCode::NOT_FOUND, "not_found", "missing")
                } else {
                    torii_proxy_error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "route_unavailable",
                        "authoritative peers offline",
                    )
                }
            },
        )
        .await
        .expect_err("not_found should outrank route_unavailable when no alias route succeeds");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-not-found")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-unavailable")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn collect_torii_alias_json_payloads_returns_route_unavailable_when_only_unavailable() {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];

        let response = collect_torii_alias_json_payloads(
            &routes,
            0,
            "alias fanout denied",
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            move |_route| async move {
                torii_proxy_error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "route_unavailable",
                    "authoritative peers offline",
                )
            },
        )
        .await
        .expect_err("all unavailable alias routes should surface route_unavailable");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-unavailable")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_torii_read_request_locally_alias_resolve_rejects_invalid_proxy_body() {
        let authority = routed_read_test_account(0x81);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let response = execute_torii_read_request_locally(
            &app,
            torii_read_request(
                ToriiReadEndpointV1::AliasResolve,
                route,
                Vec::new(),
                None,
                b"{".to_vec(),
            ),
            route,
            "local",
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_request")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_torii_read_request_locally_alias_resolve_index_rejects_invalid_proxy_body() {
        let authority = routed_read_test_account(0x82);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let response = execute_torii_read_request_locally(
            &app,
            torii_read_request(
                ToriiReadEndpointV1::AliasResolveIndex,
                route,
                Vec::new(),
                None,
                b"{".to_vec(),
            ),
            route,
            "local",
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_request")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_torii_read_request_locally_alias_lookup_by_account_rejects_invalid_proxy_body()
    {
        let authority = routed_read_test_account(0x83);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let response = execute_torii_read_request_locally(
            &app,
            torii_read_request(
                ToriiReadEndpointV1::AliasLookupByAccount,
                route,
                Vec::new(),
                None,
                b"{".to_vec(),
            ),
            route,
            "local",
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_request")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_torii_read_request_locally_alias_resolve_uses_route_local_alias() {
        let authority = routed_read_test_account(0x84);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        bind_account_alias_for_test(&app, &authority, "merchant@universal");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");
        let body = norito::json::to_vec(&routing::AliasResolveRequestDto {
            alias: "merchant@universal".to_string(),
        })
        .expect("encode request");

        let response = execute_torii_read_request_locally(
            &app,
            torii_read_request(
                ToriiReadEndpointV1::AliasResolve,
                route,
                Vec::new(),
                None,
                body,
            ),
            route,
            "local",
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("local")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-route-dataspace-id")
                .and_then(|value| value.to_str().ok()),
            Some("0")
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let payload: routing::AliasResolveResponseDto =
            norito::json::from_slice(&body).expect("alias-resolve response");
        assert_eq!(payload.alias, "merchant@universal");
        assert_eq!(payload.account_id, authority.to_string());
        assert_eq!(payload.source.as_deref(), Some("active_sns"));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn routed_alias_resolve_sanitizer_rejects_forged_account_payload() {
        let authority = routed_read_test_account(0x94);
        let forged = routed_read_test_account(0x95);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        bind_account_alias_for_test(&app, &authority, "merchant@universal");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");
        let request_body = norito::json::to_vec(&routing::AliasResolveRequestDto {
            alias: "merchant@universal".to_owned(),
        })
        .expect("encode request");
        let forged_body = norito::json::to_vec(&routing::AliasResolveResponseDto {
            alias: "merchant@universal".to_owned(),
            account_id: forged.to_string(),
            index: None,
            source: Some("active_sns".to_owned()),
        })
        .expect("encode forged response");
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(forged_body))
            .expect("forged response");

        let response = sanitize_exact_alias_route_response(
            &app,
            route,
            ToriiReadEndpointV1::AliasResolve,
            &request_body,
            response,
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn routed_alias_index_sanitizer_rejects_mismatched_alias_payload() {
        let authority = routed_read_test_account(0x96);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        bind_account_alias_for_test(&app, &authority, "merchant@universal");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");
        let request_body = norito::json::to_vec(&routing::AliasResolveIndexRequestDto { index: 0 })
            .expect("encode request");
        let forged_body = norito::json::to_vec(&routing::AliasResolveIndexResponseDto {
            index: 0,
            alias: "attacker@universal".to_owned(),
            account_id: authority.to_string(),
            source: Some("active_sns".to_owned()),
        })
        .expect("encode forged response");
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(forged_body))
            .expect("forged response");

        let response = sanitize_exact_alias_route_response(
            &app,
            route,
            ToriiReadEndpointV1::AliasResolveIndex,
            &request_body,
            response,
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn routed_contract_alias_sanitizer_rejects_forged_subject_payload() {
        let authority = routed_read_test_account(0x98);
        let forged_subject = routed_read_test_account(0x99);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        bind_contract_alias_for_test(&app, &contract_address, "router::universal");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");
        let request_body = norito::json::to_vec(&routing::ContractAliasResolveRequestDto {
            contract_alias: "router::universal".to_owned(),
        })
        .expect("encode request");
        let forged_body = norito::json::to_vec(&routing::ContractAliasResolveResponseDto {
            contract_alias: "router::universal".to_owned(),
            contract_address: contract_address.to_string(),
            contract_subject_account: forged_subject.to_string(),
            dataspace: "universal".to_owned(),
            contract_alias_binding: routing::ContractAliasBindingDto {
                alias: "router::universal".to_owned(),
                status: "permanent".to_owned(),
                lease_expiry_ms: None,
                grace_until_ms: None,
                bound_at_ms: 0,
            },
            source: "world_state".to_owned(),
        })
        .expect("encode forged response");
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(forged_body))
            .expect("forged response");

        let response = sanitize_exact_alias_route_response(
            &app,
            route,
            ToriiReadEndpointV1::ContractAliasResolve,
            &request_body,
            response,
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn protected_alias_reads_ignore_unsigned_public_upstream() {
        let authority = routed_read_test_account(0x97);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        bind_account_alias_for_test(&app, &authority, "merchant@universal");
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            1,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        bind_contract_alias_for_test(&app, &contract_address, "router::universal");
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .public_dataspace_upstreams = Arc::new(BTreeMap::from([(
            DataSpaceId::UNIVERSAL,
            "http://127.0.0.1:9".to_owned(),
        )]));
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");
        let body = norito::json::to_vec(&routing::AliasResolveIndexRequestDto { index: 0 })
            .expect("encode request");

        let response = execute_torii_read_for_route(
            &app,
            route,
            torii_read_request(
                ToriiReadEndpointV1::AliasResolveIndex,
                route,
                Vec::new(),
                None,
                body,
            ),
            None,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("local")
        );

        for (endpoint, body) in [
            (
                ToriiReadEndpointV1::AliasResolve,
                norito::json::to_vec(&routing::AliasResolveRequestDto {
                    alias: "merchant@universal".to_owned(),
                })
                .expect("encode alias request"),
            ),
            (
                ToriiReadEndpointV1::ContractAliasResolve,
                norito::json::to_vec(&routing::ContractAliasResolveRequestDto {
                    contract_alias: "router::universal".to_owned(),
                })
                .expect("encode contract alias request"),
            ),
        ] {
            let response = execute_torii_read_for_route(
                &app,
                route,
                torii_read_request(endpoint, route, Vec::new(), None, body),
                None,
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response
                    .headers()
                    .get("x-iroha-routed-by")
                    .and_then(|value| value.to_str().ok()),
                Some("local")
            );
        }
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn exact_alias_resolve_rejects_expired_authoritative_lease() {
        let authority = routed_read_test_account(0x98);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let alias_literal = "merchant@universal";
        bind_account_alias_for_test(&app, &authority, alias_literal);
        let catalog = app.state.nexus_snapshot().dataspace_catalog;
        let alias = AccountAlias::from_literal(alias_literal, &catalog).expect("account alias");
        let selector = iroha_core::sns::selector_for_account_alias(&alias, &catalog)
            .expect("account alias selector");
        let account_address = AccountAddress::from_account_id(&authority).expect("account address");
        let expired = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            authority.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(
                &account_address,
            )],
            0,
            0,
            1,
            1,
            1,
            iroha_data_model::metadata::Metadata::default(),
        );
        let height = app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let header = BlockHeader::new(
            std::num::NonZeroU64::new(height).expect("nonzero height"),
            None,
            None,
            None,
            2,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        tx.world_mut_for_testing()
            .smart_contract_state_mut_for_testing()
            .insert(
                iroha_core::sns::record_storage_key(&selector),
                norito::codec::Encode::encode(&expired),
            );
        tx.apply();
        block.commit().expect("commit expired alias lease");

        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");
        let response = execute_alias_resolve_local_read(
            &app,
            route,
            &routing::AliasResolveRequestDto {
                alias: alias_literal.to_owned(),
            },
        )
        .expect("expired alias should produce a not-found response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn exact_alias_resolve_rejects_rekey_index_split_brain() {
        let authority = routed_read_test_account(0x99);
        let forged = routed_read_test_account(0x9a);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let alias_literal = "merchant@universal";
        bind_account_alias_for_test(&app, &authority, alias_literal);
        let alias = AccountAlias::from_literal(
            alias_literal,
            &app.state.nexus_snapshot().dataspace_catalog,
        )
        .expect("account alias");
        let height = app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let header = BlockHeader::new(
            std::num::NonZeroU64::new(height).expect("nonzero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        tx.world_mut_for_testing()
            .account_rekey_records_mut_for_testing()
            .insert(
                alias.clone(),
                iroha_data_model::account::rekey::AccountRekeyRecord::new(alias, forged),
            );
        tx.apply();
        block.commit().expect("commit split-brain rekey record");

        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");
        let response = execute_alias_resolve_local_read(
            &app,
            route,
            &routing::AliasResolveRequestDto {
                alias: alias_literal.to_owned(),
            },
        )
        .expect("split-brain alias should produce a not-found response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_torii_read_request_locally_alias_resolve_index_uses_route_local_index() {
        let authority = routed_read_test_account(0x85);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@universal");
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::new(1))
            .expect("secondary route");
        let body = norito::json::to_vec(&routing::AliasResolveIndexRequestDto { index: 0 })
            .expect("encode request");

        let response = execute_torii_read_request_locally(
            &app,
            torii_read_request(
                ToriiReadEndpointV1::AliasResolveIndex,
                route,
                Vec::new(),
                None,
                body,
            ),
            route,
            "local",
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("local")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-route-dataspace-id")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let payload: routing::AliasResolveIndexResponseDto =
            norito::json::from_slice(&body).expect("alias-index response");
        assert_eq!(payload.index, 0);
        assert_eq!(payload.alias, "merchant@secondary");
        assert_eq!(payload.account_id, authority.to_string());
        assert_eq!(payload.source.as_deref(), Some("active_sns"));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_alias_resolve_index_local_read_returns_not_found_for_route_without_aliases() {
        let authority = routed_read_test_account(0x86);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let response = execute_alias_resolve_index_local_read(
            &app,
            route,
            &routing::AliasResolveIndexRequestDto { index: 0 },
        )
        .expect("route-local alias-index read should return a response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_torii_read_request_locally_alias_lookup_by_account_filters_items_to_route_dataspace()
     {
        let authority = routed_read_test_account(0x87);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@universal");
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::new(1))
            .expect("secondary route");
        let body = norito::json::to_vec(&routing::AliasLookupByAccountRequestDto {
            account_id: authority.to_string(),
            dataspace: None,
            domain: None,
        })
        .expect("encode request");

        let response = execute_torii_read_request_locally(
            &app,
            torii_read_request(
                ToriiReadEndpointV1::AliasLookupByAccount,
                route,
                Vec::new(),
                None,
                body,
            ),
            route,
            "local",
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-route-dataspace-id")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let payload: routing::AliasLookupByAccountResponseDto =
            norito::json::from_slice(&body).expect("alias by-account response");
        assert_eq!(payload.account_id, authority.to_string());
        assert_eq!(payload.total, 1);
        assert_eq!(payload.items.len(), 1);
        assert_eq!(payload.items[0].alias, "merchant@secondary");
        assert_eq!(payload.items[0].dataspace, "secondary");
        assert_eq!(payload.source.as_deref(), Some("on_chain"));
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn execute_alias_resolve_local_read_rejects_empty_alias() {
        let authority = routed_read_test_account(0x88);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let err = execute_alias_resolve_local_read(
            &app,
            route,
            &routing::AliasResolveRequestDto {
                alias: "   ".to_string(),
            },
        )
        .expect_err("empty aliases should be rejected before local execution");

        match err {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => assert_eq!(message, "alias must not be empty"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn execute_alias_resolve_local_read_returns_not_found_for_route_mismatch() {
        let authority = routed_read_test_account(0x89);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let response = execute_alias_resolve_local_read(
            &app,
            route,
            &routing::AliasResolveRequestDto {
                alias: "merchant@secondary".to_string(),
            },
        )
        .expect("route-mismatched local alias resolve should return a response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn execute_alias_lookup_by_account_local_read_rejects_empty_account_id() {
        let authority = routed_read_test_account(0x8a);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let err = execute_alias_lookup_by_account_local_read(
            &app,
            route,
            &routing::AliasLookupByAccountRequestDto {
                account_id: " ".to_string(),
                dataspace: None,
                domain: None,
            },
        )
        .expect_err("empty account ids should be rejected before local execution");

        match err {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => assert_eq!(message, "account_id must not be empty"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn execute_alias_lookup_by_account_local_read_rejects_invalid_account_id() {
        let authority = routed_read_test_account(0x8b);
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let err = execute_alias_lookup_by_account_local_read(
            &app,
            route,
            &routing::AliasLookupByAccountRequestDto {
                account_id: "not-an-account".to_string(),
                dataspace: None,
                domain: None,
            },
        )
        .expect_err("invalid account ids should be rejected before route-local lookup");

        match err {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => assert!(
                message.starts_with("invalid account_id:"),
                "unexpected conversion message: {message}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn execute_alias_lookup_by_account_local_read_returns_empty_items_when_route_filters_out_aliases()
     {
        let authority = routed_read_test_account(0x8c);
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");
        let route = resolve_torii_route_for_dataspace_id(app.as_ref(), DataSpaceId::UNIVERSAL)
            .expect("universal route");

        let response = execute_alias_lookup_by_account_local_read(
            &app,
            route,
            &routing::AliasLookupByAccountRequestDto {
                account_id: authority.to_string(),
                dataspace: None,
                domain: None,
            },
        )
        .expect("route-local alias-by-account should return an empty payload when filters remove every alias");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let payload: routing::AliasLookupByAccountResponseDto =
            norito::json::from_slice(&body).expect("alias by-account response");
        assert_eq!(payload.account_id, authority.to_string());
        assert_eq!(payload.total, 0);
        assert!(payload.items.is_empty());
        assert_eq!(payload.source.as_deref(), Some("on_chain"));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn torii_partition_routes_by_visibility_counts_private_dataspaces_as_denied_for_unsigned_reads()
     {
        let authority = routed_read_test_account(0x8d);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::partition-unsigned"));
        let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
            &authority,
            uaid,
            DataSpaceId::new(10),
        ));
        configure_private_ingress_routes_for_test(&mut app);

        let (allowed_routes, denied_routes) = torii_partition_routes_by_visibility(
            &app,
            torii_all_dataspace_routes(app.as_ref()),
            &ToriiAccountReadVisibility::None,
        );
        let dataspaces = allowed_routes
            .into_iter()
            .map(|route| route.dataspace_id)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            dataspaces,
            std::collections::BTreeSet::from([DataSpaceId::UNIVERSAL, DataSpaceId::new(1)])
        );
        assert_eq!(denied_routes, 1);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn torii_visibility_account_from_headers_rejects_unsigned_account_header() {
        let key_pair = routed_read_ed25519_test_keypair(0x8e);
        let authority = AccountId::new(key_pair.public_key().clone());
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{authority}/transactions")
            .parse()
            .expect("valid URI");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            authority.to_string().parse().expect("account header"),
        );

        let err = torii_visibility_account_from_headers(
            &app,
            &headers,
            &method,
            &uri,
            &[],
            routing::ENDPOINT_ACCOUNTS_TRANSACTIONS,
        )
        .expect_err("bare account headers must not create caller identity");

        match err {
            Error::Query(iroha_data_model::ValidationFail::NotPermitted(message)) => {
                assert!(
                    message.contains("requires canonical request signing"),
                    "unexpected rejection message: {message}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn torii_visibility_account_from_headers_accepts_signed_caller() {
        let key_pair = routed_read_ed25519_test_keypair(0x8f);
        let authority = AccountId::new(key_pair.public_key().clone());
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{authority}/transactions")
            .parse()
            .expect("valid URI");
        let headers = crate::tests_runtime_handlers::signed_app_headers(
            &authority,
            &key_pair,
            &method,
            &uri,
            &[],
        );

        let visibility = torii_visibility_account_from_headers(
            &app,
            &headers,
            &method,
            &uri,
            &[],
            routing::ENDPOINT_ACCOUNTS_TRANSACTIONS,
        )
        .expect("signed account headers should verify");

        match visibility {
            ToriiAccountReadVisibility::Signed(account) => assert_eq!(account, authority),
            other => panic!("unexpected visibility: {other:?}"),
        }
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn torii_partition_routes_by_visibility_allows_bound_private_dataspaces_for_signed_caller()
     {
        let authority = routed_read_test_account(0x90);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::partition-caller"));
        let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
            &authority,
            uaid,
            DataSpaceId::new(10),
        ));
        configure_private_ingress_routes_for_test(&mut app);

        let (allowed_routes, denied_routes) = torii_partition_routes_by_visibility(
            &app,
            torii_all_dataspace_routes(app.as_ref()),
            &ToriiAccountReadVisibility::Signed(authority.clone()),
        );
        let dataspaces = allowed_routes
            .into_iter()
            .map(|route| route.dataspace_id)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            dataspaces,
            std::collections::BTreeSet::from([
                DataSpaceId::UNIVERSAL,
                DataSpaceId::new(1),
                DataSpaceId::new(10),
            ])
        );
        assert_eq!(denied_routes, 0);
    }

    #[tokio::test]
    async fn merged_list_response_deduplicates_items_and_sets_total() {
        let response = merged_list_response(
            vec![
                norito::json!({
                    "items": [{"id": "a"}, {"id": "b"}],
                    "total": 2
                }),
                norito::json!({
                    "items": [{"id": "b"}, {"id": "c"}],
                    "total": 2
                }),
            ],
            "proxy",
            routed_read_test_budget(),
        )
        .expect("list merge should succeed");

        assert_eq!(
            response
                .headers()
                .get("x-iroha-routed-by")
                .expect("routed-by header should be present"),
            "proxy"
        );

        let json = response_json(response).await;
        let items = json["items"]
            .as_array()
            .expect("merged response should include items");
        let ids: Vec<&str> = items
            .iter()
            .map(|item| item["id"].as_str().expect("each item should have an id"))
            .collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
        assert_eq!(json["total"].as_u64(), Some(3));
    }

    #[tokio::test]
    async fn merged_list_response_preserves_first_seen_order() {
        let response = merged_list_response(
            vec![
                norito::json!({
                    "items": [{"id": "b"}, {"id": "a"}],
                    "total": 2
                }),
                norito::json!({
                    "items": [{"id": "a"}, {"id": "c"}],
                    "total": 2
                }),
            ],
            "proxy",
            routed_read_test_budget(),
        )
        .expect("list merge should succeed");

        let json = response_json(response).await;
        let items = json["items"]
            .as_array()
            .expect("merged response should include items");
        let ids: Vec<&str> = items
            .iter()
            .map(|item| item["id"].as_str().expect("each item should have an id"))
            .collect();
        assert_eq!(ids, vec!["b", "a", "c"]);
    }

    #[tokio::test]
    async fn paginated_accounts_fanout_drains_deduplicates_and_pages_globally() {
        let routes = [
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
        ];
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let collected = collect_torii_paginated_list_json_payloads(
            &routes,
            2,
            routed_read_test_working_set_bytes(),
            ROUTED_READ_TEST_BODY_BYTES,
            {
                let calls = std::sync::Arc::clone(&calls);
                move |route, offset, limit| {
                let calls = std::sync::Arc::clone(&calls);
                async move {
                    calls.lock().expect("call log lock").push((
                        route.dataspace_id.as_u64(),
                        offset,
                        limit,
                    ));
                    let payload = match (route.dataspace_id.as_u64(), offset) {
                        (1, 0) => norito::json!({
                            "items": [{"id": "a"}, {"id": "b"}],
                            "total": 3,
                            "has_more": true,
                            "count_mode": "exact"
                        }),
                        (1, 2) => norito::json!({
                            "items": [{"id": "c"}],
                            "total": 3,
                            "has_more": false,
                            "count_mode": "exact"
                        }),
                        (2, 0) => norito::json!({
                            "items": [{"id": "b"}, {"id": "d"}],
                            "total": 2,
                            "has_more": false,
                            "count_mode": "exact"
                        }),
                        other => panic!("unexpected routed page request: {other:?}"),
                    };
                    crate::utils::respond_value_with_format(payload, ResponseFormat::Json)
                }
                }
            },
        )
        .await
        .expect("all routed pages should validate");

        assert_eq!(collected.diagnostics.succeeded_routes, 2);
        assert_eq!(
            *calls.lock().expect("call log lock"),
            vec![(1, 0, 2), (1, 2, 2), (2, 0, 2)]
        );
        let response = merged_paginated_list_response(
            collected.payloads,
            1,
            2,
            "exact",
            "proxy",
            collected.budget,
        )
        .expect("drained account pages should merge");
        let json = response_json(response).await;
        let root = json.as_object().expect("merged response object");
        assert_eq!(root.len(), 4);
        assert_eq!(root.get("total").and_then(Value::as_u64), Some(4));
        assert_eq!(root.get("has_more").and_then(Value::as_bool), Some(true));
        assert_eq!(
            root.get("count_mode").and_then(Value::as_str),
            Some("exact")
        );
        let ids = root
            .get("items")
            .and_then(Value::as_array)
            .expect("merged items")
            .iter()
            .map(|item| item["id"].as_str().expect("item id"))
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["b", "c"]);
    }

    #[test]
    fn routed_account_page_rejects_missing_pagination_metadata() {
        let response =
            validate_torii_exact_list_page(&norito::json!({"items": [], "total": 0}), 0, 100, None)
                .expect_err("fanout must not accept an unverifiable partial account inventory");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[tokio::test]
    async fn merged_account_history_response_sorts_deduplicates_and_pages_globally() {
        let response = merged_account_history_response(
            vec![
                norito::json!({
                    "items": [
                        {"id": "old", "timestamp_ms": 100, "account_id": "alice"},
                        {"id": "new", "timestamp_ms": 300, "account_id": "alice"}
                    ],
                    "total": 2
                }),
                norito::json!({
                    "items": [
                        {"id": "mid", "timestamp_ms": 200, "account_id": "alice"},
                        {"id": "old", "timestamp_ms": 100, "account_id": "alice"}
                    ],
                    "total": 2
                }),
            ],
            1,
            2,
            "exact",
            "proxy",
            routed_read_test_budget(),
        )
        .expect("account history merge should succeed");

        let json = response_json(response).await;
        let ids = json["items"]
            .as_array()
            .expect("merged account history should include items")
            .iter()
            .map(|item| item["id"].as_str().expect("item id should be present"))
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["mid", "old"]);
        assert_eq!(json["total"].as_u64(), Some(3));
        assert_eq!(json["has_more"].as_bool(), Some(false));
        assert_eq!(json["count_mode"].as_str(), Some("exact"));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_alias_resolve_index_response_deduplicates_identical_bindings() {
        let payload = norito::json!({
            "index": 7,
            "alias": "merchant@paynet",
            "account_id": "alice.i105.invalid",
            "source": "on_chain"
        });

        let response = merged_alias_resolve_index_response(
            vec![payload.clone(), payload],
            "proxy",
            "fanout",
            routed_read_test_budget(),
        )
        .expect("identical alias-index bindings should merge cleanly");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("proxy")
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let payload: routing::AliasResolveIndexResponseDto =
            norito::json::from_slice(&body).expect("alias-index response");
        assert_eq!(payload.index, 7);
        assert_eq!(payload.alias, "merchant@paynet");
        assert_eq!(payload.account_id, "alice.i105.invalid");
        assert_eq!(payload.source.as_deref(), Some("fanout"));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_alias_resolve_index_response_rejects_conflicting_bindings() {
        let response = merged_alias_resolve_index_response(
            vec![
                norito::json!({
                    "index": 7,
                    "alias": "merchant@paynet",
                    "account_id": "alice.i105.invalid",
                    "source": "on_chain"
                }),
                norito::json!({
                    "index": 7,
                    "alias": "merchant@aed",
                    "account_id": "alice.i105.invalid",
                    "source": "on_chain"
                }),
            ],
            "proxy",
            "fanout",
            routed_read_test_budget(),
        )
        .expect_err("incompatible alias-index bindings should conflict");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_conflict")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_alias_resolve_index_response_rejects_malformed_payload() {
        let response = merged_alias_resolve_index_response(
            vec![norito::json!({
                "alias": "merchant@paynet",
                "account_id": "alice.i105.invalid"
            })],
            "proxy",
            "fanout",
            routed_read_test_budget(),
        )
        .expect_err("malformed alias-index payloads should fail decoding");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_alias_resolve_index_response_returns_not_found_when_payloads_are_empty() {
        let response = merged_alias_resolve_index_response(
            Vec::new(),
            "proxy",
            "fanout",
            routed_read_test_budget(),
        )
        .expect_err("empty alias-index merges should surface not_found");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("not_found")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_alias_lookup_by_account_response_deduplicates_items_and_recomputes_total() {
        let response = merged_alias_lookup_by_account_response(
            vec![
                norito::json!({
                    "account_id": "alice.i105.invalid",
                    "total": 2,
                    "items": [{
                        "alias": "merchant@paynet",
                        "dataspace": "paynet",
                        "domain": null,
                        "is_primary": true
                    }, {
                        "alias": "merchant@banka.paynet",
                        "dataspace": "paynet",
                        "domain": "banka",
                        "is_primary": false
                    }],
                    "source": "on_chain"
                }),
                norito::json!({
                    "account_id": "alice.i105.invalid",
                    "total": 1,
                    "items": [{
                        "alias": "merchant@paynet",
                        "dataspace": "paynet",
                        "domain": null,
                        "is_primary": true
                    }],
                    "source": "on_chain"
                }),
            ],
            "proxy",
            "fanout",
            0,
            routed_read_test_budget(),
        )
        .expect("compatible alias-by-account payloads should merge");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let payload: routing::AliasLookupByAccountResponseDto =
            norito::json::from_slice(&body).expect("alias by-account response");
        assert_eq!(payload.account_id, "alice.i105.invalid");
        assert_eq!(payload.total, 2);
        assert_eq!(payload.items.len(), 2);
        assert_eq!(payload.source.as_deref(), Some("fanout"));
        assert_eq!(payload.items[0].alias, "merchant@paynet");
        assert_eq!(payload.items[1].alias, "merchant@banka.paynet");
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_alias_lookup_by_account_response_rejects_malformed_payload() {
        let response = merged_alias_lookup_by_account_response(
            vec![norito::json!({
                "total": 1,
                "items": [],
                "source": "on_chain"
            })],
            "proxy",
            "fanout",
            0,
            routed_read_test_budget(),
        )
        .expect_err("malformed alias-account payloads should fail decoding");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_alias_lookup_by_account_response_rejects_conflicting_account_roots() {
        let response = merged_alias_lookup_by_account_response(
            vec![
                norito::json!({
                    "account_id": "alice.i105.invalid",
                    "total": 1,
                    "items": [],
                    "source": "on_chain"
                }),
                norito::json!({
                    "account_id": "bob.i105.invalid",
                    "total": 1,
                    "items": [],
                    "source": "on_chain"
                }),
            ],
            "proxy",
            "fanout",
            0,
            routed_read_test_budget(),
        )
        .expect_err("conflicting account roots should fail");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_conflict")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_alias_lookup_by_account_response_returns_permission_denied_when_items_are_empty_and_denied_routes_exist()
     {
        let response = merged_alias_lookup_by_account_response(
            vec![norito::json!({
                "account_id": "alice.i105.invalid",
                "total": 0,
                "items": [],
                "source": "on_chain"
            })],
            "proxy",
            "fanout",
            1,
            routed_read_test_budget(),
        )
        .expect_err("empty merged alias rows with denied routes should fail closed");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let error = response_error(response).await;
        assert_eq!(error.code(), "permission_denied");
    }

    #[tokio::test]
    async fn merged_singleton_response_rejects_conflicting_payloads() {
        let response = merged_singleton_response(
            vec![
                norito::json!({"id": "asset#a"}),
                norito::json!({"id": "asset#b"}),
            ],
            "proxy",
            routed_read_test_budget(),
        )
        .expect_err("conflicting singleton payloads should fail");

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn merged_pipeline_status_response_prefers_highest_semantic_status() {
        let response = merged_pipeline_status_response(
            vec![
                norito::json!({
                    "hash": "abc",
                    "status": {"kind": "Committed", "block_height": 7},
                    "scope": "global",
                    "resolved_from": "cache"
                }),
                norito::json!({
                    "hash": "abc",
                    "status": {"kind": "Applied", "block_height": 7},
                    "scope": "global",
                    "resolved_from": "state"
                }),
            ],
            "proxy",
            routed_read_test_budget(),
        )
        .expect("pipeline status fanout should merge semantically compatible statuses");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: PipelineTransactionStatusResponse =
            norito::json::from_slice(&body).expect("status payload");
        assert_eq!(payload.status.kind, "Applied");
        assert_eq!(payload.resolved_from, "state");
    }

    #[tokio::test]
    async fn merged_pipeline_status_response_prefers_applied_over_cached_rejection() {
        let response = merged_pipeline_status_response(
            vec![
                norito::json!({
                    "hash": "abc",
                    "status": {
                        "kind": "Rejected",
                        "block_height": null
                    },
                    "scope": "global",
                    "resolved_from": "cache"
                }),
                norito::json!({
                    "hash": "abc",
                    "status": {"kind": "Applied", "block_height": 7},
                    "scope": "global",
                    "resolved_from": "state"
                }),
            ],
            "proxy",
            routed_read_test_budget(),
        )
        .expect("pipeline status fanout should prefer committed success over stale rejection");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: PipelineTransactionStatusResponse =
            norito::json::from_slice(&body).expect("status payload");
        assert_eq!(payload.status.kind, "Applied");
        assert_eq!(payload.resolved_from, "state");
    }

    fn pipeline_status_hint_response(kind: &str, resolved_from: &str) -> Response {
        crate::utils::respond_with_format(
            PipelineTransactionStatusResponse::new(
                "abc".to_owned(),
                PipelineTransactionStatus {
                    kind: kind.to_owned(),
                    block_height: Some(7),
                },
                "global".to_owned(),
                resolved_from.to_owned(),
            ),
            ResponseFormat::Json,
        )
    }

    #[tokio::test]
    async fn pipeline_status_hint_ignores_non_terminal_successes() {
        for kind in ["Queued", "Approved", "Committed"] {
            let response = pipeline_status_hint_response(kind, "cache");

            let hinted =
                pipeline_status_hinted_global_response(response, ROUTED_READ_TEST_BODY_BYTES)
                    .await
                    .expect("hint classifier should not fail");

            assert!(
                hinted.is_none(),
                "global status must fan out instead of trusting hinted {kind} state"
            );
        }
    }

    #[tokio::test]
    async fn pipeline_status_hint_ignores_cached_negative_terminal_statuses() {
        for kind in ["Rejected", "Expired"] {
            let response = pipeline_status_hint_response(kind, "cache");

            let hinted =
                pipeline_status_hinted_global_response(response, ROUTED_READ_TEST_BODY_BYTES)
                    .await
                    .expect("hint classifier should not fail");

            assert!(
                hinted.is_none(),
                "global status must fan out instead of trusting hinted cached {kind}"
            );
        }
    }

    #[tokio::test]
    async fn pipeline_status_hint_allows_authoritative_terminal_statuses() {
        for (kind, resolved_from) in [("Applied", "cache"), ("Rejected", "state")] {
            let response = pipeline_status_hint_response(kind, resolved_from);

            let hinted =
                pipeline_status_hinted_global_response(response, ROUTED_READ_TEST_BODY_BYTES)
                    .await
                    .expect("hint classifier should not fail")
                    .expect("authoritative hinted status may short-circuit");
            let body = axum::body::to_bytes(hinted.into_body(), usize::MAX)
                .await
                .expect("hinted body should be readable");
            let payload: PipelineTransactionStatusResponse =
                norito::json::from_slice(&body).expect("status payload");

            assert_eq!(payload.status.kind, kind);
            assert_eq!(payload.resolved_from, resolved_from);
        }
    }

    #[tokio::test]
    async fn pipeline_status_hint_ignores_malformed_success() {
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(br#"{"status":{"kind":"Queued"}"#.as_slice()))
            .expect("malformed JSON response");

        let hinted = pipeline_status_hinted_global_response(response, ROUTED_READ_TEST_BODY_BYTES)
            .await
            .expect("malformed hinted success should fall through to fanout");

        assert!(hinted.is_none());
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_bindings_response_unions_accounts_and_omits_singular_route() {
        let response = merged_space_directory_bindings_response(
            vec![
                norito::json!({
                    "uaid": "uaid:alice",
                    "dataspaces": [{
                        "dataspace_id": 2,
                        "dataspace_alias": "centralbank",
                        "accounts": ["alice@centralbank", "bob@centralbank"]
                    }]
                }),
                norito::json!({
                    "uaid": "uaid:alice",
                    "dataspaces": [{
                        "dataspace_id": 2,
                        "dataspace_alias": "centralbank",
                        "accounts": ["alice@centralbank"]
                    }, {
                        "dataspace_id": 7,
                        "dataspace_alias": "ops",
                        "accounts": ["carol@ops"]
                    }]
                }),
            ],
            "proxy",
            routed_read_test_budget(),
        )
        .expect("bindings merge should succeed");

        assert_eq!(
            response
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("proxy")
        );
        assert!(
            response.headers().get("x-iroha-route-lane-id").is_none(),
            "fanout merge must not report a singular lane"
        );
        assert!(
            response
                .headers()
                .get("x-iroha-route-dataspace-id")
                .is_none(),
            "fanout merge must not report a singular dataspace"
        );

        let json = response_json(response).await;
        assert_eq!(json["uaid"].as_str(), Some("uaid:alice"));
        let dataspaces = json["dataspaces"]
            .as_array()
            .expect("bindings merge should expose dataspaces");
        assert_eq!(dataspaces.len(), 2);
        assert_eq!(dataspaces[0]["dataspace_id"].as_u64(), Some(2));
        assert_eq!(
            dataspaces[0]["dataspace_alias"].as_str(),
            Some("centralbank")
        );
        assert_eq!(
            dataspaces[0]["accounts"].as_array().expect("accounts"),
            &vec![
                Value::from("alice@centralbank"),
                Value::from("bob@centralbank"),
            ]
        );
        assert_eq!(dataspaces[1]["dataspace_id"].as_u64(), Some(7));
        assert_eq!(dataspaces[1]["dataspace_alias"].as_str(), Some("ops"));
    }
