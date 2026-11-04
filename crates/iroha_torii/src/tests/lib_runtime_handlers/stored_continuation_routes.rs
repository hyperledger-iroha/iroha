// Stored continuations route and authorize by the Start their cursor retains.
//
// A home-routed Start opens its cursor at the asset definition's home.
// Classified as authority-routed, the cursor's Continue still needed read
// grants on every unrelated restricted dataspace. The retained Start keeps the
// restricted-home grant check, the cursor's owner binding and the executor's
// exact-owner revalidation.

struct StoredContinuationFixture {
    app: SharedAppState,
    universal_domain: DomainId,
    restricted_domain: DomainId,
    domainless: AssetDefinitionId,
    universal_owned: AssetDefinitionId,
    restricted_owned: AssetDefinitionId,
    stranger_key: KeyPair,
}

/// Certified chain whose three DS definitions each carry two UID policies.
fn stored_continuation_fixture_for_test() -> StoredContinuationFixture {
    use iroha_core::sumeragi::test_chain::TestChainConfig;
    let xor: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        .parse()
        .expect("canonical XOR id");
    let universal_domain = DomainId::try_new("boi", "universal").expect("universal domain");
    let restricted_domain = DomainId::try_new("bank", "restricted").expect("restricted domain");
    let id = |name: &str| {
        AssetDefinitionId::derive_from_components(
            universal_domain.clone(),
            name.parse().expect("asset name"),
        )
    };
    let domainless = id("ds-domainless");
    let universal_owned = id("ds-universal-domain");
    let restricted_owned = id("ds-restricted-domain");
    let stranger_key = checked_torii_test_ed25519_keypair(0x62, "continuation stranger");
    let stranger = AccountId::new(stranger_key.public_key().clone());
    let first_member = checked_torii_test_account_id(0x63, "continuation first UID member");
    let second_member = checked_torii_test_account_id(0x64, "continuation second UID member");
    let fee_sink = checked_torii_test_account_id(0x65, "continuation fee sink");
    let mut nexus = private_ingress_nexus_for_test();
    assert_eq!(
        nexus.fees.fee_asset_id, xor,
        "the fixture pays its original fee in XOR"
    );
    nexus.fees.fee_sink_account_id = fee_sink.clone();
    let global = |id: &AssetDefinitionId, name: &str, domain: Option<&DomainId>| {
        AssetDefinition::numeric(
            id.clone(),
            name,
            AssetBalancePolicy::Global,
            domain.cloned(),
        )
        .build(&ALICE_ID)
    };
    let world = World::with_assets(
        [
            Domain::new(DomainId::try_new("wonderland", "universal").expect("domain"))
                .build(&ALICE_ID),
            Domain::new(universal_domain.clone()).build(&ALICE_ID),
            Domain::new(restricted_domain.clone()).build(&ALICE_ID),
        ],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(stranger.clone()).build(&stranger),
            Account::new(first_member.clone()).build(&first_member),
            Account::new(second_member.clone()).build(&second_member),
            Account::new(fee_sink.clone()).build(&fee_sink),
        ],
        [
            global(&xor, "XOR", None),
            global(&domainless, "DS", None),
            global(&universal_owned, "DS", Some(&universal_domain)),
            global(&restricted_owned, "DS", Some(&restricted_domain)),
        ],
        [Asset::new(
            AssetId::new(xor.clone(), ALICE_ID.clone()),
            Quantity::from(1_u32),
        )],
        [],
    );
    let mut config = TestChainConfig::new(world, 1_000);
    config.nexus = Some(nexus);
    let (mut app, _, chain) = executed_history_test_fixture(
        config,
        &ALICE_KEYPAIR,
        vec![Log::new(Level::INFO, "stored continuation fixture".to_owned()).into()],
        true,
    );
    Arc::get_mut(&mut app).unwrap().local_peer_id = Some(chain.validators()[0].0.clone());
    widen_ordinary_row_budget_for_test(&mut app, 4 * 1024);
    assert!(super::should_execute_route_locally(
        app.as_ref(),
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    ));
    assert!(
        route_dataspaces_for_test(&super::torii_all_dataspace_routes(app.as_ref()))
            .contains(&DataSpaceId::new(10)),
        "an authority-routed read would reach the unrelated restricted dataspace"
    );
    for definition in [&domainless, &universal_owned, &restricted_owned] {
        set_uid_policies_for_test(&app, definition, [&first_member, &second_member]);
    }
    StoredContinuationFixture {
        app,
        universal_domain,
        restricted_domain,
        domainless,
        universal_owned,
        restricted_owned,
        stranger_key,
    }
}

/// Give the fixture's ordinary queries a wider per-row source budget.
///
/// Under Torii's fixed 1 KiB row budget, Core's UID roster adapter refuses
/// even a one-member policy row with `CapacityLimit`. Routing and
/// authorization do not depend on that budget, so only the row widens; every
/// other limit keeps the fixture's production geometry.
fn widen_ordinary_row_budget_for_test(app: &mut SharedAppState, max_source_item_bytes: u64) {
    use iroha_core::smartcontracts::isi::query::OrdinaryQueryExecutionLimits;
    let app = Arc::get_mut(app).expect("the fixture app is not shared yet");
    let current = app.ordinary_query_policy.limits;
    let headroom = |row_bytes| {
        OrdinaryQueryExecutionLimits::required_execution_headroom_bytes(
            current.max_page_items(),
            row_bytes,
            current.max_response_bytes(),
            current.max_request_graph_bytes(),
            current.max_revalidation_archive_bytes(),
            current.revalidation_decode_limits(),
        )
        .expect("execution headroom geometry fits")
    };
    // Keep Torii's transport-copy surcharge above Core's minimum headroom.
    let surcharge = current.execution_headroom_bytes() - headroom(current.max_source_item_bytes());
    let cursor_items = current.max_cursor_retained_items();
    let cursor_value_bytes = cursor_items * max_source_item_bytes;
    let cursor_bytes = OrdinaryQueryExecutionLimits::required_cursor_retained_bytes(
        cursor_items,
        max_source_item_bytes,
        cursor_value_bytes,
        current.max_revalidation_archive_bytes(),
    )
    .expect("cursor retention geometry fits");
    app.ordinary_query_policy.limits = OrdinaryQueryExecutionLimits::try_new(
        super::next_ordinary_query_policy_generation(),
        current.execution_budget(),
        current.max_page_items(),
        headroom(max_source_item_bytes) + surcharge,
        max_source_item_bytes,
        current.max_response_bytes(),
        cursor_items,
        cursor_value_bytes,
        cursor_bytes,
        current.max_request_graph_bytes(),
        current.max_revalidation_archive_bytes(),
        current.revalidation_decode_limits(),
    )
    .expect("the wider row budget fits the fixture's execution budget");
    assert!(
        app.query_fanout_inflight
            .can_reserve_parts(app.ordinary_query_policy.start_reservation_parts()),
        "the fixture's query pool admits one wider stored Start"
    );
}

/// Install one single-member UID policy per member, as the definition owner.
fn set_uid_policies_for_test(
    app: &SharedAppState,
    asset_definition_id: &AssetDefinitionId,
    members: [&AccountId; 2],
) {
    use iroha_data_model::{
        asset::uid::{AssetUidDailyWindowV1, AssetUidPolicyV1},
        isi::asset_uid::SetAssetUidPolicyV1,
    };
    let next_height = app
        .state
        .latest_block_header_fast()
        .map_or(1, |header| header.height().get().saturating_add(1));
    let mut block = app.state.block(BlockHeader::new(
        NonZeroU64::new(next_height).expect("non-zero height"),
        None,
        None,
        0,
        0,
    ));
    let mut tx = block.transaction();
    for (uid, member) in (1_u8..).zip(members) {
        SetAssetUidPolicyV1 {
            policy: AssetUidPolicyV1 {
                asset_definition_id: asset_definition_id.clone(),
                uid: [uid; 32],
                revision: 1,
                accounts: std::collections::BTreeSet::from([member.clone()]),
                holding_limit: None,
                daily_window: AssetUidDailyWindowV1::UtcDay,
                daily_spend_limit: None,
                daily_outgoing_limit: None,
            },
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("the owner installs a UID policy");
    }
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit UID policies");
}

fn revoke_account_permission_for_test(
    app: &SharedAppState,
    authority: &AccountId,
    permission: &Permission,
) {
    let next_height = app
        .state
        .latest_block_header_fast()
        .map_or(1, |header| header.height().get().saturating_add(1));
    let mut block = app.state.block(BlockHeader::new(
        NonZeroU64::new(next_height).expect("non-zero height"),
        None,
        None,
        0,
        0,
    ));
    let mut tx = block.transaction();
    assert!(
        tx.world_mut_for_testing()
            .remove_account_permission(authority, permission),
        "the revoked grant was held"
    );
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit permission revocation");
}

/// The full UID roster of one definition, one row per page.
fn uid_roster_start_for_test(
    asset_definition_id: &AssetDefinitionId,
) -> iroha_data_model::query::QueryRequest {
    asset_home_start_for_test!(iroha_data_model::query::asset_uid::FindAssetUidPoliciesV1 {
        asset_definition_id: asset_definition_id.clone(),
        uid: None,
        wallet_account_id: None,
    })
}

/// Check one page served at `home`; return its output.
async fn home_page_for_test(
    name: &str,
    outcome: Result<Response, Error>,
    home: &str,
) -> iroha_data_model::query::QueryOutput {
    let response = outcome
        .unwrap_or_else(|error| panic!("{name}: the owner reads its home: {error:?}"))
        .into_response();
    assert_eq!(response.status(), StatusCode::OK, "{name}");
    assert_eq!(
        torii_response_header(&response, "x-iroha-route-dataspace-id"),
        Some(home),
        "{name} runs at its home"
    );
    assert_eq!(
        torii_response_header(&response, "x-iroha-routed-by"),
        Some("local"),
        "{name}"
    );
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("query response body");
    let iroha_data_model::query::QueryResponse::Iterable(output) =
        norito::decode_from_bytes(&body).expect("decode iterable response")
    else {
        panic!("{name} returns an iterable response");
    };
    output
}

/// Check one stored page served at `home`; return its rows and continuation.
async fn stored_page_for_test(
    name: &str,
    outcome: Result<Response, Error>,
    home: &str,
) -> (
    usize,
    Option<iroha_data_model::query::parameters::ForwardCursor>,
) {
    let output = home_page_for_test(name, outcome, home).await;
    (output.batch.len(), output.continue_cursor)
}

/// Continue `cursor` as the owner until the roster ends; return the rows read.
async fn finish_stored_roster_for_test(
    app: &SharedAppState,
    mut cursor: iroha_data_model::query::parameters::ForwardCursor,
    home: &str,
) -> (usize, iroha_data_model::query::parameters::ForwardCursor) {
    let mut rows = 0;
    for _ in 0..2 {
        let (page_rows, next) = stored_page_for_test(
            "UID roster Continue",
            asset_home_signed_query_for_test(
                app,
                &ALICE_KEYPAIR,
                iroha_data_model::query::QueryRequest::Continue(cursor.clone()),
                Some("stored"),
            )
            .await,
            home,
        )
        .await;
        rows += page_rows;
        match next {
            Some(next) => cursor = next,
            None => return (rows, cursor),
        }
    }
    panic!("a two-row roster ends within two continuations");
}

fn refused_with_for_test(response: &Response, status: StatusCode, code: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(
        torii_response_header(response, "x-iroha-reject-code"),
        Some(code)
    );
}

#[tokio::test]
async fn stored_continuations_of_home_reads_run_at_the_home_without_restricted_grants() {
    use super::SignedQueryScope;
    use iroha_data_model::query::QueryRequest;
    let fixture = stored_continuation_fixture_for_test();
    let app = &fixture.app;
    let stranger = AccountId::new(fixture.stranger_key.public_key().clone());
    let limits = app.query_ingress_envelope.query_scope_limits();
    let scopes = |request: &iroha_data_model::query::QueryRequestWithAuthority| {
        let unbounded = super::signed_query_scope_for_app(app.as_ref(), request);
        let bounded = super::signed_query_scope_for_app_bounded(app.as_ref(), request, limits)
            .expect("bounded scope classification");
        assert_eq!(unbounded, bounded, "both classifiers agree");
        bounded
    };
    for (definition, home_scope) in [
        (
            &fixture.domainless,
            SignedQueryScope::UniversalAssetDefinition(fixture.domainless.clone()),
        ),
        (
            &fixture.universal_owned,
            SignedQueryScope::TargetDomain(fixture.universal_domain.clone()),
        ),
    ] {
        // The node's default ephemeral mode serves one page and keeps no
        // cursor, so the SDK never sends a Continue for it.
        let ephemeral = home_page_for_test(
            "ephemeral UID roster Start",
            asset_home_signed_query_for_test(
                app,
                &ALICE_KEYPAIR,
                uid_roster_start_for_test(definition),
                None,
            )
            .await,
            "0",
        )
        .await;
        assert_eq!(ephemeral.batch.len(), 1);
        assert!(ephemeral.has_more && ephemeral.continue_cursor.is_none());
        let (rows, cursor) = stored_page_for_test(
            "UID roster Start",
            asset_home_signed_query_for_test(
                app,
                &ALICE_KEYPAIR,
                uid_roster_start_for_test(definition),
                Some("stored"),
            )
            .await,
            "0",
        )
        .await;
        assert_eq!(rows, 1, "one row per page");
        let cursor = cursor.expect("the second UID policy leaves a stored cursor");
        let continuation = request_for_test(&ALICE_ID, QueryRequest::Continue(cursor.clone()));
        assert_eq!(
            scopes(&continuation),
            home_scope,
            "{definition}: the continuation routes like its Start"
        );
        assert_eq!(
            super::signed_query_scope(&continuation),
            SignedQueryScope::AuthorityRouted,
            "a source-free classifier never invents a home"
        );
        // The authority-routed classification is what refused the grant-free owner.
        refused_with_for_test(
            &super::torii_authorized_signed_query_routes(
                app.as_ref(),
                &continuation,
                &SignedQueryScope::AuthorityRouted,
            )
            .expect_err("every restricted route needs its read grant"),
            StatusCode::FORBIDDEN,
            "permission_denied",
        );
        let routes =
            super::torii_authorized_signed_query_routes(app.as_ref(), &continuation, &home_scope)
                .unwrap_or_else(|response| {
                    panic!("a public home needs no grant: {}", response.status())
                });
        assert_eq!(
            route_dataspaces_for_test(&routes),
            vec![DataSpaceId::UNIVERSAL]
        );
        // The store releases a retained Start only to the cursor's owner.
        let foreign = request_for_test(&stranger, QueryRequest::Continue(cursor.clone()));
        assert_eq!(scopes(&foreign), SignedQueryScope::AuthorityRouted);
        refused_with_for_test(
            &asset_home_signed_query_for_test(
                app,
                &fixture.stranger_key,
                QueryRequest::Continue(cursor.clone()),
                Some("stored"),
            )
            .await
            .unwrap_or_else(IntoResponse::into_response),
            StatusCode::FORBIDDEN,
            "permission_denied",
        );
        let (rows, finished) = finish_stored_roster_for_test(app, cursor, "0").await;
        assert_eq!(rows, 1, "the owner reads the second UID policy at the home");
        assert_eq!(
            scopes(&request_for_test(
                &ALICE_ID,
                QueryRequest::Continue(finished)
            )),
            SignedQueryScope::AuthorityRouted,
            "a finished cursor retains no Start"
        );
    }
}

#[tokio::test]
async fn stored_continuations_of_restricted_home_reads_still_need_the_home_grant() {
    use super::SignedQueryScope;
    use iroha_data_model::query::QueryRequest;
    let fixture = stored_continuation_fixture_for_test();
    let app = &fixture.app;
    let restricted = DataSpaceId::new(10);
    let grant: Permission = CanReadRestrictedDataspace {
        dataspace: restricted,
    }
    .into();
    refused_with_for_test(
        &asset_home_signed_query_for_test(
            app,
            &ALICE_KEYPAIR,
            uid_roster_start_for_test(&fixture.restricted_owned),
            Some("stored"),
        )
        .await
        .unwrap_or_else(IntoResponse::into_response),
        StatusCode::FORBIDDEN,
        "permission_denied",
    );
    grant_account_permission_for_test(app, &ALICE_ID, grant.clone());
    let (rows, cursor) = stored_page_for_test(
        "restricted UID roster Start",
        asset_home_signed_query_for_test(
            app,
            &ALICE_KEYPAIR,
            uid_roster_start_for_test(&fixture.restricted_owned),
            Some("stored"),
        )
        .await,
        "10",
    )
    .await;
    assert_eq!(rows, 1);
    let cursor = cursor.expect("the second UID policy leaves a stored cursor");
    let continuation = request_for_test(&ALICE_ID, QueryRequest::Continue(cursor.clone()));
    let limits = app.query_ingress_envelope.query_scope_limits();
    assert_eq!(
        super::signed_query_scope_for_app_bounded(app.as_ref(), &continuation, limits)
            .expect("bounded scope classification"),
        SignedQueryScope::TargetDomain(fixture.restricted_domain.clone())
    );
    revoke_account_permission_for_test(app, &ALICE_ID, &grant);
    refused_with_for_test(
        &asset_home_signed_query_for_test(
            app,
            &ALICE_KEYPAIR,
            QueryRequest::Continue(cursor.clone()),
            Some("stored"),
        )
        .await
        .unwrap_or_else(IntoResponse::into_response),
        StatusCode::FORBIDDEN,
        "permission_denied",
    );
    // Torii's refusal neither consumed nor advanced the cursor.
    grant_account_permission_for_test(app, &ALICE_ID, grant);
    let (rows, _) = finish_stored_roster_for_test(app, cursor, "10").await;
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn stored_continuations_stay_bound_to_the_cursor_owner() {
    use iroha_data_model::query::QueryRequest;
    let fixture = stored_continuation_fixture_for_test();
    let app = &fixture.app;
    let stranger = AccountId::new(fixture.stranger_key.public_key().clone());
    // With every read grant, Torii admits the stranger as authority-routed;
    // Core still refuses a cursor opened by another authority.
    grant_account_permission_for_test(
        app,
        &stranger,
        CanReadRestrictedDataspace {
            dataspace: DataSpaceId::new(10),
        }
        .into(),
    );
    let (_, cursor) = stored_page_for_test(
        "UID roster Start",
        asset_home_signed_query_for_test(
            app,
            &ALICE_KEYPAIR,
            uid_roster_start_for_test(&fixture.domainless),
            Some("stored"),
        )
        .await,
        "0",
    )
    .await;
    let cursor = cursor.expect("the second UID policy leaves a stored cursor");
    let outcome = asset_home_signed_query_for_test(
        app,
        &fixture.stranger_key,
        QueryRequest::Continue(cursor.clone()),
        Some("stored"),
    )
    .await;
    assert!(
        matches!(
            outcome,
            Err(Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Expired
            )))
        ),
        "a foreign cursor is indistinguishable from an expired one"
    );
    let (rows, _) = finish_stored_roster_for_test(app, cursor, "0").await;
    assert_eq!(
        rows, 1,
        "the foreign attempt left the owner's cursor intact"
    );
}

#[cfg(feature = "connect")]
#[tokio::test]
async fn proxied_fanout_and_route_scans_still_refuse_live_client_continuations() {
    use iroha_data_model::query::QueryRequest;
    let fixture = stored_continuation_fixture_for_test();
    let app = &fixture.app;
    let (_, cursor) = stored_page_for_test(
        "UID roster Start",
        asset_home_signed_query_for_test(
            app,
            &ALICE_KEYPAIR,
            uid_roster_start_for_test(&fixture.domainless),
            Some("stored"),
        )
        .await,
        "0",
    )
    .await;
    let cursor = cursor.expect("the second UID policy leaves a stored cursor");
    let signed_continuation = || {
        let mut request =
            authorize_query_for_test(QueryRequest::Continue(cursor.clone()), ALICE_ID.clone());
        request.network_id = *app.state.network_id_ref();
        iroha_version::codec::EncodeVersioned::encode_versioned(&request.sign(&ALICE_KEYPAIR))
    };
    refused_with_for_test(
        &super::execute_incoming_torii_signed_query_route_scan(
            app,
            signed_continuation(),
            ToriiRouteHintV1::from(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)),
            ToriiProxyResponseFormatV1::Norito,
            None,
        )
        .await,
        StatusCode::BAD_REQUEST,
        "query_unsupported",
    );
    refused_with_for_test(
        &super::execute_torii_signed_query_fanout_proxy_request(
            app,
            signed_continuation(),
            ToriiProxyResponseFormatV1::Norito,
            None,
            None,
        )
        .await,
        StatusCode::BAD_REQUEST,
        "query_unsupported",
    );
    refused_with_for_test(
        &super::ensure_bounded_fanout_query(&request_for_test(
            &ALICE_ID,
            QueryRequest::Continue(cursor.clone()),
        ))
        .expect_err("fanout never carries a client continuation"),
        StatusCode::CONFLICT,
        "query_unsupported",
    );
    let (rows, _) = finish_stored_roster_for_test(app, cursor, "0").await;
    assert_eq!(rows, 1, "the refusals left the owner's cursor intact");
}
