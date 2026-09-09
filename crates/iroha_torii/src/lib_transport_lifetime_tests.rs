// Crate-root transport, connection lifetime and emergency-surface test modules.

#[cfg(test)]
mod mcp_dispatch_router_lifetime_tests {
    use super::McpDispatchRouterSlot;

    #[test]
    fn slot_does_not_keep_dispatch_router_alive() {
        let slot = McpDispatchRouterSlot::default();
        assert!(slot.load().is_none());

        let owner = slot.install(axum::Router::new());
        assert!(slot.load().is_some());

        drop(owner);
        assert!(slot.load().is_none());
    }
}

#[cfg(all(test, feature = "app_api"))]
mod preauth_connection_lifetime_tests {
    use super::*;
    use axum::{
        Router,
        body::{Body, Bytes},
        extract::Extension,
        http::{Request, StatusCode, header},
        response::Response,
        routing::{get, post},
    };
    use futures::StreamExt as _;
    use http_body_util::BodyExt as _;
    use iroha_crypto::Algorithm;
    use std::{collections::HashSet, convert::Infallible, sync::Arc};
    use tokio::sync::{Semaphore, mpsc};
    use tower::ServiceExt as _;
    fn app_with_scheme_cap(scheme: &str) -> SharedAppState {
        let mut app = crate::mk_app_state_for_tests();
        Arc::get_mut(&mut app)
            .expect("test app state must be uniquely owned")
            .preauth_gate = Arc::new(limits::PreAuthGate::new(limits::PreAuthConfig {
            max_total: None,
            max_per_ip: None,
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: None,
            ban_capacity: NonZeroUsize::new(4_096).expect("test ban capacity is non-zero"),
            allow_nets: Vec::new(),
            scheme_limits: vec![limits::SchemeLimit {
                name: scheme.to_owned(),
                max_connections: 1,
            }],
        }));
        app
    }
    fn app_with_per_ip_cap() -> SharedAppState {
        let mut app = crate::mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.trusted_proxy_nets = Arc::new(limits::parse_cidrs(&["127.0.0.0/8".to_owned()]));
        state.preauth_gate = Arc::new(limits::PreAuthGate::new(limits::PreAuthConfig {
            max_total: None,
            max_per_ip: Some(1),
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: None,
            ban_capacity: NonZeroUsize::new(4_096).expect("test ban capacity is non-zero"),
            allow_nets: Vec::new(),
            scheme_limits: Vec::new(),
        }));
        app
    }
    fn per_ip_preauth_router(app: SharedAppState) -> Router {
        Router::new()
            .route("/hold", get(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_preauth,
            ))
            .layer(axum::middleware::from_fn_with_state(
                app,
                inject_remote_addr_header,
            ))
    }
    fn request_with_remote(transport_ip: &str, forwarded_ip: &str) -> Request<Body> {
        use axum::extract::ConnectInfo;
        let mut request = Request::builder()
            .uri("/hold")
            .header(limits::FORWARDED_FOR_HEADER, forwarded_ip)
            .body(Body::empty())
            .expect("request");
        request
            .extensions_mut()
            .insert(ConnectInfo(std::net::SocketAddr::new(
                transport_ip.parse().expect("transport IP"),
                12_345,
            )));
        request
    }
    #[tokio::test]
    async fn preauth_uses_distinct_client_buckets_behind_trusted_proxy() {
        let router = per_ip_preauth_router(app_with_per_ip_cap());
        let first = router
            .clone()
            .oneshot(request_with_remote("127.0.0.1", "203.0.113.10"))
            .await
            .expect("first proxied response");
        assert_eq!(first.status(), StatusCode::OK);
        let second = router
            .clone()
            .oneshot(request_with_remote("127.0.0.1", "203.0.113.11"))
            .await
            .expect("second proxied response");
        assert_eq!(
            second.status(),
            StatusCode::OK,
            "distinct trusted-proxy clients must not share the loopback bucket"
        );
        let same_client = router
            .oneshot(request_with_remote("127.0.0.1", "203.0.113.10"))
            .await
            .expect("same-client response");
        assert_eq!(same_client.status(), StatusCode::TOO_MANY_REQUESTS);
        drop((first, second, same_client));
    }
    #[tokio::test]
    async fn preauth_rejects_untrusted_forwarded_ip_spoofing() {
        let router = per_ip_preauth_router(app_with_per_ip_cap());
        let first = router
            .clone()
            .oneshot(request_with_remote("198.51.100.10", "203.0.113.10"))
            .await
            .expect("first untrusted response");
        assert_eq!(first.status(), StatusCode::OK);
        let spoofed = router
            .oneshot(request_with_remote("198.51.100.10", "203.0.113.11"))
            .await
            .expect("spoofed response");
        assert_eq!(
            spoofed.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "an untrusted peer must not evade its bucket by changing the forwarded IP"
        );
        drop((first, spoofed));
    }
    const TEST_ONBOARDING_TOKEN: &str = "torii-hbl-onboarding-test-token-32-bytes";
    const TEST_SECOND_HBL_ONBOARDING_TOKEN: &str = "torii-second-hbl-onboarding-token-32-bytes";
    const TEST_UBL_ONBOARDING_TOKEN: &str = "torii-ubl-onboarding-test-token-32-bytes";
    fn app_with_onboarding_auth(require_global_token: bool) -> SharedAppState {
        let mut app = crate::mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.require_api_token = require_global_token;
        state.api_token_digests = if require_global_token {
            Arc::new(limits::ApiTokenDigestSet::from_tokens([
                "valid-global-token",
            ]))
        } else {
            Arc::new(limits::ApiTokenDigestSet::default())
        };
        let key_pair = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("deterministic onboarding test key");
        state.account_onboarding = Some(AccountOnboardingSigner {
            authority: AccountId::new(key_pair.public_key().clone()),
            private_key: ExposedPrivateKey(key_pair.private_key().clone()),
            api_token_hashes_by_domain: BTreeMap::from([
                (
                    DomainId::try_new("hbl", "sbp").expect("HBL domain"),
                    vec![
                        *blake3::hash(TEST_ONBOARDING_TOKEN.as_bytes()).as_bytes(),
                        *blake3::hash(TEST_SECOND_HBL_ONBOARDING_TOKEN.as_bytes()).as_bytes(),
                    ],
                ),
                (
                    DomainId::try_new("ubl", "sbp").expect("UBL domain"),
                    vec![*blake3::hash(TEST_UBL_ONBOARDING_TOKEN.as_bytes()).as_bytes()],
                ),
            ]),
            api_token_hashes_by_dataspace: BTreeMap::new(),
            allowed_permissions: BTreeSet::new(),
            fee_sponsor_program_id: None,
            alias_lease_term_years: 1,
            owner_auto_renew: None,
        });
        app
    }
    #[derive(Clone, Debug, crate::json_macros::JsonDeserialize)]
    struct OnboardingAuthBoundaryBody {
        value: u32,
    }
    async fn onboarding_auth_boundary_handler(
        axum::extract::Extension(_authenticated_domain): axum::extract::Extension<
            AuthenticatedOnboardingDomain,
        >,
        headers: HeaderMap,
        JsonOnly(body): JsonOnly<OnboardingAuthBoundaryBody>,
    ) -> StatusCode {
        if headers.contains_key(HEADER_ONBOARDING_API_TOKEN) || body.value != 7 {
            StatusCode::INTERNAL_SERVER_ERROR
        } else {
            StatusCode::OK
        }
    }
    fn onboarding_auth_boundary_router(app: SharedAppState) -> Router {
        Router::new()
            .route(
                route_catalog::application_api::ACCOUNTS_ONBOARD_PLAN_POST.path(),
                post(onboarding_auth_boundary_handler),
            )
            .route(
                route_catalog::application_api::ACCOUNTS_ONBOARD_PREPARE_POST.path(),
                post(onboarding_auth_boundary_handler),
            )
            .route(
                route_catalog::application_api::ACCOUNTS_ONBOARD_POST.path(),
                post(onboarding_auth_boundary_handler),
            )
            .layer(axum::middleware::from_fn(capture_response_format))
            .layer(axum::middleware::from_fn(coalesce_accept_headers))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_onboarding_api_token,
            ))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_api_token,
            ))
            .layer(axum::middleware::from_fn_with_state(app, enforce_preauth))
            .layer(axum::middleware::from_fn(enforce_typed_error_contract))
    }
    fn onboarding_auth_boundary_request(
        descriptor: iroha_torii_shared::route_catalog::RouteDescriptor,
        global_tokens: &[&str],
        onboarding_tokens: &[&str],
        accept: &str,
        content_types: &[&str],
        body: &'static str,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(axum::http::Method::POST)
            .uri(descriptor.path())
            .header(header::ACCEPT, accept);
        for value in global_tokens {
            builder = builder.header(HEADER_API_TOKEN, *value);
        }
        for value in onboarding_tokens {
            builder = builder.header(HEADER_ONBOARDING_API_TOKEN, *value);
        }
        for value in content_types {
            builder = builder.header(header::CONTENT_TYPE, *value);
        }
        let mut request = builder.body(Body::from(body)).expect("onboarding request");
        request
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(descriptor));
        request
    }
    async fn typed_error_code(response: Response) -> String {
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect typed error response")
            .to_bytes();
        let envelope: ErrorEnvelope =
            norito::json::from_slice(&body).expect("decode typed error response");
        envelope.code().to_owned()
    }
    #[tokio::test]
    async fn onboarding_authentication_precedes_media_and_body() {
        let router = onboarding_auth_boundary_router(app_with_onboarding_auth(true));
        for descriptor in [
            route_catalog::application_api::ACCOUNTS_ONBOARD_PLAN_POST,
            route_catalog::application_api::ACCOUNTS_ONBOARD_PREPARE_POST,
            route_catalog::application_api::ACCOUNTS_ONBOARD_POST,
        ] {
            let missing_global = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &[],
                    &[],
                    "application/json;q=2",
                    &["application/json; charset==utf-8"],
                    "not-json",
                ))
                .await
                .expect("missing global token response");
            assert_eq!(missing_global.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(
                missing_global.headers().get(header::WWW_AUTHENTICATE),
                Some(&HeaderValue::from_static("IrohaApiToken realm=\"torii\""))
            );
            assert_eq!(typed_error_code(missing_global).await, "api_token_required");
            let missing_onboarding = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &["valid-global-token"],
                    &[],
                    "application/json;q=2",
                    &["application/json; charset==utf-8"],
                    "not-json",
                ))
                .await
                .expect("missing onboarding token response");
            assert_eq!(missing_onboarding.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(
                missing_onboarding.headers().get(header::WWW_AUTHENTICATE),
                Some(&HeaderValue::from_static("IrohaOnboardingToken"))
            );
            assert_eq!(
                typed_error_code(missing_onboarding).await,
                "onboarding_auth_required"
            );
            let unknown_onboarding = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &["valid-global-token"],
                    &["unknown-onboarding-token-that-is-long-enough"],
                    "application/json",
                    &["application/json"],
                    r#"{"value":7}"#,
                ))
                .await
                .expect("unknown onboarding token response");
            assert_eq!(unknown_onboarding.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(
                typed_error_code(unknown_onboarding).await,
                "onboarding_auth_required"
            );
            let duplicate_onboarding = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &["valid-global-token"],
                    &[TEST_ONBOARDING_TOKEN, TEST_ONBOARDING_TOKEN],
                    "application/json;q=2",
                    &["application/json; charset==utf-8"],
                    "not-json",
                ))
                .await
                .expect("duplicate onboarding token response");
            assert_eq!(duplicate_onboarding.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(
                typed_error_code(duplicate_onboarding).await,
                "onboarding_auth_required"
            );
            let invalid_accept = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &["valid-global-token"],
                    &[TEST_ONBOARDING_TOKEN],
                    "application/json;q=2",
                    &["application/json; charset==utf-8"],
                    "not-json",
                ))
                .await
                .expect("invalid Accept response");
            assert_eq!(invalid_accept.status(), StatusCode::NOT_ACCEPTABLE);
            assert_eq!(
                typed_error_code(invalid_accept).await,
                "response_not_acceptable"
            );
            let norito_media = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &["valid-global-token"],
                    &[TEST_ONBOARDING_TOKEN],
                    "application/json",
                    &["application/x-norito"],
                    "not-json",
                ))
                .await
                .expect("Norito media response");
            assert_eq!(norito_media.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
            assert_eq!(
                typed_error_code(norito_media).await,
                "request_content_type_unsupported"
            );
            let duplicate_content_type = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &["valid-global-token"],
                    &[TEST_ONBOARDING_TOKEN],
                    "application/json",
                    &["application/json", "application/json"],
                    "not-json",
                ))
                .await
                .expect("duplicate Content-Type response");
            assert_eq!(duplicate_content_type.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                typed_error_code(duplicate_content_type).await,
                "request_content_type_invalid"
            );
            let accepted = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &["valid-global-token"],
                    &[TEST_ONBOARDING_TOKEN],
                    "application/json",
                    &["application/json"],
                    r#"{"value":7}"#,
                ))
                .await
                .expect("accepted onboarding boundary response");
            assert_eq!(accepted.status(), StatusCode::OK);
            let accepted_ubl = router
                .clone()
                .oneshot(onboarding_auth_boundary_request(
                    descriptor,
                    &["valid-global-token"],
                    &[TEST_UBL_ONBOARDING_TOKEN],
                    "application/json",
                    &["application/json"],
                    r#"{"value":7}"#,
                ))
                .await
                .expect("accepted UBL onboarding boundary response");
            assert_eq!(accepted_ubl.status(), StatusCode::OK);
        }
    }
    #[test]
    fn onboarding_alias_token_maps_each_credential_to_its_exact_domain() {
        let app = app_with_onboarding_auth(false);
        for (token, expected_domain) in [
            (
                TEST_ONBOARDING_TOKEN,
                DomainId::try_new("hbl", "sbp").expect("HBL domain"),
            ),
            (
                TEST_SECOND_HBL_ONBOARDING_TOKEN,
                DomainId::try_new("hbl", "sbp").expect("HBL domain"),
            ),
            (
                TEST_UBL_ONBOARDING_TOKEN,
                DomainId::try_new("ubl", "sbp").expect("UBL domain"),
            ),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                HEADER_ONBOARDING_API_TOKEN,
                HeaderValue::from_str(token).expect("test onboarding token header"),
            );
            let authenticated = authenticate_onboarding_api_token(&app, &headers)
                .expect("configured onboarding credential must authenticate");
            assert_eq!(
                authenticated,
                AuthenticatedOnboardingScope::Domain(expected_domain)
            );
        }
    }
    fn request(path: &str, websocket: bool) -> Request<Body> {
        let mut builder = Request::builder().uri(path);
        if websocket {
            builder = builder
                .header(header::CONNECTION, "keep-alive, Upgrade")
                .header(header::UPGRADE, "websocket")
                .header(header::SEC_WEBSOCKET_VERSION, "13")
                .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==");
        }
        let mut request = builder.body(Body::empty()).expect("request");
        if websocket {
            request
                .extensions_mut()
                .insert(MatchedRouteMetadata::from_descriptor(
                    route_catalog::streaming::SUBSCRIPTION_WS,
                ));
        }
        request
    }
    #[test]
    fn connection_scheme_uses_only_the_matched_route_contract() {
        let mut ordinary = Request::builder()
            .method(axum::http::Method::POST)
            .uri(route_catalog::application_api::ACCOUNTS_ONBOARD_PLAN_POST.path())
            .header(header::CONTENT_TYPE, crate::utils::NORITO_MIME_TYPE)
            .body(Body::empty())
            .expect("ordinary request");
        ordinary
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(
                route_catalog::application_api::ACCOUNTS_ONBOARD_PLAN_POST,
            ));
        assert_eq!(ConnScheme::from_request(&ordinary), ConnScheme::Http);

        let mut transaction = Request::builder()
            .method(axum::http::Method::POST)
            .uri(route_catalog::pipeline::TRANSACTION.path())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::empty())
            .expect("transaction request");
        transaction
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(
                route_catalog::pipeline::TRANSACTION,
            ));
        assert_eq!(
            ConnScheme::from_request(&transaction),
            ConnScheme::NoritoRpc
        );

        let mut malformed_websocket = request("/ws", false);
        malformed_websocket
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(
                route_catalog::streaming::SUBSCRIPTION_WS,
            ));
        assert_eq!(
            ConnScheme::from_request(&malformed_websocket),
            ConnScheme::Ws,
            "malformed upgrades must still consume WebSocket capacity"
        );
    }
    #[tokio::test]
    async fn preauth_rejects_malformed_websocket_before_the_handler() {
        let app = app_with_scheme_cap("ws");
        let router = Router::new()
            .route("/ws", get(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_preauth,
            ));
        let mut malformed = request("/ws", false);
        malformed
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(
                route_catalog::streaming::SUBSCRIPTION_WS,
            ));
        let response = router
            .clone()
            .oneshot(malformed)
            .await
            .expect("malformed handshake response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            typed_error_code(response).await,
            "websocket_handshake_invalid"
        );

        let valid = router
            .oneshot(request("/ws", true))
            .await
            .expect("valid handshake response");
        assert_eq!(valid.status(), StatusCode::OK);
    }
    #[tokio::test]
    async fn partially_consumed_sse_body_holds_http_capacity_until_drop() {
        let app = app_with_scheme_cap("http");
        let router = Router::new()
            .route(
                "/stream",
                get(|| async {
                    let stream = futures::stream::once(async {
                        Ok::<Bytes, Infallible>(Bytes::from_static(b": heartbeat\n\n"))
                    })
                    .chain(futures::stream::pending());
                    Response::builder()
                        .header(header::CONTENT_TYPE, "text/event-stream")
                        .body(Body::from_stream(stream))
                        .expect("SSE response")
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_preauth,
            ));
        let first = router
            .clone()
            .oneshot(request("/stream", false))
            .await
            .expect("first response");
        assert_eq!(first.status(), StatusCode::OK);
        let mut first_body = first.into_body();
        let first_frame = first_body
            .frame()
            .await
            .expect("heartbeat frame")
            .expect("valid heartbeat frame");
        assert_eq!(
            first_frame.into_data().expect("data frame"),
            Bytes::from_static(b": heartbeat\n\n")
        );
        let rejected = router
            .clone()
            .oneshot(request("/stream", false))
            .await
            .expect("capacity response");
        assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
        drop(first_body);
        let admitted_again = router
            .oneshot(request("/stream", false))
            .await
            .expect("response after stream drop");
        assert_eq!(admitted_again.status(), StatusCode::OK);
    }
    #[tokio::test]
    async fn soracloud_streaming_body_holds_route_capacity_until_drop() {
        let gate = Arc::new(Semaphore::new(1));
        let permit = gate
            .clone()
            .try_acquire_owned()
            .expect("first public runtime request acquires capacity");
        let stream = futures::stream::once(async {
            Ok::<Bytes, Infallible>(Bytes::from_static(b"partial hosted response"))
        })
        .chain(futures::stream::pending());
        let response = Response::builder()
            .body(Body::from_stream(stream))
            .expect("streaming response");
        let guarded = hold_soracloud_public_permit_in_response_body(response, permit);
        let mut body = guarded.into_body();
        let frame = body
            .frame()
            .await
            .expect("first hosted frame")
            .expect("valid hosted frame");
        assert_eq!(
            frame.into_data().expect("hosted data frame"),
            Bytes::from_static(b"partial hosted response")
        );
        assert!(
            gate.clone().try_acquire_owned().is_err(),
            "a second hosted request must remain rejected while the first body is live"
        );
        drop(body);
        drop(
            gate.try_acquire_owned()
                .expect("dropping the hosted body releases route capacity"),
        );
    }
    #[tokio::test]
    async fn websocket_handoff_holds_ws_capacity_for_task_lifetime() {
        let app = app_with_scheme_cap("ws");
        let release = Arc::new(Semaphore::new(0));
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();
        let (released_tx, mut released_rx) = mpsc::unbounded_channel();
        let router = Router::new()
            .route(
                "/ws",
                get({
                    let release = Arc::clone(&release);
                    move |handoff: Option<Extension<PreAuthGuardHandoff>>| {
                        let release = Arc::clone(&release);
                        let started_tx = started_tx.clone();
                        let released_tx = released_tx.clone();
                        async move {
                            let guard = take_preauth_upgrade_guard(handoff)
                                .expect("production middleware supplies upgrade guard");
                            tokio::spawn(async move {
                                {
                                    let _guard = guard;
                                    started_tx.send(()).expect("test receiver remains alive");
                                    let permit = release.acquire().await.expect("semaphore open");
                                    permit.forget();
                                }
                                released_tx.send(()).expect("test receiver remains alive");
                            });
                            StatusCode::SWITCHING_PROTOCOLS
                        }
                    }
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_preauth,
            ));
        let first = router
            .clone()
            .oneshot(request("/ws", true))
            .await
            .expect("first upgrade response");
        assert_eq!(first.status(), StatusCode::SWITCHING_PROTOCOLS);
        started_rx.recv().await.expect("upgrade task started");
        let rejected = router
            .clone()
            .oneshot(request("/ws", true))
            .await
            .expect("capacity response");
        assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
        release.add_permits(1);
        released_rx.recv().await.expect("first guard released");
        let admitted_again = router
            .oneshot(request("/ws", true))
            .await
            .expect("upgrade after task exit");
        assert_eq!(admitted_again.status(), StatusCode::SWITCHING_PROTOCOLS);
        started_rx
            .recv()
            .await
            .expect("second upgrade task started");
        release.add_permits(1);
        released_rx.recv().await.expect("second guard released");
    }
    #[tokio::test]
    async fn aborted_upgrade_task_releases_handoff_capacity() {
        let app = app_with_scheme_cap("ws");
        let guard = app
            .acquire_preauth(None, ConnScheme::Ws)
            .await
            .expect("upgrade guard");
        let handoff = PreAuthGuardHandoff::new(guard);
        let upgrade_guard = handoff.take().expect("first handoff owner");
        assert!(handoff.take().is_none(), "handoff must be single-owner");
        let task = tokio::spawn(async move {
            let _upgrade_guard = upgrade_guard;
            futures::future::pending::<()>().await;
        });
        assert_eq!(
            app.preauth_gate
                .acquire(None, Some("ws"))
                .await
                .unwrap_err(),
            limits::RejectReason::SchemeCap
        );
        task.abort();
        assert!(
            task.await
                .expect_err("task must be cancelled")
                .is_cancelled(),
            "aborting an upgrade task must run guard destructors"
        );
        app.preauth_gate
            .acquire(None, Some("ws"))
            .await
            .expect("cancelled upgrade released capacity");
    }
    #[tokio::test]
    async fn guarded_response_preserves_data_and_trailer_frames() {
        let app = app_with_scheme_cap("http");
        let guard = app
            .acquire_preauth(None, ConnScheme::Http)
            .await
            .expect("guard");
        let mut trailers = HeaderMap::new();
        trailers.insert("x-test-trailer", HeaderValue::from_static("present"));
        let frames = futures::stream::iter([
            Ok::<_, Infallible>(hyper::body::Frame::data(Bytes::from_static(b"payload"))),
            Ok(hyper::body::Frame::trailers(trailers)),
        ]);
        let response = Response::new(Body::new(http_body_util::StreamBody::new(frames)));
        let response = hold_preauth_guard_in_response_body(response, guard);
        assert_eq!(
            app.preauth_gate
                .acquire(None, Some("http"))
                .await
                .unwrap_err(),
            limits::RejectReason::SchemeCap
        );
        let collected = response
            .into_body()
            .collect()
            .await
            .expect("guarded body remains valid");
        assert_eq!(
            collected
                .trailers()
                .and_then(|headers| headers.get("x-test-trailer"))
                .and_then(|value| value.to_str().ok()),
            Some("present")
        );
        assert_eq!(collected.to_bytes(), Bytes::from_static(b"payload"));
        app.preauth_gate
            .acquire(None, Some("http"))
            .await
            .expect("guard released at body EOF");
    }
    #[tokio::test]
    async fn preauth_capacity_is_enforced_before_api_token_authentication() {
        let mut app = app_with_scheme_cap("http");
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.require_api_token = true;
        state.api_token_digests = Arc::new(limits::ApiTokenDigestSet::from_tokens(["valid-token"]));
        let occupying_guard = app
            .acquire_preauth(None, ConnScheme::Http)
            .await
            .expect("occupying guard");
        let router = Router::new()
            .route("/protected", get(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_api_token,
            ))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_preauth,
            ));
        let capacity_response = router
            .clone()
            .oneshot(request("/protected", false))
            .await
            .expect("capacity response");
        assert_eq!(capacity_response.status(), StatusCode::TOO_MANY_REQUESTS);
        drop(occupying_guard);
        let authentication_response = router
            .oneshot(request("/protected", false))
            .await
            .expect("authentication response");
        assert_eq!(authentication_response.status(), StatusCode::UNAUTHORIZED);
    }
    #[tokio::test]
    async fn missing_required_api_token_configuration_fails_closed_across_router() {
        let mut app = app_with_scheme_cap("http");
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.require_api_token = true;
        state.api_token_digests = Arc::new(limits::ApiTokenDigestSet::default());
        let router = Router::new()
            .route("/protected", get(|| async { StatusCode::OK }))
            .route("/also-protected", post(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_api_token,
            ))
            .layer(axum::middleware::from_fn(enforce_typed_error_contract));
        for (method, path) in [
            (axum::http::Method::GET, "/protected"),
            (axum::http::Method::POST, "/also-protected"),
            (axum::http::Method::GET, "/not-mounted"),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header(header::ACCEPT, "application/json")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("authentication response");
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                response.headers().get(header::RETRY_AFTER),
                Some(&HeaderValue::from_static("1"))
            );
            let body = response
                .into_body()
                .collect()
                .await
                .expect("collect authentication response")
                .to_bytes();
            let envelope: ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode authentication response");
            assert_eq!(envelope.code(), "api_token_unavailable");
            assert_eq!(
                envelope
                    .details
                    .as_ref()
                    .and_then(|details| details.retry_after_seconds),
                Some(1)
            );
        }
    }
    #[tokio::test]
    async fn duplicate_api_token_headers_fail_closed() {
        let mut app = app_with_scheme_cap("http");
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.require_api_token = true;
        state.api_token_digests = Arc::new(limits::ApiTokenDigestSet::from_tokens(["valid-token"]));
        let router = Router::new()
            .route("/protected", get(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_api_token,
            ));
        let mut duplicate = request("/protected", false);
        duplicate
            .headers_mut()
            .append(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
        duplicate
            .headers_mut()
            .append(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
        let response = router
            .clone()
            .oneshot(duplicate)
            .await
            .expect("duplicate-token response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().contains_key(header::WWW_AUTHENTICATE));
        let mut single = request("/protected", false);
        single
            .headers_mut()
            .insert(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
        let response = router.oneshot(single).await.expect("single-token response");
        assert_eq!(response.status(), StatusCode::OK);
    }
    #[tokio::test]
    async fn public_sorafs_gateways_bypass_the_deployment_api_token() {
        let mut app = app_with_scheme_cap("http");
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.require_api_token = true;
        state.api_token_digests = Arc::new(limits::ApiTokenDigestSet::from_tokens(["valid-token"]));
        let router = Router::new()
            .fallback(|| async {
                let mut response = StatusCode::OK.into_response();
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=60"),
                );
                response
            })
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_api_token,
            ))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_required_api_token_private_no_store,
            ));
        for (descriptor, path) in [
            (route_catalog::sorafs::CID_LOOKUP, "/v1/sorafs/cid/example"),
            (
                route_catalog::sorafs::SITE_MANIFEST,
                "/.well-known/sorafs/manifest",
            ),
            (route_catalog::sorafs::CID_ROOT, "/sorafs/cid/example"),
            (
                route_catalog::sorafs::CID_PATH,
                "/sorafs/cid/example/index.html",
            ),
        ] {
            let mut request = Request::builder()
                .uri(path)
                .header(HEADER_API_TOKEN, "invalid-token")
                .body(Body::empty())
                .expect("public gateway request");
            request
                .extensions_mut()
                .insert(MatchedRouteMetadata::from_descriptor(descriptor));
            assert!(is_public_sorafs_gateway_route(&request));
            let response = router
                .clone()
                .oneshot(request)
                .await
                .expect("public gateway response");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL),
                Some(&HeaderValue::from_static("public, max-age=60"))
            );
        }
        let mut unrelated = Request::builder()
            .uri(route_catalog::core::HEALTH.path())
            .body(Body::empty())
            .expect("unrelated public request");
        unrelated
            .extensions_mut()
            .insert(MatchedRouteMetadata::from_descriptor(
                route_catalog::core::HEALTH,
            ));
        assert!(!is_public_sorafs_gateway_route(&unrelated));
        let response = router
            .oneshot(unrelated)
            .await
            .expect("unrelated listener-token response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    #[tokio::test]
    async fn required_api_token_forces_private_no_store_on_every_response() {
        let mut app = app_with_scheme_cap("http");
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.require_api_token = true;
        state.api_token_digests = Arc::new(limits::ApiTokenDigestSet::from_tokens(["valid-token"]));
        let router = Router::new()
            .route(
                "/protected",
                get(|| async {
                    let mut response = StatusCode::OK.into_response();
                    response.headers_mut().insert(
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("public, max-age=86400"),
                    );
                    response
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_api_token,
            ));

        let rejection = router
            .clone()
            .oneshot(request("/protected", false))
            .await
            .expect("missing-token response");
        assert_eq!(rejection.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            rejection.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("private, no-store"))
        );

        let mut authenticated = request("/protected", false);
        authenticated
            .headers_mut()
            .insert(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
        let response = router
            .oneshot(authenticated)
            .await
            .expect("authenticated response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("private, no-store"))
        );
    }
    #[tokio::test]
    async fn required_api_token_outer_cache_boundary_covers_early_responses() {
        let mut app = app_with_scheme_cap("http");
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.require_api_token = true;
        let protected = Router::new()
            .fallback(|| async {
                let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=86400"),
                );
                response
            })
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_required_api_token_private_no_store,
            ));
        let response = protected
            .oneshot(request("/early-rejection", false))
            .await
            .expect("early response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("private, no-store"))
        );

        let public = Router::new()
            .route(
                "/public",
                get(|| async {
                    let mut response = StatusCode::OK.into_response();
                    response.headers_mut().insert(
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("public, max-age=60"),
                    );
                    response
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                app_with_scheme_cap("http"),
                enforce_required_api_token_private_no_store,
            ));
        let response = public
            .oneshot(request("/public", false))
            .await
            .expect("public response");
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("public, max-age=60"))
        );
    }
    #[tokio::test]
    async fn kagemusha_command_authentication_precedes_media_and_idempotency_validation() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        async fn error_code(response: Response) -> String {
            let body = response
                .into_body()
                .collect()
                .await
                .expect("collect typed error response")
                .to_bytes();
            let envelope: ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode typed error response");
            envelope.code().to_owned()
        }
        let mut app = app_with_scheme_cap("http");
        let state = Arc::get_mut(&mut app).expect("test app state must be uniquely owned");
        state.require_api_token = true;
        state.api_token_digests = Arc::new(limits::ApiTokenDigestSet::from_tokens(["valid-token"]));
        let handler_calls = Arc::new(AtomicUsize::new(0));
        let router = Router::new()
            .route(
                route_catalog::kagemusha::TOP_UP.path(),
                axum::routing::post({
                    let handler_calls = Arc::clone(&handler_calls);
                    move || {
                        let handler_calls = Arc::clone(&handler_calls);
                        async move {
                            handler_calls.fetch_add(1, Ordering::SeqCst);
                            StatusCode::OK
                        }
                    }
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_kagemusha_command_prebody_admission,
            ))
            .layer(axum::middleware::from_fn(capture_response_format))
            .layer(axum::middleware::from_fn(coalesce_accept_headers))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_api_token,
            ))
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_preauth,
            ))
            .layer(axum::middleware::from_fn(enforce_typed_error_contract));
        let kagemusha_request =
            |token_count: usize, content_type: &'static str, accept: &'static str| {
                let mut request = Request::builder()
                    .method(axum::http::Method::POST)
                    .uri(route_catalog::kagemusha::TOP_UP.path())
                    .header(header::ACCEPT, accept)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from("{malformed-json"))
                    .expect("KAGEMUSHA command request");
                request
                    .extensions_mut()
                    .insert(MatchedRouteMetadata::from_descriptor(
                        route_catalog::kagemusha::TOP_UP,
                    ));
                for _ in 0..token_count {
                    request
                        .headers_mut()
                        .append(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
                }
                request
            };
        let occupying_guard = app
            .acquire_preauth(None, ConnScheme::Http)
            .await
            .expect("occupy the HTTP pre-auth slot");
        let at_capacity = router
            .clone()
            .oneshot(kagemusha_request(
                0,
                "application/json; charset==utf-8",
                "application/json;q=2",
            ))
            .await
            .expect("pre-auth capacity response");
        assert_eq!(at_capacity.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error_code(at_capacity).await, "preauth_scheme_capacity");
        drop(occupying_guard);
        let missing = router
            .clone()
            .oneshot(kagemusha_request(
                0,
                "application/json; charset==utf-8",
                "application/json;q=2",
            ))
            .await
            .expect("missing-token response");
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
        assert!(missing.headers().contains_key(header::WWW_AUTHENTICATE));
        assert_eq!(error_code(missing).await, "api_token_required");
        let mut missing_with_duplicate_content_type =
            kagemusha_request(0, "application/json", "application/json");
        missing_with_duplicate_content_type.headers_mut().append(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-norito"),
        );
        let missing = router
            .clone()
            .oneshot(missing_with_duplicate_content_type)
            .await
            .expect("missing-token duplicate-Content-Type response");
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(error_code(missing).await, "api_token_required");
        let duplicate = router
            .clone()
            .oneshot(kagemusha_request(
                2,
                "application/json; charset==utf-8",
                "application/json;q=2",
            ))
            .await
            .expect("duplicate-token response");
        assert_eq!(duplicate.status(), StatusCode::UNAUTHORIZED);
        assert!(duplicate.headers().contains_key(header::WWW_AUTHENTICATE));
        assert_eq!(error_code(duplicate).await, "api_token_required");
        let invalid_accept = router
            .clone()
            .oneshot(kagemusha_request(
                1,
                "application/x-norito",
                "application/json;q=2",
            ))
            .await
            .expect("invalid-Accept response");
        assert_eq!(invalid_accept.status(), StatusCode::NOT_ACCEPTABLE);
        assert_eq!(error_code(invalid_accept).await, "response_not_acceptable");
        let mut non_ascii_accept = kagemusha_request(1, "application/x-norito", "application/json");
        non_ascii_accept.headers_mut().append(
            header::ACCEPT,
            HeaderValue::from_bytes(&[0xff]).expect("opaque Accept fixture"),
        );
        let invalid_accept = router
            .clone()
            .oneshot(non_ascii_accept)
            .await
            .expect("non-ASCII Accept response");
        assert_eq!(invalid_accept.status(), StatusCode::NOT_ACCEPTABLE);
        assert_eq!(error_code(invalid_accept).await, "response_not_acceptable");
        let mut duplicate_content_type =
            kagemusha_request(1, "application/x-norito", "application/json");
        duplicate_content_type.headers_mut().append(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-norito"),
        );
        let invalid_content_type = router
            .clone()
            .oneshot(duplicate_content_type)
            .await
            .expect("duplicate-Content-Type response");
        assert_eq!(invalid_content_type.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_code(invalid_content_type).await,
            "request_content_type_invalid"
        );
        let mut non_ascii_content_type =
            kagemusha_request(1, "application/x-norito", "application/json");
        non_ascii_content_type.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_bytes(&[0xff]).expect("opaque Content-Type fixture"),
        );
        let invalid_content_type = router
            .clone()
            .oneshot(non_ascii_content_type)
            .await
            .expect("non-ASCII Content-Type response");
        assert_eq!(invalid_content_type.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_code(invalid_content_type).await,
            "request_content_type_invalid"
        );
        let json_content_type = router
            .clone()
            .oneshot(kagemusha_request(1, "application/json", "application/json"))
            .await
            .expect("JSON content-type response");
        assert_eq!(
            json_content_type.status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            error_code(json_content_type).await,
            "request_content_type_unsupported"
        );
        let missing_idempotency_key = router
            .oneshot(kagemusha_request(
                1,
                "application/x-norito",
                "application/json",
            ))
            .await
            .expect("missing-idempotency-key response");
        assert_eq!(missing_idempotency_key.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_code(missing_idempotency_key).await,
            "idempotency_key_missing"
        );
        assert_eq!(
            handler_calls.load(Ordering::SeqCst),
            0,
            "admission failures must not invoke a handler or consume its body extractor"
        );
    }
}

#[cfg(test)]
mod emergency_fast_surface_tests {
    use super::*;

    #[test]
    fn emergency_fast_exposes_only_bounded_operational_routes() {
        for descriptor in [
            route_catalog::core::HEALTH,
            route_catalog::core::LIVEZ,
            route_catalog::core::READYZ,
            route_catalog::core::PEERS,
            route_catalog::core::CONFIGURATION_GET,
        ] {
            assert!(emergency_fast_route_is_available(
                &MatchedRouteMetadata::from_descriptor(descriptor)
            ));
        }
        assert!(!emergency_fast_route_is_available(
            &MatchedRouteMetadata::from_descriptor(route_catalog::pipeline::QUERY)
        ));
        for descriptor in [
            route_catalog::core::API_VERSION,
            route_catalog::diagnostic::STATUS,
            route_catalog::diagnostic::METRICS,
        ] {
            assert!(!emergency_fast_route_is_available(
                &MatchedRouteMetadata::from_descriptor(descriptor)
            ));
        }
    }

    #[cfg(feature = "connect")]
    #[test]
    fn emergency_fast_does_not_attach_the_torii_proxy_control_subscriber() {
        let compact_source: String = include_str!("lib.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        assert!(compact_source.contains(
            "lettorii_proxy_network_worker=ifemergency_fast{None}else{self.p2p.clone().map(|network|{attach_torii_proxy_network(app_state.clone(),network,shutdown_signal.clone(),)})};",
        ));
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn emergency_fast_does_not_expose_public_dataspace_upstreams() {
        let compact_source: String = include_str!("lib.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        assert!(compact_source.contains(
            "letpublic_dataspace_upstreams=Arc::new(ifemergency_fast{BTreeMap::new()}else{configured_public_dataspace_upstreams});",
        ));
    }

    #[test]
    fn emergency_fast_disables_optional_torii_background_services() {
        let compact_source: String = include_str!("lib.rs")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        for disabled in [
            "config.peer_telemetry_urls.clear();",
            "config.connect.enabled=false;",
            "config.mcp.enabled=false;",
        ] {
            assert!(compact_source.contains(disabled));
        }
        assert!(compact_source.contains(
            "if!emergency_fast{letmutrx=self.events.subscribe();letcache=self.pipeline_status_cache.clone();",
        ));
        assert!(compact_source.contains(
            "ifself.kura.emergency_fast_startup_enabled(){iroha_logger::warn!(\"emergencyFastKuramodeleavesToriidetachedfromP2Prelayservices\");returnself;}self.p2p=Some(p2p);",
        ));

        assert!(
            compact_source
                .contains("letconnect_runtime_enabled=!emergency_fast&&self.connect_enabled;",)
        );
    }
}

#[cfg(test)]
mod strict_request_target_tests {
    include!("tests/lib_strict_request_targets.rs");
}

#[cfg(test)]
mod bridge_finality_attestation_route_tests {
    use super::*;
    #[test]
    fn challenge_header_is_exact_nonzero_lowercase_hex() {
        let raw = [0xAB; 32];
        let canonical = hex::encode(raw);
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            BRIDGE_FINALITY_CHALLENGE_HEADER,
            axum::http::HeaderValue::from_str(&canonical).expect("canonical header"),
        );
        assert_eq!(bridge_finality_challenge(&headers).expect("challenge"), raw);
        for hostile in [
            canonical.to_ascii_uppercase(),
            "00".repeat(32),
            "ab".repeat(31),
            format!("{}g", "ab".repeat(31)),
        ] {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                BRIDGE_FINALITY_CHALLENGE_HEADER,
                axum::http::HeaderValue::from_str(&hostile).expect("ASCII hostile header"),
            );
            assert!(bridge_finality_challenge(&headers).is_err(), "{hostile}");
        }
        assert!(bridge_finality_challenge(&axum::http::HeaderMap::new()).is_err());
        let mut duplicated = axum::http::HeaderMap::new();
        let value = axum::http::HeaderValue::from_str(&canonical).expect("canonical header");
        duplicated.append(BRIDGE_FINALITY_CHALLENGE_HEADER, value.clone());
        duplicated.append(BRIDGE_FINALITY_CHALLENGE_HEADER, value);
        assert!(bridge_finality_challenge(&duplicated).is_err());
    }
    #[test]
    fn attestation_responses_are_never_cacheable_and_vary_by_challenge() {
        let mut response = axum::response::Response::new(axum::body::Body::empty());
        protect_bridge_finality_attestation_response(&mut response);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::VARY)
                .and_then(|value| value.to_str().ok()),
            Some("X-Iroha-Finality-Challenge, Accept")
        );
        let propagated_error = bridge_finality_challenge_error("invalid challenge");
        let response = finalize_bridge_finality_attestation_response(Err(propagated_error));
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::VARY)
                .and_then(|value| value.to_str().ok()),
            Some("X-Iroha-Finality-Challenge, Accept")
        );
    }
}

#[cfg(all(test, feature = "connect"))]
mod torii_proxy_session_id_tests {
    use super::*;

    #[derive(Debug)]
    struct FailingEntropy;

    impl core::fmt::Display for FailingEntropy {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("injected entropy failure")
        }
    }

    struct FailingRng;

    impl rand::rand_core::TryRngCore for FailingRng {
        type Error = FailingEntropy;

        fn try_next_u32(&mut self) -> core::result::Result<u32, Self::Error> {
            Err(FailingEntropy)
        }

        fn try_next_u64(&mut self) -> core::result::Result<u64, Self::Error> {
            Err(FailingEntropy)
        }

        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> core::result::Result<(), Self::Error> {
            Err(FailingEntropy)
        }
    }

    impl rand::rand_core::TryCryptoRng for FailingRng {}

    #[test]
    fn process_session_entropy_failure_is_a_startup_error() {
        let error = try_new_torii_proxy_session_id_with_rng(&mut FailingRng)
            .expect_err("entropy failure must not unwind");
        assert!(matches!(
            error,
            ToriiBuildError::ComponentInitialization {
                component: "torii_proxy.session_entropy",
                ..
            }
        ));
    }
}

#[cfg(all(test, feature = "app_api"))]
mod canonical_stream_handshake_tests {
    use super::*;
    use axum::extract::FromRequestParts;
    use axum::http::{HeaderMap, HeaderValue, header, request::Parts};
    use axum::{Router, body::Body, routing::get};
    use std::{
        collections::HashSet,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use tower::ServiceExt as _;
    fn error_code(error: Error) -> &'static str {
        match error {
            Error::AppQueryValidation { code, .. } => code,
            other => panic!("unexpected stream handshake error: {other:?}"),
        }
    }
    #[derive(Clone)]
    struct SyntaxProbe(Arc<AtomicUsize>);
    struct RejectingStreamSyntax;
    impl<S> FromRequestParts<S> for RejectingStreamSyntax
    where
        S: Send + Sync,
    {
        type Rejection = StatusCode;
        fn from_request_parts(
            parts: &mut Parts,
            _state: &S,
        ) -> impl core::future::Future<Output = Result<Self, Self::Rejection>> + Send {
            if let Some(probe) = parts.extensions.get::<SyntaxProbe>() {
                probe.0.fetch_add(1, Ordering::SeqCst);
            }
            core::future::ready(Err(StatusCode::BAD_REQUEST))
        }
    }
    async fn rejecting_stream_syntax(_: RejectingStreamSyntax) -> StatusCode {
        StatusCode::NO_CONTENT
    }
    fn gated_syntax_router(
        app: SharedAppState,
        route: iroha_torii_shared::route_catalog::RouteDescriptor,
    ) -> Router {
        Router::new().route(
            route.path(),
            get(rejecting_stream_syntax).layer(axum::middleware::from_fn_with_state(
                CanonicalStreamAdmission { app, route },
                enforce_canonical_stream_admission,
            )),
        )
    }
    fn stream_syntax_request(
        route: iroha_torii_shared::route_catalog::RouteDescriptor,
        probe: &Arc<AtomicUsize>,
        token: Option<&str>,
        remote: Option<std::net::SocketAddr>,
    ) -> Request<Body> {
        let mut builder = Request::builder().uri(route.path());
        if let Some(token) = token {
            builder = builder.header(HEADER_API_TOKEN, token);
        }
        let mut request = builder.body(Body::empty()).expect("stream syntax request");
        request
            .extensions_mut()
            .insert(SyntaxProbe(Arc::clone(probe)));
        if let Some(remote) = remote {
            request
                .extensions_mut()
                .insert(axum::extract::ConnectInfo(remote));
        }
        request
    }
    fn gated_event_websocket_router(app: SharedAppState) -> Router {
        let route = route_catalog::streaming::SUBSCRIPTION_WS;
        Router::new()
            .route(
                route.path(),
                get(handler_subscription_ws).layer(axum::middleware::from_fn_with_state(
                    CanonicalStreamAdmission {
                        app: Arc::clone(&app),
                        route,
                    },
                    enforce_canonical_stream_admission,
                )),
            )
            .with_state(app)
    }
    #[test]
    fn websocket_requires_exact_single_subprotocol() {
        let headers = HeaderMap::new();
        assert_eq!(
            error_code(validate_norito_websocket_handshake(&headers, None).unwrap_err()),
            "websocket_subprotocol_required"
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
        );
        validate_norito_websocket_handshake(&headers, None).expect("canonical subprotocol");
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("IROHA-NORITO-V1"),
        );
        assert_eq!(
            error_code(validate_norito_websocket_handshake(&headers, None).unwrap_err()),
            "websocket_subprotocol_unsupported"
        );
    }
    #[test]
    fn websocket_rejects_protocol_lists_duplicates_and_empty_tokens() {
        for value in [
            "iroha-norito-v1, other",
            "other, iroha-norito-v1",
            "iroha-norito-v1, iroha-norito-v1",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_str(value).expect("header value"),
            );
            assert_eq!(
                error_code(validate_norito_websocket_handshake(&headers, None).unwrap_err()),
                "websocket_subprotocol_unsupported",
                "value: {value}"
            );
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("iroha-norito-v1,"),
        );
        assert_eq!(
            error_code(validate_norito_websocket_handshake(&headers, None).unwrap_err()),
            "websocket_subprotocol_invalid"
        );
        let mut headers = HeaderMap::new();
        headers.append(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
        );
        headers.append(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
        );
        assert_eq!(
            error_code(validate_norito_websocket_handshake(&headers, None).unwrap_err()),
            "websocket_subprotocol_unsupported"
        );
    }
    #[test]
    fn websocket_rejects_resume_headers_and_query_parameters() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(NORITO_V1_WEBSOCKET_SUBPROTOCOL),
        );
        headers.insert("last-event-id", HeaderValue::from_static("cursor"));
        assert_eq!(
            error_code(validate_norito_websocket_handshake(&headers, None).unwrap_err()),
            "stream_resume_unsupported"
        );
        headers.remove("last-event-id");
        assert_eq!(
            error_code(
                validate_norito_websocket_handshake(&headers, Some("cursor=1")).unwrap_err()
            ),
            "stream_query_unsupported"
        );
        validate_norito_websocket_handshake(&headers, Some(""))
            .expect("empty query carries no parameters");
    }
    #[test]
    fn sse_rejects_any_last_event_id_field() {
        let mut headers = HeaderMap::new();
        assert!(sse_resume_rejection(&headers).is_none());
        headers.insert("last-event-id", HeaderValue::from_static(""));
        let response = sse_resume_rejection(&headers).expect("resume rejection");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-stream-error")
                .and_then(|value| value.to_str().ok()),
            Some("stream_resume_unsupported")
        );
    }
    #[tokio::test]
    async fn api_token_authentication_precedes_stream_establishment() {
        let mut app = crate::mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_token_digests =
            Arc::new(limits::ApiTokenDigestSet::from_tokens(["stream-secret"]));
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let router = Router::new()
            .route(
                "/v1/events/ws",
                get(move || {
                    let calls = Arc::clone(&handler_calls);
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        StatusCode::SWITCHING_PROTOCOLS
                    }
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                Arc::clone(&app),
                enforce_api_token,
            ));
        let unauthorized = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/events/ws")
                    .header(header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("unauthorized response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(
            unauthorized
                .headers()
                .contains_key(header::WWW_AUTHENTICATE)
        );
        let authorized = router
            .oneshot(
                Request::builder()
                    .uri("/v1/events/ws")
                    .header(HEADER_API_TOKEN, "stream-secret")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("authorized response");
        assert_eq!(authorized.status(), StatusCode::SWITCHING_PROTOCOLS);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn stream_admission_precedes_syntax_and_uses_the_stable_route_id() {
        let mut unauthorized_app = crate::mk_app_state_for_tests();
        let state = Arc::get_mut(&mut unauthorized_app).expect("unique app state");
        state.require_api_token = true;
        state.api_token_digests =
            Arc::new(limits::ApiTokenDigestSet::from_tokens(["stream-secret"]));
        let receivers_before = unauthorized_app.events.receiver_count();
        let syntax_calls = Arc::new(AtomicUsize::new(0));
        let router = gated_syntax_router(
            Arc::clone(&unauthorized_app),
            route_catalog::streaming::EVENTS_SSE,
        );
        let mut request = Request::builder()
            .uri("/v1/events/sse?filter=%FF")
            .body(Body::empty())
            .expect("unauthorized malformed SSE request");
        request
            .extensions_mut()
            .insert(SyntaxProbe(Arc::clone(&syntax_calls)));
        let response = router
            .oneshot(request)
            .await
            .expect("unauthorized response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(syntax_calls.load(Ordering::SeqCst), 0);
        assert_eq!(unauthorized_app.events.receiver_count(), receivers_before);
        let mut limited_app = crate::mk_app_state_for_tests();
        Arc::get_mut(&mut limited_app)
            .expect("unique app state")
            .rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        let receivers_before = limited_app.events.receiver_count();
        let syntax_calls = Arc::new(AtomicUsize::new(0));
        let router = gated_syntax_router(
            Arc::clone(&limited_app),
            route_catalog::streaming::EVENTS_SSE,
        );
        for (query, expected_status, expected_calls) in [
            ("filter=%FF", StatusCode::BAD_REQUEST, 1),
            (
                "different_attacker_key=%FE",
                StatusCode::TOO_MANY_REQUESTS,
                1,
            ),
        ] {
            let mut request = Request::builder()
                .uri(format!("/v1/events/sse?{query}"))
                .body(Body::empty())
                .expect("malformed SSE request");
            request
                .extensions_mut()
                .insert(SyntaxProbe(Arc::clone(&syntax_calls)));
            let response = router
                .clone()
                .oneshot(request)
                .await
                .expect("SSE admission response");
            assert_eq!(response.status(), expected_status, "query={query}");
            assert_eq!(
                syntax_calls.load(Ordering::SeqCst),
                expected_calls,
                "query={query}"
            );
        }
        assert_eq!(limited_app.events.receiver_count(), receivers_before);
    }
    #[tokio::test]
    async fn stream_high_load_thresholds_shed_before_syntax_or_subscription_creation() {
        let mut stream_app = crate::mk_app_state_for_tests();
        let state = Arc::get_mut(&mut stream_app).expect("unique app state");
        state.high_load_stream_tx_threshold = 0;
        state.high_load_subscription_tx_threshold = usize::MAX;
        let syntax_calls = Arc::new(AtomicUsize::new(0));
        let response = gated_syntax_router(
            Arc::clone(&stream_app),
            route_catalog::streaming::EVENTS_SSE,
        )
        .oneshot(stream_syntax_request(
            route_catalog::streaming::EVENTS_SSE,
            &syntax_calls,
            None,
            None,
        ))
        .await
        .expect("high-load stream response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(syntax_calls.load(Ordering::SeqCst), 0);
        let mut subscription_app = crate::mk_app_state_for_tests();
        let state = Arc::get_mut(&mut subscription_app).expect("unique app state");
        state.high_load_stream_tx_threshold = usize::MAX;
        state.high_load_subscription_tx_threshold = 0;
        let receivers_before = subscription_app.events.receiver_count();
        let syntax_calls = Arc::new(AtomicUsize::new(0));
        let response = gated_syntax_router(
            Arc::clone(&subscription_app),
            route_catalog::streaming::SUBSCRIPTION_WS,
        )
        .oneshot(stream_syntax_request(
            route_catalog::streaming::SUBSCRIPTION_WS,
            &syntax_calls,
            None,
            None,
        ))
        .await
        .expect("high-load subscription response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(syntax_calls.load(Ordering::SeqCst), 0);
        assert_eq!(subscription_app.events.receiver_count(), receivers_before);
    }
    #[tokio::test]
    async fn stream_token_rate_limit_buckets_are_isolated_by_route() {
        let mut app = crate::mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_token_digests =
            Arc::new(limits::ApiTokenDigestSet::from_tokens(["stream-secret"]));
        state.rate_limiter = limits::RateLimiter::new_per_minute(Some(1), Some(1));
        let events_route = route_catalog::streaming::EVENTS_SSE;
        let contracts_route = route_catalog::streaming::CONTRACT_EVENTS_SSE;
        let events = gated_syntax_router(Arc::clone(&app), events_route);
        let contracts = gated_syntax_router(Arc::clone(&app), contracts_route);
        let syntax_calls = Arc::new(AtomicUsize::new(0));
        for (router, route, remote, expected_status, expected_calls) in [
            (
                &events,
                events_route,
                std::net::SocketAddr::from(([203, 0, 113, 10], 1010)),
                StatusCode::BAD_REQUEST,
                1,
            ),
            (
                &contracts,
                contracts_route,
                std::net::SocketAddr::from(([203, 0, 113, 11], 1011)),
                StatusCode::BAD_REQUEST,
                2,
            ),
            (
                &events,
                events_route,
                std::net::SocketAddr::from(([203, 0, 113, 12], 1012)),
                StatusCode::TOO_MANY_REQUESTS,
                2,
            ),
            (
                &contracts,
                contracts_route,
                std::net::SocketAddr::from(([203, 0, 113, 13], 1013)),
                StatusCode::TOO_MANY_REQUESTS,
                2,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(stream_syntax_request(
                    route,
                    &syntax_calls,
                    Some("stream-secret"),
                    Some(remote),
                ))
                .await
                .expect("token-scoped stream admission response");
            assert_eq!(response.status(), expected_status, "route={}", route.path());
            assert_eq!(
                syntax_calls.load(Ordering::SeqCst),
                expected_calls,
                "route={}",
                route.path()
            );
        }
    }
    #[tokio::test]
    async fn stream_connect_info_rate_limit_buckets_are_isolated_by_route() {
        let mut app = crate::mk_app_state_for_tests();
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .rate_limiter = limits::RateLimiter::new_per_minute(Some(1), Some(1));
        let events_route = route_catalog::streaming::EVENTS_SSE;
        let contracts_route = route_catalog::streaming::CONTRACT_EVENTS_SSE;
        let events = gated_syntax_router(Arc::clone(&app), events_route);
        let contracts = gated_syntax_router(Arc::clone(&app), contracts_route);
        let syntax_calls = Arc::new(AtomicUsize::new(0));
        let remote_ip = [203, 0, 113, 42];
        for (router, route, token, port, expected_status, expected_calls) in [
            (
                &events,
                events_route,
                "attacker-a",
                2010,
                StatusCode::BAD_REQUEST,
                1,
            ),
            (
                &contracts,
                contracts_route,
                "attacker-b",
                2011,
                StatusCode::BAD_REQUEST,
                2,
            ),
            (
                &events,
                events_route,
                "attacker-c",
                2012,
                StatusCode::TOO_MANY_REQUESTS,
                2,
            ),
            (
                &contracts,
                contracts_route,
                "attacker-d",
                2013,
                StatusCode::TOO_MANY_REQUESTS,
                2,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(stream_syntax_request(
                    route,
                    &syntax_calls,
                    Some(token),
                    Some(std::net::SocketAddr::from((remote_ip, port))),
                ))
                .await
                .expect("IP-scoped stream admission response");
            assert_eq!(response.status(), expected_status, "route={}", route.path());
            assert_eq!(
                syntax_calls.load(Ordering::SeqCst),
                expected_calls,
                "route={}",
                route.path()
            );
        }
    }
    #[tokio::test]
    async fn stream_admission_precedes_websocket_upgrade_and_subscriber_creation() {
        let mut unauthorized_app = crate::mk_app_state_for_tests();
        let state = Arc::get_mut(&mut unauthorized_app).expect("unique app state");
        state.require_api_token = true;
        state.api_token_digests =
            Arc::new(limits::ApiTokenDigestSet::from_tokens(["stream-secret"]));
        let receivers_before = unauthorized_app.events.receiver_count();
        let response = gated_event_websocket_router(Arc::clone(&unauthorized_app))
            .oneshot(
                Request::builder()
                    .uri("/v1/events/ws?unsupported=1")
                    .body(Body::empty())
                    .expect("unauthorized malformed WebSocket request"),
            )
            .await
            .expect("unauthorized WebSocket response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(unauthorized_app.events.receiver_count(), receivers_before);
        let mut limited_app = crate::mk_app_state_for_tests();
        Arc::get_mut(&mut limited_app)
            .expect("unique app state")
            .rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        let receivers_before = limited_app.events.receiver_count();
        let router = gated_event_websocket_router(Arc::clone(&limited_app));
        let first = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/events/ws?unsupported=first")
                    .body(Body::empty())
                    .expect("first malformed WebSocket request"),
            )
            .await
            .expect("first WebSocket response");
        assert_eq!(first.status(), StatusCode::BAD_REQUEST);
        let second = router
            .oneshot(
                Request::builder()
                    .uri("/v1/events/ws?unsupported=second")
                    .body(Body::empty())
                    .expect("rate-limited malformed WebSocket request"),
            )
            .await
            .expect("rate-limited WebSocket response");
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(limited_app.events.receiver_count(), receivers_before);
    }
}

#[cfg(all(test, feature = "app_api"))]
mod ws_disconnect_classification_tests {
    use super::is_expected_ws_disconnect;
    #[test]
    fn detects_expected_disconnect_errors() {
        assert!(is_expected_ws_disconnect(&eyre::eyre!(
            "WebSocket error: IO error: Broken pipe (os error 32)"
        )));
        assert!(is_expected_ws_disconnect(&eyre::eyre!(
            "Event consumption resulted in an error: Connection is closed"
        )));
        assert!(is_expected_ws_disconnect(&eyre::eyre!(
            "Event stream authorization was revoked"
        )));
    }
    #[test]
    fn keeps_unexpected_errors_as_failures() {
        assert!(!is_expected_ws_disconnect(&eyre::eyre!(
            "event stream lagged; skipping buffered events"
        )));
    }
}

#[cfg(test)]
mod cors_runtime_validation_tests {
    use super::{Torii, ToriiBuildError};

    fn owned(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn cors_origins_require_unique_canonical_http_origins() {
        let parsed = Torii::parse_cors_origins(&owned(&["https://wallet.example:8443"]))
            .expect("canonical explicit origin");
        assert_eq!(parsed[0], "https://wallet.example:8443");

        for origins in [
            owned(&["*"]),
            owned(&[""]),
            owned(&[" https://wallet.example"]),
            owned(&["https://wallet.example/path"]),
            owned(&["https://wallet.example", "https://wallet.example"]),
        ] {
            assert!(matches!(
                Torii::parse_cors_origins(&origins),
                Err(ToriiBuildError::InvalidConfiguration {
                    component: "cors.allowed_origins",
                    ..
                })
            ));
        }
    }

    #[test]
    fn cors_methods_require_unique_canonical_standard_methods() {
        let parsed =
            Torii::parse_cors_methods(&owned(&["GET", "POST"])).expect("canonical methods");
        assert_eq!(parsed, [axum::http::Method::GET, axum::http::Method::POST]);

        for methods in [
            owned(&[""]),
            owned(&[" GET"]),
            owned(&["get"]),
            owned(&["TRACE"]),
            owned(&["GET", "GET"]),
        ] {
            assert!(Torii::parse_cors_methods(&methods).is_err());
        }
    }

    #[test]
    fn cors_headers_require_unique_lowercase_names() {
        assert!(
            Torii::parse_cors_headers("cors.allowed_headers", &[])
                .expect("simple CORS needs no allowed request headers")
                .is_empty()
        );
        let parsed = Torii::parse_cors_headers(
            "cors.allowed_headers",
            &owned(&["content-type", "authorization"]),
        )
        .expect("canonical header names");
        assert_eq!(parsed[0].as_str(), "content-type");

        for headers in [
            owned(&[""]),
            owned(&["content-type "]),
            owned(&["Content-Type"]),
            owned(&["content type"]),
            owned(&["content-type", "content-type"]),
        ] {
            assert!(Torii::parse_cors_headers("cors.allowed_headers", &headers).is_err());
        }
    }
}
