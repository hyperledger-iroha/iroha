#[tokio::test]
async fn alias_lookup_by_account_unsigned_read_returns_only_public_aliases() {
    let authority = checked_torii_test_account_id(
        0xa8,
        "derive unsigned alias lookup filtering authority fixture key",
    );
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-warning-fanout"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        DataSpaceId::new(10),
    ));
    configure_private_ingress_routes_for_test(&mut app);
    bind_account_alias_for_test(&app, &authority, "merchant@universal");
    bind_account_alias_for_test(&app, &authority, "merchant@restricted");

    let request = routing::AliasLookupByAccountRequestDto {
        account_id: authority.to_string(),
        dataspace: None,
        domain: None,
    };
    let body = norito::json::to_vec(&request).expect("encode request");
    let response = handler_alias_lookup_by_account(
        State(app),
        axum::http::Method::POST,
        "/v1/aliases/by-account"
            .parse()
            .expect("alias by-account uri"),
        HeaderMap::new(),
        axum::body::Bytes::from(body),
    )
    .await
    .expect("unsigned lookup should return only visible public aliases")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect public alias lookup response")
        .to_bytes();
    let dto: routing::AliasLookupByAccountResponseDto =
        norito::json::from_slice(&body).expect("decode public alias lookup response");
    assert_eq!(dto.total, 1);
    assert_eq!(dto.items[0].alias, "merchant@universal");
}

#[tokio::test]
async fn alias_lookup_by_account_rejects_unsigned_restricted_alias_lookup() {
    let authority = checked_torii_test_account_id(
        0xa9,
        "derive alias lookup hidden-route authority fixture key",
    );
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-denied-fanout"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        DataSpaceId::new(10),
    ));
    configure_private_ingress_routes_for_test(&mut app);
    bind_account_alias_for_test(&app, &authority, "merchant@restricted");

    let request = routing::AliasLookupByAccountRequestDto {
        account_id: authority.to_string(),
        dataspace: Some("restricted".to_owned()),
        domain: None,
    };
    let body = norito::json::to_vec(&request).expect("encode request");
    let error = handler_alias_lookup_by_account(
        State(app),
        axum::http::Method::POST,
        "/v1/aliases/by-account"
            .parse()
            .expect("alias by-account uri"),
        HeaderMap::new(),
        axum::body::Bytes::from(body),
    )
    .await
    .expect_err("unsigned restricted alias lookup must fail closed");

    assert!(matches!(
        &error,
        Error::AppUnauthorized {
            code: "alias_auth_required",
            ..
        }
    ));
    assert_eq!(error.into_response().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn alias_lookup_by_account_rejects_invalid_auth_for_restricted_filter() {
    let authority = checked_torii_test_account_id(
        0xb0,
        "derive invalid restricted alias lookup auth fixture key",
    );
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-invalid-auth"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        DataSpaceId::new(10),
    ));
    configure_private_ingress_routes_for_test(&mut app);

    let request = routing::AliasLookupByAccountRequestDto {
        account_id: authority.to_string(),
        dataspace: Some("restricted".to_owned()),
        domain: None,
    };
    let body = norito::json::to_vec(&request).expect("encode request");
    let mut headers = HeaderMap::new();
    headers.insert(
        HEADER_ACCOUNT,
        authority.to_string().parse().expect("account header"),
    );
    let error = handler_alias_lookup_by_account(
        State(app),
        axum::http::Method::POST,
        "/v1/aliases/by-account"
            .parse()
            .expect("alias by-account uri"),
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .expect_err("incomplete canonical authentication must fail closed");

    assert!(matches!(
        &error,
        Error::AppUnauthorized {
            code: "alias_auth_invalid",
            ..
        }
    ));
    assert_eq!(error.into_response().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn alias_lookup_by_account_explicit_restricted_filter_requires_exact_resolve_permission() {
    let caller_keypair = checked_torii_test_ed25519_keypair(
        0x35,
        "derive alias lookup permission caller fixture key",
    );
    let caller = AccountId::new(caller_keypair.public_key().clone());
    let target =
        checked_torii_test_account_id(0x36, "derive alias lookup permission target fixture key");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-permission-fanout"));
    let mut app =
        mk_app_state_for_tests_with_world(world_with_target_and_caller_bound_to_dataspace(
            &target,
            &caller,
            uaid,
            DataSpaceId::new(10),
        ));
    configure_private_ingress_routes_for_test(&mut app);
    bind_account_alias_for_test(&app, &target, "merchant@restricted");

    let request = routing::AliasLookupByAccountRequestDto {
        account_id: target.to_string(),
        dataspace: Some("restricted".to_owned()),
        domain: None,
    };
    let body = norito::json::to_vec(&request).expect("encode request");
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/aliases/by-account"
        .parse()
        .expect("alias by-account uri");
    let headers = signed_app_headers(&caller, &caller_keypair, &method, &uri, &body);
    let response = handler_alias_lookup_by_account(
        State(app),
        method,
        uri,
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn account_alias_enumeration_rejects_signed_caller_without_exact_scope() {
    let caller_keypair = checked_torii_test_ed25519_keypair(
        0x39,
        "derive account alias enumeration caller fixture key",
    );
    let caller = AccountId::new(caller_keypair.public_key().clone());
    let target =
        checked_torii_test_account_id(0x3a, "derive account alias enumeration target fixture key");
    let world = World::with(
        [],
        [
            Account::new(caller.clone()).build(&caller),
            Account::new(target.clone()).build(&target),
        ],
        [],
    );
    let app = mk_app_state_for_tests_with_world(world);
    bind_account_alias_for_test(&app, &target, "merchant@universal");

    let method = Method::GET;
    let uri: axum::http::Uri = format!("/v1/accounts/{target}/aliases")
        .parse()
        .expect("account aliases uri");
    let headers = signed_app_headers(&caller, &caller_keypair, &method, &uri, &[]);
    let error = match handler_account_aliases(
        State(app),
        method,
        uri,
        headers,
        crate::loopback_connect_info(),
        AxPath(target.to_string()),
    )
    .await
    {
        Err(error) => error,
        Ok(_) => panic!("caller without exact alias scope must not enumerate bindings"),
    };

    assert!(matches!(
        error,
        Error::Query(ValidationFail::NotPermitted(message))
            if message == "exact account-alias resolve permission is required"
    ));
}

#[tokio::test]
async fn alias_lookup_by_account_filters_domain_aliases_until_exact_domain_grant() {
    let caller_keypair = checked_torii_test_ed25519_keypair(
        0x37,
        "derive alias lookup permission filter caller fixture key",
    );
    let caller = AccountId::new(caller_keypair.public_key().clone());
    let target = checked_torii_test_account_id(
        0x38,
        "derive alias lookup permission filter target fixture key",
    );
    let restricted_dataspace = DataSpaceId::new(10);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-permission-filter-fanout"));
    let mut app =
        mk_app_state_for_tests_with_world(world_with_target_and_caller_bound_to_dataspace(
            &target,
            &caller,
            uaid,
            restricted_dataspace,
        ));
    configure_private_ingress_routes_for_test(&mut app);
    bind_account_alias_for_test(&app, &target, "merchant@restricted");
    bind_account_alias_for_test(&app, &target, "merchant@bank.restricted");
    grant_alias_resolve_dataspace_permission(&app, &caller, restricted_dataspace);

    let request = routing::AliasLookupByAccountRequestDto {
        account_id: target.to_string(),
        dataspace: None,
        domain: None,
    };
    let body = norito::json::to_vec(&request).expect("encode request");
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/aliases/by-account"
        .parse()
        .expect("alias by-account uri");
    let headers = signed_app_headers(&caller, &caller_keypair, &method, &uri, &body);
    let response = handler_alias_lookup_by_account(
        State(app.clone()),
        method.clone(),
        uri.clone(),
        headers.clone(),
        axum::body::Bytes::from(body.clone()),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let dto: routing::AliasLookupByAccountResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.total, 1);
    assert_eq!(dto.items[0].alias, "merchant@restricted");

    let domain_alias = AccountAlias::from_literal(
        "merchant@bank.restricted",
        &app.state.nexus_snapshot().dataspace_catalog,
    )
    .expect("domain alias");
    grant_alias_resolve_permissions(&app, &caller, &domain_alias);
    let headers = signed_app_headers(&caller, &caller_keypair, &method, &uri, &body);
    let response = handler_alias_lookup_by_account(
        State(app),
        method,
        uri,
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .expect("handler should succeed after exact domain grant")
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let dto: routing::AliasLookupByAccountResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.total, 2);
    assert!(
        dto.items
            .iter()
            .any(|item| item.alias == "merchant@bank.restricted")
    );
}

#[tokio::test]
async fn alias_lookup_by_account_returns_empty_fanout_result_when_offline_route_has_no_reachable_aliases()
 {
    let authority_keypair = checked_torii_test_ed25519_keypair(
        0xaa,
        "derive alias lookup offline fanout authority fixture key",
    );
    let authority = AccountId::new(authority_keypair.public_key().clone());
    let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-lookup-offline"));
    let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
        &authority,
        uaid,
        DataSpaceId::new(12),
    ));
    let (_local_route, _foreign_route) =
            crate::tests_runtime_handlers::configure_private_ingress_with_offline_foreign_route_for_test(
                &mut app,
            );
    bind_account_alias_for_test(&app, &authority, "merchant@foreign-restricted");
    let alias = AccountAlias::from_literal(
        "merchant@foreign-restricted",
        &app.state.nexus_snapshot().dataspace_catalog,
    )
    .expect("foreign account alias");
    grant_alias_resolve_permissions(&app, &authority, &alias);

    let request = routing::AliasLookupByAccountRequestDto {
        account_id: authority.to_string(),
        dataspace: None,
        domain: None,
    };
    let body = norito::json::to_vec(&request).expect("encode request");
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/aliases/by-account"
        .parse()
        .expect("alias by-account uri");
    let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);
    let response = handler_alias_lookup_by_account(
        State(app),
        method,
        uri,
        headers,
        axum::body::Bytes::from(body),
    )
    .await
    .expect("handler should return a routed response")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok()),
        Some("proxy")
    );
    assert_eq!(
        response
            .headers()
            .get("x-iroha-fanout-routes-unavailable")
            .and_then(|value| value.to_str().ok()),
        Some("1")
    );
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let dto: routing::AliasLookupByAccountResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.account_id, authority.to_string());
    assert_eq!(dto.total, 0);
    assert!(dto.items.is_empty());
    assert_eq!(dto.source.as_deref(), Some("fanout"));
}

#[tokio::test]
async fn alias_resolve_rejects_account_label_without_authoritative_binding() {
    let alias = "banking@centralbank.universal";
    let alias_label = iroha_data_model::account::rekey::AccountAlias::new(
        "banking".parse::<Name>().expect("label"),
        Some(iroha_data_model::account::rekey::AccountAliasDomain::new(
            "centralbank".parse::<Name>().expect("domain id"),
        )),
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    );
    let domain_id: DomainId = DomainId::try_new("centralbank", "universal").expect("domain id");
    let authority_keypair = checked_torii_test_ed25519_keypair(
        0xab,
        "derive alias resolve account-label fallback authority fixture key",
    );
    let authority = AccountId::new(authority_keypair.public_key().clone());
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let account_id = authority.clone();
    let account = Account::new(account_id.account().clone())
        .with_label(Some(alias_label.clone()))
        .build(&authority);
    let world = World::with([domain], [authority_account, account], []);
    let app = mk_app_state_for_tests_with_world(world);
    {
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        let world = tx.world_mut_for_testing();
        world
            .account_rekey_records_mut_for_testing()
            .remove(alias_label.clone());
        world
            .account_aliases_mut_for_testing()
            .remove(alias_label.clone());
        if let Some(mut labels) = world
            .account_aliases_by_account_mut_for_testing()
            .get(&authority)
            .cloned()
        {
            labels.remove(&alias_label);
            world
                .account_aliases_by_account_mut_for_testing()
                .insert(authority.clone(), labels);
        }
        tx.apply();
        block.commit().expect("commit rekey record removal");
    }
    let request = routing::AliasResolveRequestDto {
        alias: alias.to_string(),
    };
    let body = norito::json::to_vec(&request).expect("encode request");
    let headers = signed_alias_resolve_headers_for_test(
        &app,
        &authority,
        &authority_keypair,
        &alias_label,
        &body,
    );

    let response = handler_alias_resolve(
        State(app),
        axum::http::Method::POST,
        "/v1/aliases/resolve".parse().expect("alias resolve uri"),
        headers,
        crate::loopback_connect_info(),
        axum::body::Bytes::from(body),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn alias_resolve_rejects_rekey_record_without_authoritative_binding() {
    let alias = "banking@centralbank.universal";
    let alias_label = iroha_data_model::account::rekey::AccountAlias::new(
        "banking".parse::<Name>().expect("label"),
        Some(iroha_data_model::account::rekey::AccountAliasDomain::new(
            "centralbank".parse::<Name>().expect("domain id"),
        )),
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    );
    let authority_keypair = checked_torii_test_ed25519_keypair(
        0xac,
        "derive alias resolve rekey-record fallback authority fixture key",
    );
    let authority = AccountId::new(authority_keypair.public_key().clone());
    let authority_account = Account::new(authority.clone()).build(&authority);
    let app = mk_app_state_for_tests_with_world(World::with([], [authority_account], []));
    {
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        tx.world_mut_for_testing()
            .account_rekey_records_mut_for_testing()
            .insert(
                alias_label.clone(),
                iroha_data_model::account::rekey::AccountRekeyRecord::new(
                    alias_label.clone(),
                    authority.clone(),
                ),
            );
        tx.apply();
        block.commit().expect("commit rekey record");
    }
    let request = routing::AliasResolveRequestDto {
        alias: alias.to_string(),
    };
    let body = norito::json::to_vec(&request).expect("encode request");
    let headers = signed_alias_resolve_headers_for_test(
        &app,
        &authority,
        &authority_keypair,
        &alias_label,
        &body,
    );

    let response = handler_alias_resolve(
        State(app),
        axum::http::Method::POST,
        "/v1/aliases/resolve".parse().expect("alias resolve uri"),
        headers,
        crate::loopback_connect_info(),
        axum::body::Bytes::from(body),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn contract_alias_resolve_returns_not_found_for_unknown_alias() {
    let authority_keypair = checked_torii_test_ed25519_keypair(
        0xad,
        "derive contract alias missing authority fixture key",
    );
    let authority = AccountId::new(authority_keypair.public_key().clone());
    let authority_account = Account::new(authority.clone()).build(&authority);
    let app = mk_app_state_for_tests_with_world(World::with([], [authority_account], []));
    let request = routing::ContractAliasResolveRequestDto {
        contract_alias: "router::universal".to_string(),
    };
    let body = norito::json::to_vec(&request).expect("encode contract alias request");
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/contracts/aliases/resolve"
        .parse()
        .expect("contract alias resolve URI");
    let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);

    let response = handler_contract_alias_resolve(
        State(app),
        method,
        uri,
        headers,
        crate::loopback_connect_info(),
        axum::body::Bytes::from(body),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn contract_alias_resolve_returns_bound_contract() {
    let authority_keypair = checked_torii_test_ed25519_keypair(
        0xae,
        "derive contract alias bound authority fixture key",
    );
    let authority = AccountId::new(authority_keypair.public_key().clone());
    let authority_account = Account::new(authority.clone()).build(&authority);
    let app = mk_app_state_for_tests_with_world(World::with([], [authority_account], []));
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        &iroha_data_model::ChainId::from("00000000-0000-0000-0000-000000000000"),
        &authority,
        0,
        DataSpaceId::UNIVERSAL,
    )
    .expect("contract address");
    bind_contract_alias_for_test(&app, &contract_address, "router::dex.universal");
    let request = routing::ContractAliasResolveRequestDto {
        contract_alias: "router::dex.universal".to_string(),
    };
    let body = norito::json::to_vec(&request).expect("encode contract alias request");
    let method = axum::http::Method::POST;
    let uri: axum::http::Uri = "/v1/contracts/aliases/resolve"
        .parse()
        .expect("contract alias resolve URI");
    let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);

    let response = handler_contract_alias_resolve(
        State(app),
        method,
        uri,
        headers,
        crate::loopback_connect_info(),
        axum::body::Bytes::from(body),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("body")
        .to_bytes();
    let value: Value = norito::json::from_slice(&body).expect("contract alias JSON value");
    let object = value.as_object().expect("contract alias response object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "contract_alias",
            "contract_address",
            "contract_subject_account",
            "dataspace",
            "contract_alias_binding",
            "source",
        ])
    );
    let binding = object["contract_alias_binding"]
        .as_object()
        .expect("contract alias binding object");
    assert_eq!(
        binding.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["alias", "bound_at_ms", "status"])
    );
    let dto: routing::ContractAliasResolveResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.contract_alias, "router::dex.universal");
    assert_eq!(dto.contract_address, contract_address.to_string());
    assert_eq!(
        dto.contract_subject_account,
        contract_address.subject_id().to_string()
    );
    assert_eq!(dto.dataspace, "universal");
    assert_eq!(dto.contract_alias_binding.status, "permanent");
    assert_eq!(dto.source, "world_state");
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn ram_lfe_program_policies_list_registered_program() {
    let authority =
        checked_torii_test_account_id(0xb0, "derive RAM-LFE policy-list authority fixture key");
    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer =
        checked_torii_test_ed25519_keypair(0xb1, "derive RAM-LFE policy-list signer fixture key");
    let (_policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let mut app = mk_app_state_for_tests();

    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_program_policy(&authority, &mut tx, &program_policy);
    tx.apply();
    block.commit().expect("commit block");

    let response = handler_ram_lfe_program_policies(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::RamLfeProgramPolicyListDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.total, 1);
    assert_eq!(dto.items.len(), 1);
    assert_eq!(
        dto.items[0].program_id,
        program_policy.program_id.to_string()
    );
    assert_eq!(dto.items[0].backend, "bfv-programmed-sha3-256-v1");
    assert_eq!(dto.items[0].verification_mode, "signed");
    assert!(dto.items[0].active);
    assert_eq!(dto.items[0].input_encryption.as_deref(), Some("bfv-v1"));
    assert!(dto.items[0].ram_fhe_profile.is_some());
}

#[cfg(feature = "app_api")]
#[test]
fn encrypted_only_request_dtos_reject_plaintext_fields() {
    let ram_lfe_err = norito::json::from_json::<routing::RamLfeExecuteRequestDto>(
        r#"{"encrypted_input":"00","input_hex":"00"}"#,
    )
    .expect_err("RAM-LFE execute request must not accept plaintext input_hex");
    assert!(
        ram_lfe_err
            .to_string()
            .contains("unknown field `input_hex`"),
        "unexpected RAM-LFE request error: {ram_lfe_err}"
    );

    let output_opening = String::from_utf8(
        norito::json::to_vec(&dummy_output_opening_for_access_test())
            .expect("encode dummy opening"),
    )
    .expect("opening json is utf-8");
    let identifier_body = format!(
        r#"{{"policy_id":"phone#retail","encrypted_input":"00","output_opening":{output_opening},"input":"+15551234567"}}"#
    );
    let identifier_err =
        norito::json::from_json::<routing::IdentifierResolveRequestDto>(&identifier_body)
            .expect_err("identifier request must not accept plaintext input");
    assert!(
        identifier_err.to_string().contains("unknown field `input`"),
        "unexpected identifier request error: {identifier_err}"
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn ram_lfe_execute_returns_receipt() {
    let authority =
        checked_torii_test_account_id(0xb2, "derive RAM-LFE execute authority fixture key");
    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer =
        checked_torii_test_ed25519_keypair(0xb3, "derive RAM-LFE execute signer fixture key");
    let (_policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let mut app = mk_app_state_for_tests();

    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_program_policy(&authority, &mut tx, &program_policy);
    tx.apply();
    block.commit().expect("commit block");

    let response = handler_ram_lfe_execute(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(program_policy.program_id.to_string()),
        NoritoJson(routing::RamLfeExecuteRequestDto {
            encrypted_input: encrypted_identifier_hex(
                &program_policy,
                b"identifier-input",
                b"ram-lfe-execute-route",
            ),
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let raw: norito::json::Value = norito::json::from_slice(&body).expect("raw json decode");
    assert!(
        raw.as_object()
            .expect("execute response object")
            .get("output_hex")
            .is_none()
    );
    let dto: routing::RamLfeExecuteResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.program_id, program_policy.program_id.to_string());
    assert_eq!(dto.backend, "bfv-programmed-sha3-256-v1");
    assert_eq!(dto.verification_mode, "signed");
    assert_eq!(dto.receipt.payload.program_id, dto.program_id);
    assert_eq!(dto.receipt.payload.output_hash, dto.output_hash);
    assert_eq!(
        dto.receipt.payload.associated_data_hash,
        dto.associated_data_hash
    );
    assert_eq!(dto.receipt.attestation.kind, "signed");
    assert!(dto.receipt.attestation.signature.is_some());
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn ram_lfe_receipt_verify_reports_valid_receipt_and_output_match() {
    let authority =
        checked_torii_test_account_id(0xb4, "derive RAM-LFE receipt-valid authority fixture key");
    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer =
        checked_torii_test_ed25519_keypair(0xb5, "derive RAM-LFE receipt-valid signer fixture key");
    let (_policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let mut app = mk_app_state_for_tests();

    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver.clone());

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_program_policy(&authority, &mut tx, &program_policy);
    tx.apply();
    block.commit().expect("commit block");

    let ciphertext = encrypted_identifier_ciphertext(
        &program_policy,
        b"receipt-verify-input",
        b"receipt-verify-route",
    );
    let draft = resolver
        .execute_encrypted(&program_policy, &ciphertext)
        .expect("execute program");
    let receipt = resolver
        .issue_execution_receipt(&program_policy, &draft)
        .expect("issue RAM-LFE receipt");

    let response = handler_ram_lfe_receipt_verify(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        NoritoJson(routing::RamLfeReceiptVerifyRequestDto {
            receipt,
            output_hex: Some(hex::encode(&draft.output)),
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::RamLfeReceiptVerifyResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert!(dto.valid);
    assert_eq!(dto.program_id, program_policy.program_id.to_string());
    assert_eq!(dto.backend, "bfv-programmed-sha3-256-v1");
    assert_eq!(dto.verification_mode, "signed");
    assert_eq!(dto.output_hash_matches, Some(true));
    assert!(dto.error.is_none());
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn ram_lfe_receipt_verify_rejects_expired_receipt() {
    let authority =
        checked_torii_test_account_id(0xb6, "derive RAM-LFE receipt-expired authority fixture key");
    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0xb7,
        "derive RAM-LFE receipt-expired signer fixture key",
    );
    let (_policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let mut app = mk_app_state_for_tests();

    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver.clone());

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_program_policy(&authority, &mut tx, &program_policy);
    tx.apply();
    block.commit().expect("commit block");

    let ciphertext = encrypted_identifier_ciphertext(
        &program_policy,
        b"receipt-verify-input",
        b"receipt-verify-expired-route",
    );
    let draft = resolver
        .execute_encrypted(&program_policy, &ciphertext)
        .expect("execute program");
    let mut receipt = resolver
        .issue_execution_receipt(&program_policy, &draft)
        .expect("issue RAM-LFE receipt");
    receipt.payload.executed_at_ms = 1;
    receipt.payload.expires_at_ms = Some(2);

    let response = handler_ram_lfe_receipt_verify(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        NoritoJson(routing::RamLfeReceiptVerifyRequestDto {
            receipt,
            output_hex: None,
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::RamLfeReceiptVerifyResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert!(!dto.valid);
    assert!(
        dto.error
            .as_deref()
            .expect("expiry error")
            .contains("is expired")
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_policies_lists_registered_policy() {
    let authority =
        checked_torii_test_account_id(0x10, "derive identifier policy-list authority fixture key");
    let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone())
        .with_uaid(Some(UniversalAccountId::from_hash(Hash::new(
            b"uaid-directory",
        ))))
        .build(&authority);
    let world = World::with([domain], [account], []);
    let mut app = mk_app_state_for_tests_with_world(world);

    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0x11,
        "derive identifier policy-list signer fixture key",
    );
    let (policy, program_policy) = sample_identifier_policy(&authority, &signer, &policy_id);
    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_identifier_policy_bundle(&authority, &mut tx, &policy, &program_policy);
    tx.apply();
    block.commit().expect("commit block");

    let response =
        handler_identifier_policies(State(app), HeaderMap::new(), crate::loopback_connect_info())
            .await
            .expect("handler should succeed")
            .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::IdentifierPolicyListDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.total, 1);
    assert_eq!(dto.items.len(), 1);
    assert_eq!(dto.items[0].policy_id, policy_id.to_string());
    assert!(dto.items[0].active);
    assert_eq!(dto.items[0].backend, "bfv-affine-sha3-256-v1");
    assert_eq!(dto.items[0].normalization, "phone_e164");
    assert_eq!(dto.items[0].input_encryption.as_deref(), Some("bfv-v1"));
    assert!(
        dto.items[0]
            .input_encryption_public_parameters
            .as_ref()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(dto.items[0].ram_fhe_profile.is_none());
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_policies_expose_programmed_ram_fhe_profile() {
    let authority = checked_torii_test_account_id(
        0x12,
        "derive programmed identifier policy authority fixture key",
    );
    let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone())
        .with_uaid(Some(UniversalAccountId::from_hash(Hash::new(
            b"uaid-directory-program-profile",
        ))))
        .build(&authority);
    let world = World::with([domain], [account], []);
    let mut app = mk_app_state_for_tests_with_world(world);

    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0x13,
        "derive programmed identifier policy signer fixture key",
    );
    let (policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_identifier_policy_bundle(&authority, &mut tx, &policy, &program_policy);
    tx.apply();
    block.commit().expect("commit block");

    let response =
        handler_identifier_policies(State(app), HeaderMap::new(), crate::loopback_connect_info())
            .await
            .expect("handler should succeed")
            .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::IdentifierPolicyListDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.total, 1);
    assert_eq!(dto.items[0].backend, "bfv-programmed-sha3-256-v1");
    let profile = dto.items[0]
        .ram_fhe_profile
        .clone()
        .expect("programmed policies should expose a RAM-FHE profile");
    assert_eq!(profile.profile_version, 1);
    assert_eq!(profile.register_count, 4);
    assert_eq!(profile.memory_lane_count, 32);
    assert_eq!(profile.ciphertext_mul_per_step, 16);
    assert_eq!(
        profile.encrypted_input_mode,
        iroha_crypto::BfvRamEncryptedInputMode::EncryptedEnvelopeV1
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_policies_enforce_token_policy() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        let mut tokens = HashSet::new();
        tokens.insert("token-identifier".to_owned());
        state.api_tokens_set = Arc::new(tokens);
    }

    let missing = handler_identifier_policies(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
    )
    .await;
    assert!(matches!(
        missing,
        Err(Error::Query(ValidationFail::NotPermitted(_)))
    ));

    let mut headers = HeaderMap::new();
    headers.insert("x-api-token", HeaderValue::from_static("token-identifier"));
    let response = handler_identifier_policies(State(app), headers, crate::loopback_connect_info())
        .await
        .expect("token accepted")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_resolve_returns_bound_account() {
    let authority = checked_torii_test_account_id(
        0x14,
        "derive identifier resolve bound authority fixture key",
    );
    let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-directory"));
    let account = Account::new(authority.clone())
        .with_uaid(Some(uaid))
        .build(&authority);
    let world = World::with([domain], [account], []);
    let mut app = mk_app_state_for_tests_with_world(world);

    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0x15,
        "derive identifier resolve bound signer fixture key",
    );
    let (policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver.clone());

    let encrypted_input = encrypted_identifier_ciphertext(
        &program_policy,
        b"+15551234567",
        b"identifier-resolve-bound-account",
    );
    let encrypted_input_hex =
        hex::encode(norito::to_bytes(&encrypted_input).expect("encode encrypted input"));
    let output_opening =
        output_opening_for_ciphertext(&resolver, &program_policy, &signer, &encrypted_input);
    let draft = resolver
        .derive_encrypted(
            &policy,
            &program_policy,
            &encrypted_input,
            output_opening.clone(),
        )
        .expect("derive opaque id");

    let receipt = resolver
        .issue_claim_receipt(&policy, &program_policy, &draft, uaid, authority.clone())
        .expect("claim receipt");
    let header = BlockHeader::new(
        nonzero!(1_u64),
        None,
        None,
        None,
        receipt.resolved_at_ms(),
        0,
    );
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_identifier_policy_bundle(&authority, &mut tx, &policy, &program_policy);
    ClaimIdentifier {
        account: authority.clone(),
        receipt,
    }
    .execute(&authority, &mut tx)
    .expect("claim identifier");
    tx.apply();
    block.commit().expect("commit block");

    let response = handler_identifier_resolve(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        NoritoJson(routing::IdentifierResolveRequestDto {
            policy_id: policy_id.to_string(),
            encrypted_input: encrypted_input_hex,
            output_opening,
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::IdentifierResolveResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.payload.policy_id, policy_id.to_string());
    assert_eq!(dto.payload.opaque_id, draft.opaque_id.to_string());
    assert_eq!(dto.payload.receipt_hash, draft.receipt_hash.to_string());
    assert_eq!(dto.payload.uaid, uaid.to_string());
    assert_eq!(dto.payload.account_id, authority.to_string());
    assert_eq!(dto.payload.execution.backend, "bfv-programmed-sha3-256-v1");
    assert!(
        !dto.attestation
            .signature
            .as_deref()
            .unwrap_or_default()
            .is_empty(),
        "resolve responses should carry a signed receipt"
    );
    assert_eq!(dto.attestation.kind, "signed");
    assert_eq!(dto.payload.policy_id, policy_id.to_string());
    assert_eq!(dto.payload.opaque_id, draft.opaque_id.to_string());
    assert_eq!(dto.payload.receipt_hash, draft.receipt_hash.to_string());
    assert_eq!(dto.payload.uaid, uaid.to_string());
    assert_eq!(dto.payload.account_id, authority.to_string());
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_resolve_returns_bound_account_with_programmed_backend() {
    let authority = checked_torii_test_account_id(
        0x16,
        "derive identifier resolve programmed authority fixture key",
    );
    let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-directory-programmed"));
    let account = Account::new(authority.clone())
        .with_uaid(Some(uaid))
        .build(&authority);
    let world = World::with([domain], [account], []);
    let mut app = mk_app_state_for_tests_with_world(world);

    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0x17,
        "derive identifier resolve programmed signer fixture key",
    );
    let (policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver.clone());

    let encrypted_input = encrypted_identifier_ciphertext(
        &program_policy,
        b"+15551234567",
        b"identifier-resolve-programmed",
    );
    let encrypted_input_hex =
        hex::encode(norito::to_bytes(&encrypted_input).expect("encode encrypted input"));
    let output_opening =
        output_opening_for_ciphertext(&resolver, &program_policy, &signer, &encrypted_input);
    let draft = resolver
        .derive_encrypted(
            &policy,
            &program_policy,
            &encrypted_input,
            output_opening.clone(),
        )
        .expect("derive opaque id");

    let receipt = resolver
        .issue_claim_receipt(&policy, &program_policy, &draft, uaid, authority.clone())
        .expect("claim receipt");
    let header = BlockHeader::new(
        nonzero!(1_u64),
        None,
        None,
        None,
        receipt.resolved_at_ms(),
        0,
    );
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_identifier_policy_bundle(&authority, &mut tx, &policy, &program_policy);
    ClaimIdentifier {
        account: authority.clone(),
        receipt,
    }
    .execute(&authority, &mut tx)
    .expect("claim identifier");
    tx.apply();
    block.commit().expect("commit block");

    let response = handler_identifier_resolve(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        NoritoJson(routing::IdentifierResolveRequestDto {
            policy_id: policy_id.to_string(),
            encrypted_input: encrypted_input_hex,
            output_opening,
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::IdentifierResolveResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.payload.policy_id, policy_id.to_string());
    assert_eq!(dto.payload.opaque_id, draft.opaque_id.to_string());
    assert_eq!(dto.payload.receipt_hash, draft.receipt_hash.to_string());
    assert_eq!(dto.payload.uaid, uaid.to_string());
    assert_eq!(dto.payload.account_id, authority.to_string());
    assert_eq!(dto.payload.execution.backend, "bfv-programmed-sha3-256-v1");
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_resolve_accepts_bfv_encrypted_input() {
    let authority =
        checked_torii_test_account_id(0x18, "derive identifier resolve BFV authority fixture key");
    let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-directory-encrypted"));
    let account = Account::new(authority.clone())
        .with_uaid(Some(uaid))
        .build(&authority);
    let world = World::with([Domain::new(domain_id).build(&authority)], [account], []);
    let mut app = mk_app_state_for_tests_with_world(world);

    let policy_id: IdentifierPolicyId = "string#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0x19,
        "derive identifier resolve BFV signer fixture key",
    );
    let public_parameters = shared_sdk_identifier_bfv_public_parameters(&policy_id);
    let (policy, program_policy) = sample_identifier_policy_with_public_parameters(
        &authority,
        &signer,
        &policy_id,
        IdentifierNormalization::Exact,
        &public_parameters,
    );
    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver.clone());

    let input = "ab";
    let encrypted_input_ciphertext = encrypt_identifier_from_seed(
        &public_parameters,
        input.as_bytes(),
        b"identifier-route-bfv-ciphertext",
    )
    .expect("encrypt BFV identifier input");
    let encrypted_input =
        hex::encode(norito::to_bytes(&encrypted_input_ciphertext).expect("encode BFV input"));
    let output_opening = output_opening_for_ciphertext(
        &resolver,
        &program_policy,
        &signer,
        &encrypted_input_ciphertext,
    );
    let draft = resolver
        .derive_encrypted(
            &policy,
            &program_policy,
            &encrypted_input_ciphertext,
            output_opening.clone(),
        )
        .expect("derive opaque id");
    let receipt = resolver
        .issue_claim_receipt(&policy, &program_policy, &draft, uaid, authority.clone())
        .expect("claim receipt");
    let header = BlockHeader::new(
        nonzero!(1_u64),
        None,
        None,
        None,
        receipt.resolved_at_ms(),
        0,
    );
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_identifier_policy_bundle(&authority, &mut tx, &policy, &program_policy);
    ClaimIdentifier {
        account: authority.clone(),
        receipt,
    }
    .execute(&authority, &mut tx)
    .expect("claim identifier");
    tx.apply();
    block.commit().expect("commit block");

    let response = handler_identifier_resolve(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        NoritoJson(routing::IdentifierResolveRequestDto {
            policy_id: policy_id.to_string(),
            encrypted_input,
            output_opening,
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::IdentifierResolveResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.payload.policy_id, policy_id.to_string());
    assert_eq!(dto.payload.opaque_id, draft.opaque_id.to_string());
    assert_eq!(dto.payload.receipt_hash, draft.receipt_hash.to_string());
    assert_eq!(dto.payload.uaid, uaid.to_string());
    assert_eq!(dto.payload.account_id, authority.to_string());
    assert_eq!(dto.payload.opaque_id, draft.opaque_id.to_string());
    assert_eq!(dto.payload.receipt_hash, draft.receipt_hash.to_string());
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_resolve_rejects_malformed_bfv_without_panicking() {
    let authority = checked_torii_test_account_id(
        0x1a,
        "derive malformed identifier BFV authority fixture key",
    );
    let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
    let world = World::with(
        [Domain::new(domain_id).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [],
    );
    let mut app = mk_app_state_for_tests_with_world(world);

    let policy_id: IdentifierPolicyId = "string#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0x1b,
        "derive malformed identifier BFV signer fixture key",
    );
    let public_parameters = shared_sdk_identifier_bfv_public_parameters(&policy_id);
    let (policy, program_policy) = sample_identifier_policy_with_public_parameters(
        &authority,
        &signer,
        &policy_id,
        IdentifierNormalization::Exact,
        &public_parameters,
    );
    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver);

    {
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        register_and_activate_identifier_policy_bundle(
            &authority,
            &mut tx,
            &policy,
            &program_policy,
        );
        tx.apply();
        block.commit().expect("commit block");
    }

    let mut malformed = norito::to_bytes(
        &encrypt_identifier_from_seed(
            &public_parameters,
            b"ab",
            b"identifier-route-bfv-malformed-ciphertext",
        )
        .expect("encrypt BFV identifier input"),
    )
    .expect("encode BFV identifier ciphertext");
    let payload_start = norito::core::Header::SIZE;
    assert!(malformed.len() > payload_start + 1);
    let mut payload = malformed[payload_start..].to_vec();
    payload.pop();
    let payload_len = u64::try_from(payload.len())
        .expect("payload length fits u64")
        .to_le_bytes();
    let checksum = norito::hardware_crc64(&payload).to_le_bytes();
    malformed.truncate(payload_start);
    malformed[23..31].copy_from_slice(&payload_len);
    malformed[31..39].copy_from_slice(&checksum);
    malformed.extend_from_slice(&payload);

    let err = handler_identifier_resolve(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        NoritoJson(routing::IdentifierResolveRequestDto {
            policy_id: policy_id.to_string(),
            encrypted_input: hex::encode(malformed),
            output_opening: dummy_output_opening_for_access_test(),
        }),
    )
    .await
    .expect_err("malformed ciphertext should be rejected");

    let message = match &err {
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
        )) => message.as_str(),
        other => panic!("expected conversion error, got {other:?}"),
    };
    assert!(
        message.contains("encrypted identifier ciphertext is not valid Norito BFV data"),
        "unexpected conversion message: {message}"
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_resolve_enforces_token_policy() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        let mut tokens = HashSet::new();
        tokens.insert("token-resolve".to_owned());
        state.api_tokens_set = Arc::new(tokens);
    }

    let missing = handler_identifier_resolve(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        NoritoJson(routing::IdentifierResolveRequestDto {
            policy_id: "phone#retail".to_owned(),
            encrypted_input: String::new(),
            output_opening: dummy_output_opening_for_access_test(),
        }),
    )
    .await;
    assert!(matches!(
        missing,
        Err(Error::Query(ValidationFail::NotPermitted(_)))
    ));
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_claim_receipt_normalizes_phone_input() {
    let authority = checked_torii_test_account_id(
        0x1c,
        "derive identifier claim receipt authority fixture key",
    );
    let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-directory-claim"));
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone())
        .with_uaid(Some(uaid))
        .build(&authority);
    let world = World::with([domain], [account], []);
    let mut app = mk_app_state_for_tests_with_world(world);

    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0x1d,
        "derive identifier claim receipt signer fixture key",
    );
    let (policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver.clone());

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_identifier_policy_bundle(&authority, &mut tx, &policy, &program_policy);
    tx.apply();
    block.commit().expect("commit block");

    let encrypted_input = encrypted_identifier_ciphertext(
        &program_policy,
        b"+15551234567",
        b"identifier-claim-receipt",
    );
    let encrypted_input_hex =
        hex::encode(norito::to_bytes(&encrypted_input).expect("encode encrypted input"));
    let output_opening =
        output_opening_for_ciphertext(&resolver, &program_policy, &signer, &encrypted_input);
    let response = handler_identifier_claim_receipt(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(authority.to_string()),
        NoritoJson(routing::IdentifierResolveRequestDto {
            policy_id: policy_id.to_string(),
            encrypted_input: encrypted_input_hex,
            output_opening: output_opening.clone(),
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::IdentifierResolveResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    let expected_draft = resolver
        .derive_encrypted(&policy, &program_policy, &encrypted_input, output_opening)
        .expect("normalized derive");
    assert_eq!(dto.payload.opaque_id, expected_draft.opaque_id.to_string());
    assert_eq!(
        dto.payload.receipt_hash,
        expected_draft.receipt_hash.to_string()
    );
    assert_eq!(dto.payload.account_id, authority.to_string());
    assert_eq!(dto.payload.uaid, uaid.to_string());
    assert_eq!(dto.payload.uaid, uaid.to_string());
    assert_eq!(dto.payload.account_id, authority.to_string());
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_receipt_lookup_returns_persisted_claim() {
    let authority = checked_torii_test_account_id(
        0x1e,
        "derive identifier receipt lookup authority fixture key",
    );
    let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-directory-receipt-lookup"));
    let account = Account::new(authority.clone())
        .with_uaid(Some(uaid))
        .build(&authority);
    let world = World::with([Domain::new(domain_id).build(&authority)], [account], []);
    let mut app = mk_app_state_for_tests_with_world(world);

    let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
    let signer = checked_torii_test_ed25519_keypair(
        0x1f,
        "derive identifier receipt lookup signer fixture key",
    );
    let (policy, program_policy) =
        sample_programmed_identifier_policy(&authority, &signer, &policy_id);
    let resolver = Arc::new(identifier_resolution::IdentifierResolutionService::new());
    resolver.register_program_runtime(
        program_policy.program_id.clone(),
        iroha_crypto::RamLfeSecret::try_from(b"resolver-secret".to_vec())
            .expect("valid RAM-LFE test secret"),
        default_bfv_programmed_hidden_program(),
        signer.clone(),
        Some(30_000),
    );
    Arc::get_mut(&mut app)
        .expect("unique app")
        .identifier_resolver = Some(resolver.clone());

    let encrypted_input = encrypted_identifier_ciphertext(
        &program_policy,
        b"+15551234567",
        b"identifier-receipt-lookup",
    );
    let output_opening =
        output_opening_for_ciphertext(&resolver, &program_policy, &signer, &encrypted_input);
    let draft = resolver
        .derive_encrypted(&policy, &program_policy, &encrypted_input, output_opening)
        .expect("derive opaque id");
    let receipt = resolver
        .issue_claim_receipt(&policy, &program_policy, &draft, uaid, authority.clone())
        .expect("claim receipt");
    let receipt_hash = receipt.payload.receipt_hash.to_string();
    let header = BlockHeader::new(
        nonzero!(1_u64),
        None,
        None,
        None,
        receipt.resolved_at_ms(),
        0,
    );
    let mut block = app.state.block(header);
    let mut tx = block.transaction();
    register_and_activate_identifier_policy_bundle(&authority, &mut tx, &policy, &program_policy);
    ClaimIdentifier {
        account: authority.clone(),
        receipt,
    }
    .execute(&authority, &mut tx)
    .expect("claim identifier");
    tx.apply();
    block.commit().expect("commit block");

    let response = handler_identifier_receipt_lookup(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(receipt_hash),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::IdentifierClaimLookupResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.policy_id, policy_id.to_string());
    assert_eq!(dto.opaque_id, draft.opaque_id.to_string());
    assert_eq!(dto.receipt_hash, draft.receipt_hash.to_string());
    assert_eq!(dto.uaid, uaid.to_string());
    assert_eq!(dto.account_id, authority.to_string());
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn identifier_claim_receipt_enforces_token_policy() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        let mut tokens = HashSet::new();
        tokens.insert("token-claim".to_owned());
        state.api_tokens_set = Arc::new(tokens);
    }

    let missing = handler_identifier_claim_receipt(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath("ed0120deadbeef".to_owned()),
        NoritoJson(routing::IdentifierResolveRequestDto {
            policy_id: "phone#retail".to_owned(),
            encrypted_input: String::new(),
            output_opening: dummy_output_opening_for_access_test(),
        }),
    )
    .await;
    assert!(matches!(
        missing,
        Err(Error::Query(ValidationFail::NotPermitted(_)))
    ));
}

#[tokio::test]
async fn asset_alias_resolve_returns_definition_fields() {
    let authority =
        checked_torii_test_account_id(0x01, "derive asset alias definition fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("usd").expect("asset name token"),
    );
    let alias: AssetDefinitionAlias = "usd#issuer.main".parse().expect("asset alias");
    let definition = iroha_data_model::asset::AssetDefinition::numeric(definition_id.clone(), "usd".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
        .build(&authority);
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], [definition]);
    let app = mk_app_state_for_tests_with_world(world);
    bind_asset_alias_for_test(&app, &authority, &definition_id, &alias, None, 1, 0);

    let response = handler_asset_alias_resolve(
        State(app),
        NoritoJson(routing::AssetAliasResolveRequestDto {
            alias: alias.to_string(),
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::AssetAliasResolveResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.alias, "usd#issuer.main");
    assert!(!dto.asset_definition_id.contains(':'));
    assert_eq!(
        dto.asset_definition_id
            .parse::<AssetDefinitionId>()
            .expect("base58 literal must parse"),
        definition_id
    );
    assert_eq!(dto.asset_name, "usd");
    let alias_binding = dto.alias_binding.expect("alias binding metadata");
    assert_eq!(alias_binding.alias, "usd#issuer.main");
    assert_eq!(alias_binding.status, "permanent");
    assert_eq!(alias_binding.lease_expiry_ms, None);
    assert_eq!(alias_binding.grace_until_ms, None);
    assert_eq!(dto.source.as_deref(), Some("world_state"));
}

#[tokio::test]
async fn asset_alias_resolve_accepts_short_form_alias() {
    let authority = checked_torii_test_account_id(0x02, "derive short asset alias fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("usd").expect("asset name token"),
    );
    let alias: AssetDefinitionAlias = "usd#main".parse().expect("asset alias");
    let definition = iroha_data_model::asset::AssetDefinition::numeric(definition_id.clone(), "usd".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
        .build(&authority);
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], [definition]);
    let app = mk_app_state_for_tests_with_world(world);
    bind_asset_alias_for_test(&app, &authority, &definition_id, &alias, None, 1, 0);

    let response = handler_asset_alias_resolve(
        State(app),
        NoritoJson(routing::AssetAliasResolveRequestDto {
            alias: alias.to_string(),
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: routing::AssetAliasResolveResponseDto =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(dto.alias, "usd#main");
    assert!(!dto.asset_definition_id.contains(':'));
    assert_eq!(
        dto.asset_definition_id
            .parse::<AssetDefinitionId>()
            .expect("base58 literal must parse"),
        definition_id
    );
    assert_eq!(dto.asset_name, "usd");
    let alias_binding = dto.alias_binding.expect("alias binding metadata");
    assert_eq!(alias_binding.alias, "usd#main");
    assert_eq!(alias_binding.status, "permanent");
    assert_eq!(dto.source.as_deref(), Some("world_state"));
}

#[tokio::test]
async fn asset_definition_get_returns_full_definition_by_base58_id() {
    let authority = checked_torii_test_account_id(0x03, "derive asset definition get fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("usd").expect("asset name token"),
    );
    let alias: AssetDefinitionAlias = "usd#issuer.main".parse().expect("asset alias");
    let definition = iroha_data_model::asset::AssetDefinition::numeric(definition_id.clone(), "usd".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
        .with_description(Some("Treasury settlement token".to_owned()))
        .build(&authority);
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], [definition.clone()]);
    let app = mk_app_state_for_tests_with_world(world);
    bind_asset_alias_for_test(&app, &authority, &definition_id, &alias, None, 1, 0);

    let response = handler_asset_definition_get(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(definition_id.to_string()),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let returned: norito::json::Value = norito::json::from_slice(&body).expect("json decode");
    let definition_id_literal = definition.id().to_string();
    assert_eq!(
        returned["id"].as_str(),
        Some(definition_id_literal.as_str())
    );
    assert_eq!(returned["name"].as_str(), Some(definition.name().as_str()));
    assert_eq!(returned["alias"].as_str(), Some(alias.as_ref()));
    assert_eq!(
        returned["description"].as_str(),
        definition.description().as_deref()
    );
    assert_eq!(
        returned["alias_binding"]["alias"].as_str(),
        Some(alias.as_ref())
    );
    assert_eq!(
        returned["alias_binding"]["status"].as_str(),
        Some("permanent")
    );
}

#[tokio::test]
async fn asset_alias_resolve_returns_not_found_after_grace() {
    let authority = checked_torii_test_account_id(0x04, "derive expired asset alias fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("usd").expect("asset name token"),
    );
    let alias: AssetDefinitionAlias = "usd#issuer.main".parse().expect("asset alias");
    let definition = iroha_data_model::asset::AssetDefinition::numeric(definition_id.clone(), "usd".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
        .build(&authority);
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], [definition]);
    let app = mk_app_state_for_tests_with_world(world);
    bind_asset_alias_for_test(
        &app,
        &authority,
        &definition_id,
        &alias,
        Some(2_000),
        1,
        1_000,
    );

    let after_grace = 2_000_u64 + 369_u64 * 60 * 60 * 1_000 + 1;
    record_latest_committed_header_for_test(&app, 2, after_grace);

    let response = handler_asset_alias_resolve(
        State(app),
        NoritoJson(routing::AssetAliasResolveRequestDto {
            alias: alias.to_string(),
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn asset_definition_get_reports_expired_pending_cleanup_status_after_grace() {
    let authority =
        checked_torii_test_account_id(0x05, "derive expired asset definition get fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("usd").expect("asset name token"),
    );
    let alias: AssetDefinitionAlias = "usd#issuer.main".parse().expect("asset alias");
    let definition = iroha_data_model::asset::AssetDefinition::numeric(definition_id.clone(), "usd".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
        .build(&authority);
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], [definition]);
    let app = mk_app_state_for_tests_with_world(world);
    bind_asset_alias_for_test(
        &app,
        &authority,
        &definition_id,
        &alias,
        Some(2_000),
        1,
        1_000,
    );

    let after_grace = 2_000_u64 + 369_u64 * 60 * 60 * 1_000 + 1;
    record_latest_committed_header_for_test(&app, 2, after_grace);

    let response = handler_asset_definition_get(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        AxPath(definition_id.to_string()),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect body")
        .to_bytes();
    let dto: norito::json::Value = norito::json::from_slice(&body).expect("json decode");
    assert_eq!(
        dto["alias_binding"]["status"].as_str(),
        Some("expired_pending_cleanup")
    );
    assert_eq!(dto["alias"].as_str(), Some(alias.as_ref()));
}

#[tokio::test]
async fn parse_asset_definition_id_rejects_alias_after_grace() {
    let authority =
        checked_torii_test_account_id(0x06, "derive parse expired asset alias fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("usd").expect("asset name token"),
    );
    let alias: AssetDefinitionAlias = "usd#issuer.main".parse().expect("asset alias");
    let definition = iroha_data_model::asset::AssetDefinition::numeric(definition_id.clone(), "usd".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
        .build(&authority);
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], [definition]);
    let app = mk_app_state_for_tests_with_world(world);
    bind_asset_alias_for_test(
        &app,
        &authority,
        &definition_id,
        &alias,
        Some(2_000),
        1,
        1_000,
    );

    let after_grace = 2_000_u64 + 369_u64 * 60 * 60 * 1_000 + 1;
    record_latest_committed_header_for_test(&app, 2, after_grace);

    let error = parse_asset_definition_id(app.as_ref(), alias.as_ref())
        .expect_err("expired alias must stop resolving");
    assert!(matches!(
        error,
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound
        ))
    ));
}

#[tokio::test]
async fn parse_asset_definition_id_accepts_base58_and_alias_literals() {
    let authority = checked_torii_test_account_id(0x07, "derive parse asset alias fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let long_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("cbdc").expect("asset name token"),
    );
    let short_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("usd").expect("asset name token"),
    );
    let long_definition = iroha_data_model::asset::AssetDefinition::numeric(long_id.clone(), "cbdc".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
        .build(&authority);
    let short_definition =
        iroha_data_model::asset::AssetDefinition::numeric(short_id.clone(), "usd".to_owned(), iroha_data_model::asset::AssetBalancePolicy::Global, None)
            .build(&authority);
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with([domain], [account], [long_definition, short_definition]);
    let app = mk_app_state_for_tests_with_world(world);
    bind_asset_alias_for_test(
        &app,
        &authority,
        &long_id,
        &"cbdc#bankb.dataspace".parse().expect("alias"),
        None,
        1,
        0,
    );
    bind_asset_alias_for_test(
        &app,
        &authority,
        &short_id,
        &"usd#centralbank".parse().expect("alias"),
        None,
        2,
        0,
    );

    assert_eq!(
        parse_asset_definition_id(app.as_ref(), "cbdc#bankb.dataspace")
            .expect("long alias should resolve"),
        long_id
    );
    assert_eq!(
        parse_asset_definition_id(app.as_ref(), "usd#centralbank")
            .expect("short alias should resolve"),
        short_id
    );
    assert_eq!(
        parse_asset_definition_id(app.as_ref(), &long_id.to_string())
            .expect("base58 id should resolve"),
        long_id
    );
    let prefixed_error =
        parse_asset_definition_id(app.as_ref(), "prefix:2f17c72466f84a4bb8a8e24884fdcd2f")
            .expect_err("prefixed literal must be rejected");
    assert!(matches!(
        prefixed_error,
        Error::Query(ValidationFail::TooComplex)
    ));

    let missing_error = parse_asset_definition_id(app.as_ref(), "cbdc#missing")
        .expect_err("unknown alias should be rejected");
    assert!(
        matches!(
            missing_error,
            Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound
            ))
        ),
        "unexpected error: {missing_error:?}"
    );
}

#[tokio::test]
async fn resolve_tx_history_allowed_asset_definition_id_accepts_base58_literal_without_local_definition()
 {
    let authority =
        checked_torii_test_account_id(0x08, "derive tx-history base58 asset fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let expected = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        Name::from_str("cbdc").expect("asset name token"),
    );
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let mut app = mk_app_state_for_tests_with_world(World::with([domain], [account], []));
    let app_state = Arc::get_mut(&mut app).expect("unique app state");
    app_state.tx_history_access_policy = Arc::new(TxHistoryAccessPolicy {
        allowed_asset_definition_id: Some(expected.to_string()),
        ..TxHistoryAccessPolicy::default()
    });

    assert_eq!(
        resolve_tx_history_allowed_asset_definition_id(app.as_ref())
            .expect("base58 selector should not require local definition"),
        Some(expected)
    );
}

#[tokio::test]
async fn resolve_tx_history_allowed_asset_definition_id_keeps_alias_selectors_strict() {
    let authority =
        checked_torii_test_account_id(0x09, "derive tx-history strict alias fixture key");
    let domain_id: DomainId = DomainId::try_new("issuer", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let mut app = mk_app_state_for_tests_with_world(World::with([domain], [account], []));
    let app_state = Arc::get_mut(&mut app).expect("unique app state");
    app_state.tx_history_access_policy = Arc::new(TxHistoryAccessPolicy {
        allowed_asset_definition_id: Some("cbdc#missing".to_owned()),
        ..TxHistoryAccessPolicy::default()
    });

    let error = resolve_tx_history_allowed_asset_definition_id(app.as_ref())
        .expect_err("unknown alias selector must remain strict");
    assert!(matches!(
        error,
        Error::Query(ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound
        ))
    ));
}

#[tokio::test]
async fn asset_alias_resolve_returns_not_found_for_unknown_alias() {
    let app = mk_app_state_for_tests();
    let response = handler_asset_alias_resolve(
        State(app),
        NoritoJson(routing::AssetAliasResolveRequestDto {
            alias: "usd#issuer.main".to_owned(),
        }),
    )
    .await
    .expect("handler should succeed")
    .into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn torii_norito_body_decodes_successful_responses() {
    let record = ProofRecord {
        id: ProofId {
            backend: "debug-proof".to_owned(),
            proof_hash: [0xAA; 32],
        },
        vk_ref: None,
        vk_commitment: None,
        status: ProofStatus::Verified,
        verified_at_height: Some(7),
        bridge: None,
    };
    let response = (StatusCode::OK, utils::NoritoBody(record.clone())).into_response();

    let decoded = super::torii_norito_body::<ProofRecord>(response, "proof record response")
        .await
        .expect("norito body should decode");

    assert_eq!(decoded, record);
}

#[tokio::test]
async fn resolve_torii_proof_record_for_routes_fanouts_matching_records() {
    let mut app = mk_app_state_for_tests();
    crate::tests_runtime_handlers::configure_private_ingress_routes_for_test(&mut app);
    let id = seed_proof_record(&app, "debug-proof", [0xBC; 32]);
    let routes = super::torii_all_dataspace_routes(app.as_ref());

    let (record, diagnostics, routed_by) =
        super::resolve_torii_proof_record_for_routes(&app, routes, id.clone())
            .await
            .expect("proof record fanout should resolve");

    assert_eq!(record.id.to_string(), id);
    assert_eq!(diagnostics.attempted_routes, 3);
    assert_eq!(diagnostics.succeeded_routes, 3);
    assert_eq!(routed_by, "local");
}

#[tokio::test]
async fn resolve_torii_proof_record_for_routes_prefers_not_found_over_route_unavailable_when_missing()
 {
    let mut app = mk_app_state_for_tests();
    let (local_route, foreign_route) =
            crate::tests_runtime_handlers::configure_private_ingress_with_offline_foreign_route_for_test(&mut app);
    let missing_id = ProofId {
        backend: "stark/fri/sha256-goldilocks-v1".to_owned(),
        proof_hash: [0x44; 32],
    }
    .to_string();

    let response = super::resolve_torii_proof_record_for_routes(
        &app,
        vec![foreign_route, local_route],
        missing_id,
    )
    .await
    .expect_err("missing proof record should return an error response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_ne!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("route_unavailable"),
        "a definitive missing-proof response should outrank an unrelated unavailable route",
    );
}

#[tokio::test]
async fn resolve_torii_proof_record_for_routes_returns_route_unavailable_when_only_unavailable() {
    let mut app = mk_app_state_for_tests();
    let (_local_route, foreign_route) =
            crate::tests_runtime_handlers::configure_private_ingress_with_offline_foreign_route_for_test(&mut app);
    let missing_id = ProofId {
        backend: "stark/fri/sha256-goldilocks-v1".to_owned(),
        proof_hash: [0x55; 32],
    }
    .to_string();

    let response =
        super::resolve_torii_proof_record_for_routes(&app, vec![foreign_route], missing_id)
            .await
            .expect_err("offline authoritative route should be unavailable");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("route_unavailable")
    );
}

#[tokio::test]
async fn proof_record_get_advertises_cache_and_304() {
    let app = mk_app_state_for_tests();
    let id = seed_proof_record(&app, "debug-proof", [0xAB; 32]);

    let first = handler_proof_record_get(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        axum::extract::Path(id.clone()),
    )
    .await
    .expect("proof record ok")
    .into_response();
    assert_eq!(first.status(), StatusCode::OK);
    let etag = first
        .headers()
        .get(axum::http::header::ETAG)
        .cloned()
        .expect("etag header");
    let cache_control = first
        .headers()
        .get(axum::http::header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        cache_control.contains("max-age"),
        "cache header should be present"
    );
    let body = http_body_util::BodyExt::collect(first.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert!(!body.is_empty(), "first response includes body");
    let record = norito::decode_from_bytes::<ProofRecord>(&body).expect("proof record body");
    assert_eq!(record.id.to_string(), id);

    let mut conditional_headers = HeaderMap::new();
    conditional_headers.insert(axum::http::header::IF_NONE_MATCH, etag);
    let not_modified = handler_proof_record_get(
        State(app.clone()),
        conditional_headers,
        crate::loopback_connect_info(),
        axum::extract::Path(id),
    )
    .await
    .expect("conditional proof ok")
    .into_response();
    assert_eq!(not_modified.status(), StatusCode::NOT_MODIFIED);
    let empty = http_body_util::BodyExt::collect(not_modified.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert!(empty.is_empty(), "304 responses have no body");
}

#[tokio::test]
async fn proof_record_get_reports_fanout_headers_when_dataspaces_are_configured() {
    let mut app = mk_app_state_for_tests();
    crate::tests_runtime_handlers::configure_private_ingress_routes_for_test(&mut app);
    let id = seed_proof_record(&app, "debug-proof", [0xCD; 32]);

    let response = handler_proof_record_get(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        axum::extract::Path(id.clone()),
    )
    .await
    .expect("proof record ok")
    .into_response();

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
            .get("x-iroha-fanout-routes-attempted")
            .and_then(|value| value.to_str().ok()),
        Some("3")
    );
    assert_eq!(
        response
            .headers()
            .get("x-iroha-fanout-routes-succeeded")
            .and_then(|value| value.to_str().ok()),
        Some("3")
    );

    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let record = norito::decode_from_bytes::<ProofRecord>(&body).expect("proof record body");
    assert_eq!(record.id.to_string(), id);
}

#[tokio::test]
async fn proof_record_get_returns_not_found_when_all_routes_miss() {
    let mut app = mk_app_state_for_tests();
    crate::tests_runtime_handlers::configure_private_ingress_routes_for_test(&mut app);
    let missing_id = ProofId {
        backend: "stark/fri/sha256-goldilocks-v1".to_owned(),
        proof_hash: [0x73; 32],
    }
    .to_string();

    let response = handler_proof_record_get(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        axum::extract::Path(missing_id),
    )
    .await
    .expect("proof handler should return a response")
    .into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let envelope: ErrorEnvelope = norito::decode_from_bytes(&body).expect("error envelope payload");
    assert_eq!(envelope.code, "proof_record_not_found");
}

#[tokio::test]
async fn proof_retention_status_reports_counts() {
    let mut app = mk_app_state_for_tests();
    let cap = iroha_config::parameters::defaults::zk::proof::RECORD_HISTORY_CAP;
    let grace = iroha_config::parameters::defaults::zk::proof::RETENTION_GRACE_BLOCKS;
    let prune_batch = iroha_config::parameters::defaults::zk::proof::PRUNE_BATCH_SIZE;
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique core state");
        state.zk.proof_history_cap = cap;
        state.zk.proof_retention_grace_blocks = grace;
        state.zk.proof_prune_batch = prune_batch;
    }
    // Seed one record outside the grace window, one on the boundary, and one fresh record.
    let current_height = grace + 5;
    let stale_height = current_height.saturating_sub(grace + 1);
    let boundary_height = current_height.saturating_sub(grace);
    let fresh_height = current_height;
    {
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        let mut insert_record = |proof_hash: [u8; 32], verified_at_height: u64| -> ProofId {
            let id = ProofId {
                backend: "debug-proof".to_string(),
                proof_hash,
            };
            let rec = ProofRecord {
                id: id.clone(),
                vk_ref: None,
                vk_commitment: None,
                status: ProofStatus::Verified,
                verified_at_height: Some(verified_at_height),
                bridge: None,
            };
            stx.world.proofs_mut_for_testing().insert(id.clone(), rec);
            id
        };

        let _ = insert_record([0xCC; 32], stale_height);
        let _ = insert_record([0xDD; 32], boundary_height);
        let _ = insert_record([0xEE; 32], fresh_height);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(1).expect("block count should be non-zero"),
        );
        block
            .commit()
            .expect("seed proof block commit should succeed");
    }
    // Advance the latest block past the grace window so the stale record becomes prunable.
    set_latest_block_height(&app, current_height);
    assert_eq!(
        app.state.view().world().proofs().len(),
        3,
        "expected three proof records in the fixture"
    );

    let response = handler_proof_retention_status(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("retention status ok")
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);

    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let status: iroha_torii_shared::ProofRetentionStatus =
        norito::json::from_slice(&body).expect("decode retention status");
    let backend = status
        .backends
        .iter()
        .find(|entry| entry.backend == "debug-proof")
        .expect("backend present");
    assert_eq!(status.cap_per_backend, cap);
    assert_eq!(status.grace_blocks, grace);
    assert_eq!(status.prune_batch, prune_batch);
    assert_eq!(status.total_records, 3);
    // With defaults, the grace window is large enough that none of the seeded proofs
    // are prunable yet; this ensures the endpoint mirrors policy rather than fixed numbers.
    assert_eq!(status.total_prunable, 0);
    assert_eq!(backend.records, 3);
    assert_eq!(backend.prunable, 0);
    assert_eq!(backend.oldest_height, Some(stale_height));
    assert_eq!(backend.newest_height, Some(fresh_height));
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn axt_proof_cache_debug_reports_snapshot() {
    let mut app = mk_app_state_for_tests();
    let dsid = DataSpaceId::new(9);
    let manifest_root = [0xAA; 32];
    let policy_entries = vec![iroha_data_model::nexus::AxtPolicyBinding {
        dsid,
        policy: iroha_data_model::nexus::AxtPolicyEntry {
            manifest_root,
            target_lane: LaneId::new(2),
            min_handle_era: 10,
            min_sub_nonce: 11,
            current_slot: 5,
        },
    }];
    let policy_version = AxtPolicySnapshot::compute_version(&policy_entries);
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique core state");
        state
            .telemetry
            .set_axt_proof_cache_state(dsid, "miss", manifest_root, 5, Some(10));
        state.telemetry.note_axt_policy_reject(
            LaneId::new(2),
            iroha_data_model::nexus::AxtRejectReason::Manifest,
            77,
        );
        state.telemetry.set_axt_reject_hint(
            dsid,
            LaneId::new(2),
            10,
            11,
            iroha_data_model::nexus::AxtRejectReason::HandleEra,
        );
        state
            .telemetry
            .set_axt_policy_snapshot_version(&AxtPolicySnapshot {
                version: policy_version,
                entries: policy_entries,
            });
    }

    let response = handler_axt_proof_cache_status(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo("127.0.0.1:8080".parse().unwrap()),
    )
    .await
    .expect("handler ok")
    .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let snapshot: iroha_core::telemetry::AxtDebugStatus =
        norito::json::from_slice(&body).expect("json decode");
    assert_eq!(snapshot.policy_snapshot_version, policy_version);
    assert_eq!(
        snapshot.last_reject.as_ref().map(|reject| reject.reason),
        Some(iroha_data_model::nexus::AxtRejectReason::Manifest)
    );
    assert_eq!(snapshot.hints.len(), 1);
    assert_eq!(
        snapshot.hints[0].reason,
        iroha_data_model::nexus::AxtRejectReason::HandleEra
    );
    assert_eq!(snapshot.cache.len(), 1);
    let entry = &snapshot.cache[0];
    assert_eq!(entry.dataspace, dsid);
    assert_eq!(entry.status, "miss");
    assert_eq!(entry.manifest_root, Some(manifest_root));
    assert_eq!(entry.verified_slot, 5);
    assert_eq!(entry.expiry_slot, Some(10));
}

#[test]
fn axt_reject_query_response_carries_headers() {
    let ctx = AxtRejectContext {
        reason: AxtRejectReason::HandleEra,
        dataspace: Some(DataSpaceId::new(7)),
        lane: Some(LaneId::new(3)),
        snapshot_version: Some(77),
        detail: "handle era below policy minimum".to_owned(),
        next_min_handle_era: Some(5),
        next_min_sub_nonce: Some(2),
    };
    let response = Error::Query(ValidationFail::AxtReject(ctx)).into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let headers = response.headers();
    assert_eq!(
        headers
            .get("x-iroha-axt-code")
            .and_then(|v| v.to_str().ok()),
        Some("AXT_HANDLE_ERA")
    );
    assert_eq!(
        headers
            .get("x-iroha-axt-reason")
            .and_then(|v| v.to_str().ok()),
        Some("era")
    );
    assert_eq!(
        headers
            .get("x-iroha-axt-snapshot-version")
            .and_then(|v| v.to_str().ok()),
        Some("77")
    );
    assert_eq!(
        headers
            .get("x-iroha-axt-dataspace")
            .and_then(|v| v.to_str().ok()),
        Some("7")
    );
    assert_eq!(
        headers
            .get("x-iroha-axt-lane")
            .and_then(|v| v.to_str().ok()),
        Some("3")
    );
    assert_eq!(
        headers
            .get("x-iroha-axt-next-handle-era")
            .and_then(|v| v.to_str().ok()),
        Some("5")
    );
    assert_eq!(
        headers
            .get("x-iroha-axt-next-sub-nonce")
            .and_then(|v| v.to_str().ok()),
        Some("2")
    );
    let body = executor::block_on(http_body_util::BodyExt::collect(response.into_body()))
        .expect("response body")
        .to_bytes();
    let envelope: ErrorEnvelope = norito::decode_from_bytes(&body).expect("error envelope payload");
    let axt = envelope
        .details
        .and_then(|details| details.axt)
        .expect("axt details");
    assert_eq!(axt.code.as_deref(), Some("AXT_HANDLE_ERA"));
    assert_eq!(axt.dataspace, Some(7));
    assert_eq!(axt.lane, Some(3));
}

#[tokio::test]
async fn proof_get_egress_throttled_returns_retry_after() {
    let mut app = mk_app_state_for_tests();
    let _ = seed_proof_record(&app, "debug-proof", [0xBB; 32]);
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.proof_limits.retry_after = std::time::Duration::from_secs(2);
        state.proof_egress_limiter = limits::RateLimiter::new_u64(Some(1), Some(1));
    }

    let resp = handler_get_proof_by_backend_hash(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        axum::extract::Path(("debug-proof".to_string(), hex::encode([0xBB; 32]))),
    )
    .await
    .unwrap_or_else(Error::into_response);

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after = resp
        .headers()
        .get(axum::http::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert_eq!(retry_after, "2");
}

#[tokio::test]
async fn proof_body_limit_rejects_oversize_body() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.proof_limits.max_body_bytes = 4;
    }
    let err = enforce_proof_body_limit(&app, 16, "v1/zk/verify-batch")
        .expect_err("oversized proof should be rejected");

    match err {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(msg),
        )) => assert!(
            msg.contains("payload too large"),
            "error message should explain limit"
        ),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn proof_request_rate_limit_counts_requests_instead_of_body_chunks() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.proof_rate_limiter = limits::RateLimiter::new(Some(2), Some(60));
    }

    // This was the first permanently unserviceable size under the former
    // 4-KiB chunk cost: floor(245_760 / 4_096) + 1 == 61 > burst 60.
    check_proof_access(
        &app,
        &HeaderMap::new(),
        Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        "v1/zk/verify-batch",
        PROOF_REQUEST_RATE_COST,
        true,
    )
    .await
    .expect("one maximum-size request must consume one request token");
}

#[tokio::test]
async fn proof_request_rate_limit_admits_max_body_cost_and_throttles_repetition() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.proof_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
    }
    let headers = HeaderMap::new();
    let remote = Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    let max_body = app.proof_limits.max_body_bytes as usize;
    enforce_proof_body_limit(&app, max_body, "v1/zk/verify-batch")
        .expect("configured maximum body remains admissible");

    check_proof_access(
        &app,
        &headers,
        remote,
        "v1/zk/verify-batch",
        PROOF_REQUEST_RATE_COST,
        true,
    )
    .await
    .expect("first admissible request should consume one request token");
    let err = check_proof_access(
        &app,
        &headers,
        remote,
        "v1/zk/verify-batch",
        PROOF_REQUEST_RATE_COST,
        true,
    )
    .await
    .expect_err("a repeated request should still be throttled");
    assert!(matches!(
        err,
        Error::ProofRateLimited {
            endpoint: "v1/zk/verify-batch",
            ..
        }
    ));
}
