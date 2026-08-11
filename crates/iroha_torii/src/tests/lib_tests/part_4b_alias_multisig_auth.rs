    #[tokio::test]
    async fn alias_resolve_index_rejects_unsigned_request() {
        let authority = checked_torii_test_account_id(
            0x0a,
            "derive alias resolve-index unsigned authority fixture key",
        );
        let alias_label = AccountAlias::new(
            "banking".parse().expect("label"),
            Some(iroha_data_model::account::rekey::AccountAliasDomain::new(
                "centralbank".parse::<Name>().expect("domain id"),
            )),
            DataSpaceId::UNIVERSAL,
        );
        let authority_account = Account::new(authority.clone()).build(&authority);
        let domain = Domain::new(DomainId::try_new("centralbank", "universal").expect("domain id"))
            .build(&authority);
        let account = Account::new(authority.clone())
            .with_label(Some(alias_label))
            .build(&authority);
        let body = norito::json::to_vec(&routing::AliasResolveIndexRequestDto { index: 0 })
            .expect("encode request");
        let error = handler_alias_resolve_index(
            State(mk_app_state_for_tests_with_world(World::with(
                [domain],
                [authority_account, account],
                [],
            ))),
            axum::http::Method::POST,
            "/v1/aliases/resolve-index"
                .parse()
                .expect("alias resolve-index uri"),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        .expect_err("unsigned index enumeration must be rejected");

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "alias_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn alias_resolve_index_rejects_malformed_json_body() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x0b,
            "derive alias resolve-index malformed body authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let body = b"{";
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, body);

        let err = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from_static(body),
        )
        .await
        .expect_err("malformed resolve-index bodies should be rejected");

        match err {
            Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => assert!(
                !message.trim().is_empty(),
                "malformed request bodies should surface a parse diagnostic"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn multisig_read_payload<T>(value: T) -> NoritoJsonWithBytes<T>
    where
        T: norito::json::JsonSerialize,
    {
        let raw = norito::json::to_vec(&value).expect("encode multisig read request");
        NoritoJsonWithBytes {
            value,
            raw: axum::body::Bytes::from(raw),
        }
    }

    #[tokio::test]
    async fn contract_code_artifact_read_rejects_unsigned_requests() {
        let uri: axum::http::Uri = format!("/v1/contracts/code-bytes/{}", "a".repeat(64))
            .parse()
            .expect("contract code URI");
        let error = match handler_get_contract_code_bytes(
            State(mk_app_state_for_tests()),
            Method::GET,
            uri,
            HeaderMap::new(),
            crate::loopback_connect_info(),
            axum::extract::Path("a".repeat(64)),
        )
        .await
        {
            Ok(_) => panic!("unsigned contract artifact read must fail closed"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "contract_code_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn multisig_spec_rejects_unsigned_alias_selector() {
        let request = routing::MultisigSpecRequestDto {
            selector: routing::MultisigAccountSelectorDto {
                multisig_account_id: None,
                multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
            },
        };
        let error = handler_post_multisig_spec(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_SPEC
                .parse()
                .expect("multisig spec uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed");

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "multisig_read_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn multisig_proposals_query_rejects_unsigned_alias_selector() {
        let request = routing::MultisigProposalsQueryRequestDto {
            selector: routing::MultisigAccountSelectorDto {
                multisig_account_id: None,
                multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
            },
            status: Vec::new(),
            cursor: None,
            limit: None,
        };
        let error = handler_post_multisig_proposals_query(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_QUERY
                .parse()
                .expect("multisig proposals query uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed");

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "multisig_read_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn multisig_proposals_resolve_rejects_unsigned_alias_selector() {
        let request = routing::MultisigProposalsResolveRequestDto {
            selector: routing::MultisigAccountSelectorDto {
                multisig_account_id: None,
                multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
            },
            proposal_id: Some("deadbeef".to_owned()),
            instructions_hash: None,
        };
        let error = handler_post_multisig_proposals_resolve(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_RESOLVE
                .parse()
                .expect("multisig proposals resolve uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed");

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "multisig_read_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn multisig_reads_reject_unsigned_concrete_account_selectors() {
        let selector = || routing::MultisigAccountSelectorDto {
            multisig_account_id: Some((*ALICE_ID).clone()),
            multisig_account_alias: None,
        };

        let spec = handler_post_multisig_spec(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_SPEC
                .parse()
                .expect("multisig spec uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(routing::MultisigSpecRequestDto {
                selector: selector(),
            }),
        )
        .await
        .expect_err("unsigned concrete spec read must fail closed")
        .into_response();
        assert_eq!(spec.status(), StatusCode::UNAUTHORIZED);

        let query = handler_post_multisig_proposals_query(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_QUERY
                .parse()
                .expect("multisig proposals query uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(routing::MultisigProposalsQueryRequestDto {
                selector: selector(),
                status: Vec::new(),
                cursor: None,
                limit: None,
            }),
        )
        .await
        .expect_err("unsigned concrete proposal query must fail closed")
        .into_response();
        assert_eq!(query.status(), StatusCode::UNAUTHORIZED);

        let resolve = handler_post_multisig_proposals_resolve(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_RESOLVE
                .parse()
                .expect("multisig proposals resolve uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(routing::MultisigProposalsResolveRequestDto {
                selector: selector(),
                proposal_id: Some("a".repeat(64)),
                instructions_hash: None,
            }),
        )
        .await
        .expect_err("unsigned concrete proposal resolve must fail closed")
        .into_response();
        assert_eq!(resolve.status(), StatusCode::UNAUTHORIZED);
    }

    fn multisig_read_contract_test_router(app: SharedAppState) -> Router {
        Router::new()
            .route(
                route_catalog::contracts_and_verification_keys::MULTISIG_SPEC_POST.path(),
                post(handler_post_multisig_spec)
                    .layer(DefaultBodyLimit::max(MULTISIG_READ_MAX_BODY_BYTES)),
            )
            .route(
                route_catalog::contracts_and_verification_keys::MULTISIG_PROPOSALS_QUERY_POST
                    .path(),
                post(handler_post_multisig_proposals_query)
                    .layer(DefaultBodyLimit::max(MULTISIG_READ_MAX_BODY_BYTES)),
            )
            .route(
                route_catalog::contracts_and_verification_keys::MULTISIG_PROPOSALS_RESOLVE_POST
                    .path(),
                post(handler_post_multisig_proposals_resolve)
                    .layer(DefaultBodyLimit::max(MULTISIG_READ_MAX_BODY_BYTES)),
            )
            .fallback(|| async { StatusCode::NOT_FOUND })
            .with_state(app)
    }

    fn multisig_read_contract_request(
        method: HttpMethod,
        path: &str,
        body: impl Into<Body>,
    ) -> Request<Body> {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .header(axum::http::header::ACCEPT, "application/json")
            .body(body.into())
            .expect("multisig read contract request");
        request
            .extensions_mut()
            .insert(crate::loopback_connect_info());
        request
    }

    #[tokio::test]
    async fn multisig_read_http_contract_is_signed_post_only_closed_and_bounded() {
        let router = multisig_read_contract_test_router(mk_app_state_for_tests());
        let alias_body = r#"{"multisig_account_alias":"banking@centralbank.universal"}"#;

        let unsigned = router
            .clone()
            .oneshot(multisig_read_contract_request(
                HttpMethod::POST,
                "/v1/multisig/spec",
                alias_body,
            ))
            .await
            .expect("unsigned spec response");
        assert_eq!(
            unsigned.status(),
            StatusCode::UNAUTHORIZED,
            "unsigned alias selectors must fail before alias resolution"
        );

        for path in [
            "/v1/multisig/spec",
            "/v1/multisig/proposals/query",
            "/v1/multisig/proposals/resolve",
        ] {
            let method_response = router
                .clone()
                .oneshot(multisig_read_contract_request(
                    HttpMethod::GET,
                    path,
                    Body::empty(),
                ))
                .await
                .expect("method response");
            assert_eq!(
                method_response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{path}"
            );
        }
        for retired in [
            "/v1/multisig/proposals/lookup",
            "/v1/multisig/proposals/list",
            "/v1/multisig/proposals/get",
            "/v1/multisig/proposals/search",
        ] {
            let response = router
                .clone()
                .oneshot(multisig_read_contract_request(
                    HttpMethod::POST,
                    retired,
                    alias_body,
                ))
                .await
                .expect("retired route response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{retired}");
        }

        for (path, body) in [
            (
                "/v1/multisig/spec",
                r#"{"multisig_account_alias":"banking@centralbank.universal","extra":true}"#,
            ),
            (
                "/v1/multisig/proposals/query",
                r#"{"multisig_account_alias":"banking@centralbank.universal","status":[],"extra":true}"#,
            ),
            (
                "/v1/multisig/proposals/resolve",
                r#"{"multisig_account_alias":"banking@centralbank.universal","proposal_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","extra":true}"#,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(multisig_read_contract_request(HttpMethod::POST, path, body))
                .await
                .expect("closed-schema response");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        }

        let malformed = router
            .clone()
            .oneshot(multisig_read_contract_request(
                HttpMethod::POST,
                "/v1/multisig/proposals/query",
                r#"{"multisig_account_alias": "unterminated"#,
            ))
            .await
            .expect("malformed JSON response");
        assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);

        let mut missing_content_type = multisig_read_contract_request(
            HttpMethod::POST,
            "/v1/multisig/proposals/query",
            alias_body,
        );
        missing_content_type
            .headers_mut()
            .remove(axum::http::header::CONTENT_TYPE);
        let missing_content_type = router
            .clone()
            .oneshot(missing_content_type)
            .await
            .expect("missing Content-Type response");
        assert_eq!(
            missing_content_type.status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );

        let oversized = format!(
            "{{\"multisig_account_alias\":\"banking@centralbank.universal\",\"padding\":\"{}\"}}",
            "x".repeat(MULTISIG_READ_MAX_BODY_BYTES)
        );
        let oversized_response = router
            .oneshot(multisig_read_contract_request(
                HttpMethod::POST,
                "/v1/multisig/proposals/query",
                oversized,
            ))
            .await
            .expect("oversized response");
        assert_eq!(oversized_response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn multisig_read_handler_requires_api_token_and_signed_viewer_auth() {
        let mut app = mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::from(["valid-token".to_owned()]));
        let router = multisig_read_contract_test_router(app);
        let canonical_account_id = checked_torii_test_account_id(
            0x0c,
            "derive multisig API-token policy account fixture key",
        );
        let body = norito::json::to_vec(&routing::MultisigSpecRequestDto {
            selector: routing::MultisigAccountSelectorDto {
                multisig_account_id: Some(canonical_account_id),
                multisig_account_alias: None,
            },
        })
        .expect("encode canonical multisig selector");

        let missing = router
            .clone()
            .oneshot(multisig_read_contract_request(
                HttpMethod::POST,
                "/v1/multisig/spec",
                body.clone(),
            ))
            .await
            .expect("missing-token response");
        assert_eq!(missing.status(), StatusCode::FORBIDDEN);

        let mut authenticated =
            multisig_read_contract_request(HttpMethod::POST, "/v1/multisig/spec", body);
        authenticated
            .headers_mut()
            .insert(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
        let still_unsigned = router
            .oneshot(authenticated)
            .await
            .expect("authenticated read response");
        assert_eq!(still_unsigned.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn browser_read_endpoints_are_not_throttled_by_deploy_limiter() {
        let app = mk_app_state_for_tests_with_options(None, Some((1, 1)), None, None);
        let headers = HeaderMap::new();
        let remote_ip = std::net::IpAddr::from([127, 0, 0, 1]);

        let key = super::rate_limit_key(
            &headers,
            Some(remote_ip),
            "v1/contracts/state",
            app.api_token_enforced(),
        );
        assert!(app.deploy_rate_limiter.allow(&key).await);
        assert!(!app.deploy_rate_limiter.allow(&key).await);

        let contract_state_response = match handler_get_contract_state(
            State(app.clone()),
            headers.clone(),
            crate::loopback_connect_info(),
            AxQuery(routing::ContractStateQuery {
                prefix: Some("missing".to_owned()),
                ..Default::default()
            }),
        )
        .await
        {
            Ok(response) => response.into_response(),
            Err(error) => error.into_response(),
        };
        assert_ne!(
            contract_state_response.status(),
            StatusCode::TOO_MANY_REQUESTS
        );

        let selector = || routing::MultisigAccountSelectorDto {
            multisig_account_id: None,
            multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
        };

        let spec_request = routing::MultisigSpecRequestDto {
            selector: selector(),
        };
        let spec_response = handler_post_multisig_spec(
            State(app.clone()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_SPEC
                .parse()
                .expect("multisig spec uri"),
            headers.clone(),
            crate::loopback_connect_info(),
            multisig_read_payload(spec_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(spec_response.status(), StatusCode::TOO_MANY_REQUESTS);

        let query_request = routing::MultisigProposalsQueryRequestDto {
            selector: selector(),
            status: Vec::new(),
            cursor: None,
            limit: None,
        };
        let query_response = handler_post_multisig_proposals_query(
            State(app.clone()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_QUERY
                .parse()
                .expect("multisig proposals query uri"),
            headers.clone(),
            crate::loopback_connect_info(),
            multisig_read_payload(query_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(query_response.status(), StatusCode::TOO_MANY_REQUESTS);

        let resolve_request = routing::MultisigProposalsResolveRequestDto {
            selector: selector(),
            proposal_id: Some("deadbeef".to_owned()),
            instructions_hash: None,
        };
        let resolve_response = handler_post_multisig_proposals_resolve(
            State(app),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_RESOLVE
                .parse()
                .expect("multisig proposals resolve uri"),
            headers,
            crate::loopback_connect_info(),
            multisig_read_payload(resolve_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(resolve_response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn browser_read_endpoints_use_route_scoped_query_rate_keys() {
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app state");
            state.rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        }
        let headers = HeaderMap::new();
        let remote_ip = std::net::IpAddr::from([127, 0, 0, 1]);

        let shared_key = super::rate_limit_key(
            &headers,
            Some(remote_ip),
            "v1/contracts/state",
            app.api_token_enforced(),
        );
        assert!(app.rate_limiter.allow(&shared_key).await);
        assert!(!app.rate_limiter.allow(&shared_key).await);

        let selector = || routing::MultisigAccountSelectorDto {
            multisig_account_id: None,
            multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
        };

        let spec_request = routing::MultisigSpecRequestDto {
            selector: selector(),
        };
        let spec_response = handler_post_multisig_spec(
            State(app.clone()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_SPEC
                .parse()
                .expect("multisig spec uri"),
            headers.clone(),
            crate::loopback_connect_info(),
            multisig_read_payload(spec_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(spec_response.status(), StatusCode::TOO_MANY_REQUESTS);

        let query_request = routing::MultisigProposalsQueryRequestDto {
            selector: selector(),
            status: Vec::new(),
            cursor: None,
            limit: None,
        };
        let query_response = handler_post_multisig_proposals_query(
            State(app),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_QUERY
                .parse()
                .expect("multisig proposals query uri"),
            headers,
            crate::loopback_connect_info(),
            multisig_read_payload(query_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(query_response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn alias_resolve_index_returns_on_chain_alias_record() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x0d,
            "derive alias resolve-index on-chain authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let alias_label = AccountAlias::new(
            "banking".parse().expect("label"),
            Some(iroha_data_model::account::rekey::AccountAliasDomain::new(
                "centralbank".parse::<Name>().expect("domain id"),
            )),
            DataSpaceId::UNIVERSAL,
        );
        let authority_account = Account::new(authority.clone()).build(&authority);
        let domain = Domain::new(DomainId::try_new("centralbank", "universal").expect("domain id"))
            .build(&authority);
        let account = Account::new(authority.clone())
            .with_label(Some(alias_label.clone()))
            .build(&authority);
        let app = mk_app_state_for_tests_with_world(World::with(
            [domain],
            [authority_account, account],
            [],
        ));
        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);

        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let dto: routing::AliasResolveIndexResponseDto =
            norito::json::from_slice(&body).expect("json decode");
        assert_eq!(dto.index, 0);
        assert_eq!(dto.alias, "banking@centralbank.universal");
        assert_eq!(dto.account_id, authority.to_string());
        assert_eq!(dto.source.as_deref(), Some("on_chain"));
    }

    #[tokio::test]
    async fn alias_resolve_index_rejects_multiroute_before_reachable_source() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x0e,
            "derive alias resolve-index fanout authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");

        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);
        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should return a fixed multi-route rejection")
        .into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("query_unsupported")
        );
        assert!(
            response
                .headers()
                .get("x-iroha-fanout-routes-attempted")
                .is_none(),
            "the rejection must precede alias source execution"
        );
    }

    #[tokio::test]
    async fn alias_resolve_index_rejects_multiroute_before_conflicting_sources() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x0f,
            "derive alias resolve-index route-conflict authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@universal");
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");

        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);
        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("query_unsupported")
        );
        assert!(
            response
                .headers()
                .get("x-iroha-fanout-routes-attempted")
                .is_none(),
            "the rejection must precede conflicting source execution"
        );
    }

    #[tokio::test]
    async fn alias_resolve_index_returns_not_found_when_index_is_missing() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x20,
            "derive alias resolve-index missing authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let authority_account = Account::new(authority.clone()).build(&authority);
        let app = mk_app_state_for_tests_with_world(World::with([], [authority_account], []));
        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);

        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn alias_resolve_index_rejects_multiroute_before_permission_partition_or_fetch()
    {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x21,
            "derive alias resolve-index signed authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-index-miss-offline"));
        let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
            &authority,
            uaid,
            DataSpaceId::new(12),
        ));
        let (_local_route, _foreign_route) =
            crate::tests_runtime_handlers::configure_private_ingress_with_offline_foreign_route_for_test(
                &mut app,
            );

        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);
        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("query_unsupported"),
        );
        assert!(
            response
                .headers()
                .get("x-iroha-fanout-routes-attempted")
                .is_none(),
            "the rejection must precede permission partitioning and route execution"
        );
    }

    #[tokio::test]
    async fn alias_resolve_index_rejects_multiroute_before_hidden_source_resolution() {
        let caller_keypair = checked_torii_test_ed25519_keypair(
            0x22,
            "derive alias resolve-index hidden-route caller fixture key",
        );
        let caller = AccountId::new(caller_keypair.public_key().clone());
        let target = checked_torii_test_account_id(
            0x23,
            "derive alias resolve-index hidden-route target fixture key",
        );
        let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-index-denied-fanout"));
        let mut app =
            mk_app_state_for_tests_with_world(world_with_target_and_caller_bound_to_dataspace(
                &target,
                &caller,
                uaid,
                DataSpaceId::new(10),
            ));
        configure_private_ingress_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &target, "merchant@restricted");

        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&caller, &caller_keypair, &method, &uri, &body);
        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("query_unsupported")
        );
        assert!(
            response
                .headers()
                .get("x-iroha-fanout-routes-attempted")
                .is_none(),
            "the rejection must precede permission partitioning and hidden source execution"
        );
    }

    #[test]
    fn api_token_evaluator_has_one_exact_header_policy() {
        let configured = HashSet::from(["secret".to_owned()]);
        let empty = HashSet::new();
        let no_headers = HeaderMap::new();
        let mut supplied = HeaderMap::new();
        supplied.insert(HEADER_API_TOKEN, HeaderValue::from_static("secret"));
        assert_eq!(
            evaluate_api_token(false, &empty, &no_headers),
            ApiTokenEvaluation::Disabled
        );
        assert!(
            evaluate_api_token(false, &configured, &supplied)
                .authenticated_token()
                .is_none(),
            "an unauthenticated header must not become a rate-limit principal"
        );
        assert_eq!(
            evaluate_api_token(true, &empty, &supplied),
            ApiTokenEvaluation::Unavailable
        );
        assert_eq!(
            evaluate_api_token(true, &configured, &no_headers),
            ApiTokenEvaluation::Invalid
        );

        let mut invalid_utf8 = HeaderMap::new();
        invalid_utf8.insert(
            HEADER_API_TOKEN,
            HeaderValue::from_bytes(&[0xff]).expect("opaque token fixture"),
        );
        assert_eq!(
            evaluate_api_token(true, &configured, &invalid_utf8),
            ApiTokenEvaluation::Invalid
        );

        let mut duplicate_valid = HeaderMap::new();
        duplicate_valid.append(HEADER_API_TOKEN, HeaderValue::from_static("secret"));
        duplicate_valid.append(HEADER_API_TOKEN, HeaderValue::from_static("secret"));
        assert_eq!(
            evaluate_api_token(true, &configured, &duplicate_valid),
            ApiTokenEvaluation::Invalid
        );

        let mut mixed_duplicate = HeaderMap::new();
        mixed_duplicate.append(HEADER_API_TOKEN, HeaderValue::from_static("secret"));
        mixed_duplicate.append(HEADER_API_TOKEN, HeaderValue::from_static("other"));
        assert_eq!(
            evaluate_api_token(true, &configured, &mixed_duplicate),
            ApiTokenEvaluation::Invalid
        );
        assert_eq!(
            evaluate_api_token(true, &configured, &supplied),
            ApiTokenEvaluation::Authenticated("secret")
        );
    }

    #[test]
    fn validate_api_token_rejects_missing_or_unconfigured() {
        let mut app = mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::new());

        let headers = HeaderMap::new();
        assert!(validate_api_token(state, &headers).is_err());

        let mut configured_headers = HeaderMap::new();
        configured_headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("secret"));
        let mut tokens = HashSet::new();
        tokens.insert("secret".to_string());
        state.api_tokens_set = Arc::new(tokens);
        assert!(validate_api_token(state, &configured_headers).is_ok());
    }

    fn assert_unconfigured_api_token_error(error: Error) {
        match error {
            Error::Query(ValidationFail::NotPermitted(message)) => {
                assert!(
                    message.contains("none are configured"),
                    "unexpected API-token rejection: {message}"
                );
            }
            other => panic!("unexpected API-token rejection: {other:?}"),
        }
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn direct_result_handler_fails_closed_with_no_configured_api_tokens() {
        let mut app = mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::new());

        let error =
            handler_soracloud_status(State(app), HeaderMap::new(), loopback_connect_info(), None)
                .await
                .expect_err("handler-local API-token validation must fail closed");
        assert_unconfigured_api_token_error(error);
    }

    #[tokio::test]
    async fn direct_transaction_ingress_fails_closed_before_queue_or_rate_work() {
        let mut app = mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::new());
        let keypair = checked_torii_test_ed25519_keypair(
            0xd1,
            "derive fail-closed transaction ingress fixture key",
        );
        let transaction = TransactionBuilder::new(
            *app.state.network_id_ref(),
            AccountId::new(keypair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(keypair.private_key());
        let queue_len = app.queue.active_len();

        let error = submit_signed_transaction_for_ingress_globally_synced(
            Arc::clone(&app),
            HeaderMap::new(),
            None,
            transaction,
        )
        .await
        .expect_err("direct transaction ingress must fail closed");
        assert_unconfigured_api_token_error(error);
        assert_eq!(
            app.queue.active_len(),
            queue_len,
            "authentication failure must not mutate queue state"
        );
    }
