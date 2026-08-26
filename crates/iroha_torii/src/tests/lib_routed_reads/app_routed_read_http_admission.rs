// Exact public-HTTP admission regressions for all application routed reads.
mod app_routed_read_http_admission_tests {
    use super::*;
    use axum::{
        Router,
        body::{Body, Bytes},
        http::{HeaderMap, HeaderValue, Request, StatusCode, header},
        middleware::{Next, from_fn, from_fn_with_state},
        response::{IntoResponse as _, Response},
        routing::any,
    };
    use std::{
        convert::Infallible,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::Poll,
    };
    use tower::ServiceExt as _;
    fn pending_body(polls: &Arc<AtomicUsize>) -> Body {
        let polls = Arc::clone(polls);
        Body::from_stream(futures::stream::poll_fn(move |_| {
            polls.fetch_add(1, Ordering::SeqCst);
            Poll::<Option<Result<Bytes, Infallible>>>::Pending
        }))
    }
    async fn insert_test_route_metadata(
        descriptor: iroha_torii_shared::route_catalog::RouteDescriptor,
        mut request: Request<Body>,
        next: Next,
    ) -> Response {
        request
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(descriptor));
        next.run(request).await
    }
    async fn test_auth_gate(request: Request<Body>, next: Next) -> Response {
        if request.headers().get("x-test-auth").is_none() {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        next.run(request).await
    }
    fn admission_router(
        app: SharedAppState,
        descriptor: iroha_torii_shared::route_catalog::RouteDescriptor,
        authenticate_first: bool,
    ) -> Router {
        let path = descriptor.path();
        let mut router = Router::new()
            .route(path, any(|| async { StatusCode::NO_CONTENT }))
            .layer(from_fn_with_state(
                app,
                enforce_app_routed_read_http_admission,
            ))
            .layer(from_fn(move |request, next| {
                insert_test_route_metadata(descriptor, request, next)
            }));
        if authenticate_first {
            router = router.layer(from_fn(test_auth_gate));
        }
        router
    }
    fn json_request(path: &str, body: Body) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .expect("valid test request")
    }
    macro_rules! define_endpoint_inventory {
        ($($endpoint:ident),+ $(,)?) => {
            const ENDPOINT_INVENTORY: &[ToriiReadEndpointV1] = &[
                $(ToriiReadEndpointV1::$endpoint),+
            ];
            fn endpoint_inventory_is_exhaustive(endpoint: ToriiReadEndpointV1) -> bool {
                match endpoint {
                    $(ToriiReadEndpointV1::$endpoint => true),+
                }
            }
        };
    }
    define_endpoint_inventory!(
        AccountGet,
        ExplorerAccountDetail,
        AccountAssetsGet,
        AccountAssetsQuery,
        AccountPermissionsGet,
        AccountTransactionsGet,
        AccountTransactionsQuery,
        TransactionsQuery,
        PipelineTransactionStatusGet,
        ProofRecordGet,
        AccountsList,
        AccountsQuery,
        AccountsPortfolio,
        AssetDefinitionsList,
        AssetDefinitionGet,
        AssetDefinitionsQuery,
        AssetHoldersGet,
        AssetHoldersQuery,
        DomainsList,
        DomainsQuery,
        NftsList,
        NftsQuery,
        NexusPublicLaneValidators,
        NexusPublicLaneStake,
        NexusPublicLaneRewards,
        NexusDataspacesAccountSummary,
        SpaceDirectoryBindingsGet,
        SpaceDirectoryManifestsGet,
        RwasList,
        RwasQuery,
        AliasResolve,
        AliasResolveIndex,
        AliasLookupByAccount,
        ExplorerAssetDefinitionDetail,
        ExplorerAssetDefinitionEconometrics,
        ExplorerAssetDefinitionSnapshot,
        ContractAliasResolve,
        ContractStateGet,
        ContractViewPost,
        ContractViewBatchPost,
        AccountHistoryGet,
        InternalAccountGet,
        InternalAccountTransactionGet,
        InternalAccountAssetGet,
        ContractDeploymentState,
        AccountOnboardingCurrentState,
    );
    #[test]
    fn all_46_endpoint_and_stable_route_id_mappings_are_exact_and_unique() {
        let expected = ENDPOINT_INVENTORY;
        assert_eq!(APP_ROUTED_READ_HTTP_ENDPOINTS_V1.len(), expected.len());
        for &endpoint in expected {
            assert!(endpoint_inventory_is_exhaustive(endpoint));
            assert_eq!(
                APP_ROUTED_READ_HTTP_ENDPOINTS_V1
                    .iter()
                    .filter(|entry| entry.endpoint == endpoint)
                    .count(),
                1,
                "endpoint {endpoint:?} must have exactly one public HTTP admission entry"
            );
        }
        let mut route_ids = std::collections::BTreeSet::new();
        for entry in APP_ROUTED_READ_HTTP_ENDPOINTS_V1 {
            assert!(route_ids.insert(entry.route.stable_route_id()));
            assert_eq!(
                app_routed_read_http_endpoint(entry.route.stable_route_id())
                    .expect("catalog route must resolve")
                    .endpoint,
                entry.endpoint
            );
            match entry.decoder {
                AppRoutedReadHttpDecoder::None
                | AppRoutedReadHttpDecoder::ExactInternalAssetScope => {
                    assert!(entry.decoder.typed_request_name().is_none());
                }
                AppRoutedReadHttpDecoder::Query(name)
                | AppRoutedReadHttpDecoder::StringQuery(name)
                | AppRoutedReadHttpDecoder::JsonOrNorito(name)
                | AppRoutedReadHttpDecoder::Json(name) => {
                    assert!(!name.is_empty());
                    assert_eq!(entry.decoder.typed_request_name(), Some(name));
                }
            }
        }
        assert_eq!(route_ids.len(), 46);
    }
    #[test]
    fn catalog_bounds_axum_url_parameter_topology() {
        let mut maximum = 0;
        for entry in APP_ROUTED_READ_HTTP_ENDPOINTS_V1 {
            let names: Vec<_> = entry
                .route
                .path()
                .split('/')
                .filter_map(|component| component.strip_prefix('{')?.strip_suffix('}'))
                .collect();
            maximum = maximum.max(names.len());
            assert!(names.len() <= APP_ROUTED_READ_MAX_URL_PARAMETERS_V1);
            assert!(
                names.iter().map(|name| name.len()).sum::<usize>()
                    <= APP_ROUTED_READ_URL_PARAMETER_KEY_BYTES_V1
            );
        }
        assert_eq!(maximum, APP_ROUTED_READ_MAX_URL_PARAMETERS_V1);
        assert_eq!(APP_ROUTED_READ_URL_PARAMETER_VEC_CAPACITY_V1, 4);
    }
    #[test]
    fn content_length_grammar_is_canonical_and_exact() {
        for invalid in ["", "00", "01", "+1", " 1", "1 ", "1,1", "-1"] {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(invalid).expect("test header value"),
            );
            assert!(preflight_app_routed_read_content_length(&headers, 8).is_err());
        }
        let mut exact = HeaderMap::new();
        exact.insert(header::CONTENT_LENGTH, HeaderValue::from_static("8"));
        assert_eq!(
            preflight_app_routed_read_content_length(&exact, 8).expect("exact length"),
            Some(8)
        );
        assert!(preflight_app_routed_read_content_length(&exact, 7).is_err());
        let mut duplicate = HeaderMap::new();
        duplicate.append(header::CONTENT_LENGTH, HeaderValue::from_static("1"));
        duplicate.append(header::CONTENT_LENGTH, HeaderValue::from_static("1"));
        assert!(preflight_app_routed_read_content_length(&duplicate, 8).is_err());
        let mut ambiguous = exact;
        ambiguous.insert(
            header::TRANSFER_ENCODING,
            HeaderValue::from_static("chunked"),
        );
        assert!(preflight_app_routed_read_content_length(&ambiguous, 8).is_err());
        assert_eq!(
            preflight_app_routed_read_content_length(&HeaderMap::new(), 8)
                .expect("missing length is admitted as unknown"),
            None
        );
    }
    #[test]
    fn bodyless_routes_allow_only_missing_or_canonical_zero_framing() {
        assert_eq!(
            preflight_bodyless_app_routed_read(&HeaderMap::new()).expect("missing framing"),
            None
        );
        let mut zero = HeaderMap::new();
        zero.insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
        assert_eq!(
            preflight_bodyless_app_routed_read(&zero).expect("canonical zero length"),
            Some(0)
        );
        zero.insert(header::CONTENT_LENGTH, HeaderValue::from_static("1"));
        assert!(preflight_bodyless_app_routed_read(&zero).is_err());
        let mut chunked = HeaderMap::new();
        chunked.insert(
            header::TRANSFER_ENCODING,
            HeaderValue::from_static("chunked"),
        );
        assert!(preflight_bodyless_app_routed_read(&chunked).is_err());
    }
    #[tokio::test]
    async fn missing_content_length_keeps_exact_owner_and_body_extraction_is_zero_copy() {
        let admitted = collect_app_routed_read_body(Body::from("abc"), 8, None)
            .await
            .expect("small unknown-length body");
        assert_eq!(admitted.bytes.as_ref(), b"abc");
        assert_eq!(admitted.destination_bytes, 8);
        let pointer = admitted.bytes.as_ptr();
        let mut request = Request::new(Body::from(admitted.bytes.clone()));
        request.extensions_mut().insert(admitted.clone());
        assert!(
            admitted_app_routed_read_body(&request).is_none(),
            "a typed extension alone is not trusted without task-local admission provenance"
        );
        let app = mk_app_state_for_tests();
        let reservation = try_acquire_new_query_fanout_memory(&app).expect("test reservation");
        let admission = AppRoutedReadHttpAdmission {
            reservation: reservation.clone(),
            decode_plan: torii_routed_read_request_decode_plan(&app).expect("test request plan"),
        };
        let extension = APP_ROUTED_READ_HTTP_ADMISSION
            .scope(admission, async {
                admitted_app_routed_read_body(&request).expect("stored admitted body")
            })
            .await;
        assert_eq!(extension.as_ptr(), pointer);
        let extracted = axum::body::to_bytes(request.into_body(), 8)
            .await
            .expect("one Full<Bytes> frame collects");
        assert_eq!(extracted.as_ptr(), pointer);
    }
    #[tokio::test]
    async fn zero_body_is_drained_before_returning_canonical_empty_bytes() {
        let polls = Arc::new(AtomicUsize::new(0));
        let body_polls = Arc::clone(&polls);
        let body = Body::from_stream(futures::stream::poll_fn(move |_| {
            let poll = body_polls.fetch_add(1, Ordering::SeqCst);
            Poll::Ready(match poll {
                0 => Some(Ok::<Bytes, Infallible>(Bytes::new())),
                _ => None,
            })
        }));
        let admitted = collect_app_routed_read_body(body, 8, None)
            .await
            .expect("zero-byte body should drain successfully");
        assert_eq!(polls.load(Ordering::SeqCst), 2);
        assert_eq!(admitted.destination_bytes, 8);
        assert_eq!(admitted.bytes, Bytes::new());
        assert_eq!(admitted.bytes.as_ptr(), Bytes::new().as_ptr());
    }
    #[tokio::test]
    async fn unknown_and_lying_lengths_reject_exact_plus_one() {
        let unknown = collect_app_routed_read_body(Body::from("123456789"), 8, None)
            .await
            .expect_err("unknown body over cap must fail");
        assert_eq!(unknown.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let short = collect_app_routed_read_body(Body::from("123"), 8, Some(4))
            .await
            .expect_err("short declared body must fail");
        assert_eq!(short.status(), StatusCode::BAD_REQUEST);
        let long = collect_app_routed_read_body(Body::from("12345"), 8, Some(4))
            .await
            .expect_err("long declared body must fail");
        assert_eq!(long.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
    #[tokio::test]
    async fn bodyless_routes_probe_missing_and_zero_length_bodies() {
        let mut app = mk_app_state_for_tests();
        Arc::get_mut(&mut app)
            .expect("unique test state")
            .app_api_routed_read_body_read_timeout = std::time::Duration::from_millis(1);
        let descriptor = route_catalog::pipeline::TRANSACTION_STATUS;
        let before = app.query_fanout_inflight.available_permits();
        for declared_zero in [false, true] {
            let mut request = Request::builder()
                .uri(descriptor.path())
                .body(Body::from("x"))
                .expect("bodyless request with data");
            if declared_zero {
                request
                    .headers_mut()
                    .insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
            }
            let response = admission_router(Arc::clone(&app), descriptor, false)
                .oneshot(request)
                .await
                .expect("bodyless data rejection");
            assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
            drop(response);
            assert_eq!(app.query_fanout_inflight.available_permits(), before);
        }
        let polls = Arc::new(AtomicUsize::new(0));
        let response = admission_router(Arc::clone(&app), descriptor, false)
            .oneshot(
                Request::builder()
                    .uri(descriptor.path())
                    .body(pending_body(&polls))
                    .expect("stalled bodyless request"),
            )
            .await
            .expect("bodyless deadline response");
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        assert!(polls.load(Ordering::SeqCst) > 0);
        assert!(app.query_fanout_inflight.available_permits() < before);
        drop(response);
        assert_eq!(app.query_fanout_inflight.available_permits(), before);
    }
    #[tokio::test]
    async fn bodyless_zero_length_success_holds_admission_until_response_drop() {
        let app = mk_app_state_for_tests();
        let descriptor = route_catalog::pipeline::TRANSACTION_STATUS;
        let before = app.query_fanout_inflight.available_permits();
        let request = Request::builder()
            .uri(descriptor.path())
            .header(header::CONTENT_LENGTH, "0")
            .body(Body::empty())
            .expect("empty bodyless request");
        let response = admission_router(Arc::clone(&app), descriptor, false)
            .oneshot(request)
            .await
            .expect("bodyless success response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(app.query_fanout_inflight.available_permits() < before);
        drop(response);
        assert_eq!(app.query_fanout_inflight.available_permits(), before);
    }
    #[tokio::test]
    async fn one_byte_and_empty_frames_are_bounded_only_by_admitted_bytes() {
        const BYTES: usize = 5_000;
        let frames = futures::stream::iter((0..BYTES * 2).map(|index| {
            let data = if index % 2 == 0 {
                Bytes::from_static(b"x")
            } else {
                Bytes::new()
            };
            Ok::<_, Infallible>(hyper::body::Frame::data(data))
        }));
        let body = Body::new(http_body_util::StreamBody::new(frames));
        let admitted = collect_app_routed_read_body(body, BYTES, Some(BYTES))
            .await
            .expect("more than 4096 one-byte frames remain compatible");
        assert_eq!(admitted.bytes.len(), BYTES);
    }
    #[tokio::test]
    async fn malformed_and_duplicate_media_reject_without_poll_or_permit() {
        let app = mk_app_state_for_tests();
        let before = app.query_fanout_inflight.available_permits();
        let descriptor = route_catalog::application_api::ACCOUNTS_QUERY_POST;
        for duplicate in [false, true] {
            let polls = Arc::new(AtomicUsize::new(0));
            let mut request = Request::builder()
                .method("POST")
                .uri(descriptor.path())
                .body(pending_body(&polls))
                .expect("pending request");
            request
                .headers_mut()
                .append(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
            if duplicate {
                request.headers_mut().append(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
            }
            let response = admission_router(Arc::clone(&app), descriptor, false)
                .oneshot(request)
                .await
                .expect("media rejection response");
            assert!(matches!(
                response.status(),
                StatusCode::BAD_REQUEST | StatusCode::UNSUPPORTED_MEDIA_TYPE
            ));
            assert_eq!(polls.load(Ordering::SeqCst), 0);
            assert_eq!(app.query_fanout_inflight.available_permits(), before);
        }
    }
    #[tokio::test]
    async fn listener_authentication_precedes_admission_and_body_reaches_deadline() {
        let mut app = mk_app_state_for_tests();
        Arc::get_mut(&mut app)
            .expect("unique test state")
            .app_api_routed_read_body_read_timeout = std::time::Duration::from_millis(1);
        let descriptor = route_catalog::application_api::ACCOUNTS_QUERY_POST;
        let before = app.query_fanout_inflight.available_permits();
        let unauthenticated_polls = Arc::new(AtomicUsize::new(0));
        let response = admission_router(Arc::clone(&app), descriptor, true)
            .oneshot(json_request(
                descriptor.path(),
                pending_body(&unauthenticated_polls),
            ))
            .await
            .expect("auth rejection");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(unauthenticated_polls.load(Ordering::SeqCst), 0);
        assert_eq!(app.query_fanout_inflight.available_permits(), before);
        let authenticated_polls = Arc::new(AtomicUsize::new(0));
        let mut request = json_request(descriptor.path(), pending_body(&authenticated_polls));
        request
            .headers_mut()
            .insert("x-test-auth", HeaderValue::from_static("yes"));
        let response = admission_router(Arc::clone(&app), descriptor, true)
            .oneshot(request)
            .await
            .expect("deadline response");
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        assert!(authenticated_polls.load(Ordering::SeqCst) > 0);
        assert!(app.query_fanout_inflight.available_permits() < before);
        drop(response);
        assert_eq!(app.query_fanout_inflight.available_permits(), before);
    }
    #[tokio::test]
    async fn busy_admission_fails_before_body_poll_and_task_local_clone_does_not_reacquire() {
        let app = mk_app_state_for_tests();
        let reservation = try_acquire_new_query_fanout_memory(&app)
            .expect("fixture occupies complete fanout working set");
        let occupied = app.query_fanout_inflight.available_permits();
        let polls = Arc::new(AtomicUsize::new(0));
        let descriptor = route_catalog::application_api::ACCOUNTS_QUERY_POST;
        let response = admission_router(Arc::clone(&app), descriptor, false)
            .oneshot(json_request(descriptor.path(), pending_body(&polls)))
            .await
            .expect("busy response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert_eq!(app.query_fanout_inflight.available_permits(), occupied);
        let plan = torii_routed_read_request_decode_plan(&app).expect("test request plan");
        let admission = AppRoutedReadHttpAdmission {
            reservation: reservation.clone(),
            decode_plan: plan,
        };
        APP_ROUTED_READ_HTTP_ADMISSION
            .scope(admission, async {
                let cloned = try_acquire_query_fanout_memory(&app)
                    .expect("downstream borrows outer reservation");
                assert_eq!(app.query_fanout_inflight.available_permits(), occupied);
                drop(cloned);
            })
            .await;
        drop(reservation);
    }
    #[test]
    fn dynamic_raw_target_exact_and_plus_one_precede_permit_acquisition() {
        let app = mk_app_state_for_tests();
        let before = app.query_fanout_inflight.available_permits();
        let mut plan = torii_routed_read_request_decode_plan(&app).expect("test request plan");
        // `http::Uri` itself has a u16-sized textual ceiling. A small synthetic
        // admission cap exercises exact/+1 accounting without hitting that
        // independent parser boundary first.
        plan.raw_input_limit_bytes = 4_096;
        let exact = format!("/{}", "x".repeat(plan.raw_input_limit_bytes - 1));
        let exact_uri: axum::http::Uri = exact.parse().expect("exact URI");
        assert_eq!(
            app_routed_read_raw_target_bytes(&exact_uri),
            plan.raw_input_limit_bytes
        );
        plan.admit_raw_input(app_routed_read_raw_target_bytes(&exact_uri))
            .expect("exact raw target fits");
        let over_uri: axum::http::Uri = format!("{exact}x").parse().expect("over-limit URI");
        assert!(
            plan.admit_raw_input(app_routed_read_raw_target_bytes(&over_uri))
                .is_err()
        );
        let absolute_prefix = "http://torii.example/";
        let suffix_bytes = plan
            .raw_input_limit_bytes
            .checked_sub(absolute_prefix.len())
            .expect("request limit exceeds absolute-form prefix");
        let absolute = format!("{absolute_prefix}{}", "x".repeat(suffix_bytes));
        let absolute_uri: axum::http::Uri = absolute.parse().expect("absolute-form URI");
        assert_eq!(
            app_routed_read_raw_target_bytes(&absolute_uri),
            absolute.len()
        );
        plan.admit_raw_input(app_routed_read_raw_target_bytes(&absolute_uri))
            .expect("exact absolute-form target fits");
        let absolute_over_uri: axum::http::Uri = format!("{absolute}x")
            .parse()
            .expect("over-limit absolute-form URI");
        assert!(
            plan.admit_raw_input(app_routed_read_raw_target_bytes(&absolute_over_uri))
                .is_err()
        );
        let asterisk_uri: axum::http::Uri = "*".parse().expect("asterisk-form URI");
        assert_eq!(app_routed_read_raw_target_bytes(&asterisk_uri), 1);
        let authority_uri: axum::http::Uri =
            "torii.example:8080".parse().expect("authority-form URI");
        assert_eq!(
            app_routed_read_raw_target_bytes(&authority_uri),
            "torii.example:8080".len()
        );
        assert_eq!(app.query_fanout_inflight.available_permits(), before);
        let source = include_str!("../../torii_app_routed_read_http.rs");
        let target = source
            .find("let target_bytes = app_routed_read_raw_target_bytes")
            .expect("target preflight source");
        let permit = source
            .find("let reservation = match try_acquire_new_query_fanout_memory")
            .expect("permit source");
        assert!(target < permit);
    }
}
