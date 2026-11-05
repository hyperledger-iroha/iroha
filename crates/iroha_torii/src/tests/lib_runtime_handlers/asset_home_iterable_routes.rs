// Asset-keyed native iterable queries route to their asset definition's home.
//
// An asset owner signs these Starts for its own definition. Classified as a
// cross-dataspace fanout, each also needed read grants on every unrelated
// restricted dataspace. Home routing keeps the restricted-home grant check and
// the executor's exact-owner gate.

/// One asset-keyed iterable Start, encoded as the SDK executors encode it.
macro_rules! asset_home_start_for_test {
    ($query:expr) => {
        iroha_data_model::query::QueryRequest::Start(
            iroha_data_model::query::builder::QueryBuilder::new(&(), $query)
                .with_fetch_size(iroha_data_model::query::parameters::FetchSize {
                    fetch_size: NonZeroU64::new(1),
                })
                .into_request()
                .1,
        )
    };
}

/// Every asset-keyed iterable Start for one definition, with each SDK's cursor mode.
fn asset_home_starts_for_test(
    asset_definition_id: &AssetDefinitionId,
    wallet: &AccountId,
    fi: &AccountId,
    incarnation: iroha_data_model::nexus::AxtAssetIncarnationV1,
) -> [(
    &'static str,
    Option<&'static str>,
    iroha_data_model::query::QueryRequest,
); 6] {
    use iroha_data_model::{
        asset::fi_context::AssetFiContextTargetV1,
        isi::asset_fi::TransferToLinkedFiV1,
        query::{
            asset_escrow_scope::FindAssetEscrowsByAssetDefinitionV1,
            asset_fi::{FindAssetFiLinkV1, FindAssetFiTransferReceiptV1},
            asset_fi_context::FindAssetFiContextV1,
            asset_uid::FindAssetUidPoliciesV1,
            asset_uid_enrollment::FindAssetUidEnrollmentV1,
        },
    };
    [
        (
            "FindAssetUidPoliciesV1",
            None,
            asset_home_start_for_test!(FindAssetUidPoliciesV1 {
                asset_definition_id: asset_definition_id.clone(),
                uid: None,
                wallet_account_id: None,
            }),
        ),
        (
            "FindAssetUidEnrollmentV1",
            None,
            asset_home_start_for_test!(FindAssetUidEnrollmentV1 {
                asset_definition_id: asset_definition_id.clone(),
            }),
        ),
        (
            "FindAssetFiLinkV1",
            None,
            asset_home_start_for_test!(FindAssetFiLinkV1 {
                asset_definition_id: asset_definition_id.clone(),
                account_id: wallet.clone(),
                fi_account_id: fi.clone(),
                reference_digest: [9; 32],
            }),
        ),
        (
            "FindAssetFiTransferReceiptV1",
            Some("ephemeral"),
            asset_home_start_for_test!(FindAssetFiTransferReceiptV1 {
                asset_definition_id: asset_definition_id.clone(),
                expected_incarnation: incarnation,
                account_id: wallet.clone(),
                execution_call_hash: Hash::new(b"torii::asset-home-receipt-call"),
                transcript_index: 0,
            }),
        ),
        (
            "FindAssetFiContextV1",
            Some("ephemeral"),
            asset_home_start_for_test!(FindAssetFiContextV1 {
                target: AssetFiContextTargetV1::Transfer {
                    instruction: TransferToLinkedFiV1 {
                        asset_definition_id: asset_definition_id.clone(),
                        account_id: wallet.clone(),
                        amount: Quantity::from(5_u32),
                        expected_link_revision: 1,
                        expected_policy_revision: 1,
                    },
                    fi_account_id: fi.clone(),
                    reference_digest: [9; 32],
                    expected_fi_revision: 1,
                    expected_attestation_revision: 1,
                },
                expected_incarnation: incarnation,
                expected_uid: [7; 32],
            }),
        ),
        (
            "FindAssetEscrowsByAssetDefinitionV1",
            Some("ephemeral"),
            asset_home_start_for_test!(FindAssetEscrowsByAssetDefinitionV1 {
                asset_definition_id: asset_definition_id.clone(),
                expected_incarnation: incarnation,
            }),
        ),
    ]
}

fn asset_home_test_incarnation() -> iroha_data_model::nexus::AxtAssetIncarnationV1 {
    iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
        *Hash::new(b"torii::asset-home-incarnation").as_ref(),
    )
    .expect("valid incarnation token")
}

fn route_dataspaces_for_test(routes: &[RoutingDecision]) -> Vec<DataSpaceId> {
    routes.iter().map(|route| route.dataspace_id).collect()
}

#[tokio::test]
async fn asset_keyed_iterable_queries_route_to_the_definition_home() {
    use super::SignedQueryScope;
    use iroha_data_model::query::{
        QueryItemKind, QueryRequest, asset::prelude::FindAssetDefinitionById,
        asset_fi::FindAssetFiLinkV1, asset_fi_context::FindAssetFiContextV1,
        asset_uid_enrollment::FindAssetUidEnrollmentV1,
    };
    let wallet = checked_torii_test_account_id(0x5b, "asset-home routing wallet");
    let fi = checked_torii_test_account_id(0x5c, "asset-home routing FI");
    let universal_domain = DomainId::try_new("boi", "universal").expect("universal domain");
    let public_domain = DomainId::try_new("hbl", "governance").expect("public domain");
    let restricted_domain = DomainId::try_new("bank", "restricted").expect("restricted domain");
    let id = |name: &str| {
        AssetDefinitionId::derive_from_components(
            universal_domain.clone(),
            name.parse().expect("asset name"),
        )
    };
    let domainless = id("ds-domainless");
    let universal_owned = id("ds-universal-domain");
    let public_owned = id("ds-public-domain");
    let restricted_owned = id("ds-restricted-domain");
    let direct_restricted = id("ds-direct-restricted");
    let unknown = id("ds-unknown");
    let global = |id: &AssetDefinitionId, domain: Option<&DomainId>| {
        AssetDefinition::numeric(
            id.clone(),
            "DS",
            AssetBalancePolicy::Global,
            domain.cloned(),
        )
        .build(&ALICE_ID)
    };
    let mut world = World::with(
        [
            Domain::new(universal_domain.clone()).build(&ALICE_ID),
            Domain::new(public_domain.clone()).build(&ALICE_ID),
            Domain::new(restricted_domain.clone()).build(&ALICE_ID),
        ],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(wallet.clone()).build(&wallet),
        ],
        [
            global(&domainless, None),
            global(&universal_owned, Some(&universal_domain)),
            global(&public_owned, Some(&public_domain)),
            global(&restricted_owned, Some(&restricted_domain)),
        ],
    );
    let restricted_dataspace = DataSpaceId::new(10);
    world
        .insert_direct_asset_definition_with_assets_for_testing(
            AssetDefinition::numeric(
                direct_restricted.clone(),
                "DS",
                AssetBalancePolicy::DataspaceRestricted,
                None,
            )
            .build(&ALICE_ID),
            restricted_dataspace,
            [],
        )
        .expect("direct restricted home fixture");
    let mut app =
        mk_app_state_for_tests_with_world_and_nexus(world, private_ingress_nexus_for_test());
    let (_, restricted) = configure_private_ingress_routes_for_test(&mut app);
    assert_eq!(restricted, restricted_dataspace);
    assert!(
        route_dataspaces_for_test(&super::torii_all_dataspace_routes(app.as_ref()))
            .contains(&restricted),
        "a fanout reaches the unrelated restricted dataspace"
    );
    let limits = app.query_ingress_envelope.query_scope_limits();
    let homes = [
        (
            &domainless,
            SignedQueryScope::UniversalAssetDefinition(domainless.clone()),
            DataSpaceId::UNIVERSAL,
        ),
        (
            &universal_owned,
            SignedQueryScope::TargetDomain(universal_domain.clone()),
            DataSpaceId::UNIVERSAL,
        ),
        (
            &public_owned,
            SignedQueryScope::TargetDomain(public_domain.clone()),
            DataSpaceId::new(1),
        ),
        (
            &restricted_owned,
            SignedQueryScope::TargetDomain(restricted_domain.clone()),
            restricted,
        ),
        (
            &direct_restricted,
            SignedQueryScope::DataspaceAssetDefinition(restricted),
            restricted,
        ),
    ];
    let scopes = |request: &iroha_data_model::query::QueryRequestWithAuthority| {
        let unbounded = super::signed_query_scope_for_app(app.as_ref(), request);
        let bounded = super::signed_query_scope_for_app_bounded(app.as_ref(), request, limits)
            .expect("bounded scope classification");
        assert_eq!(unbounded, bounded, "both classifiers agree");
        bounded
    };
    let denied = |request: &iroha_data_model::query::QueryRequestWithAuthority,
                  scope: &SignedQueryScope| {
        let response = super::torii_authorized_signed_query_routes(app.as_ref(), request, scope)
            .expect_err("a restricted route requires its read grant");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            torii_response_header(&response, "x-iroha-reject-code"),
            Some("permission_denied")
        );
    };
    for (definition, expected, home) in &homes {
        let singular = request_for_test(
            &ALICE_ID,
            QueryRequest::Singular(FindAssetDefinitionById::new((*definition).clone()).into()),
        );
        assert_eq!(
            &scopes(&singular),
            expected,
            "iterable reads route exactly like the singular definition lookup"
        );
        for (name, _, start) in
            asset_home_starts_for_test(definition, &wallet, &fi, asset_home_test_incarnation())
        {
            let request = roundtrip_request_for_test(&ALICE_ID, start);
            assert_eq!(
                &scopes(&request),
                expected,
                "{name} routes to {definition}'s home"
            );
            assert_eq!(
                super::signed_query_scope(&request),
                SignedQueryScope::CrossDataspaceFanout,
                "{name}: a source-free classifier never invents a home"
            );
            if *home == restricted {
                denied(&request, expected);
            } else {
                let routes =
                    super::torii_authorized_signed_query_routes(app.as_ref(), &request, expected)
                        .unwrap_or_else(|response| {
                            panic!(
                                "{name} at a public home needs no grant: {}",
                                response.status()
                            )
                        });
                assert_eq!(route_dataspaces_for_test(&routes), vec![*home], "{name}");
            }
        }
    }
    // The old fanout classification is what refused the live DS owner.
    let request = request_for_test(
        &ALICE_ID,
        asset_home_start_for_test!(FindAssetUidEnrollmentV1 {
            asset_definition_id: domainless.clone(),
        }),
    );
    denied(&request, &SignedQueryScope::CrossDataspaceFanout);
    // An unknown definition never inherits a public route.
    for (name, _, start) in
        asset_home_starts_for_test(&unknown, &wallet, &fi, asset_home_test_incarnation())
    {
        let request = request_for_test(&ALICE_ID, start);
        assert_eq!(
            scopes(&request),
            SignedQueryScope::CrossDataspaceFanout,
            "{name} for an unknown definition"
        );
    }
    // FI context targets name their definition in either variant.
    let link_approval = request_for_test(
        &ALICE_ID,
        asset_home_start_for_test!(FindAssetFiContextV1 {
            target: iroha_data_model::asset::fi_context::AssetFiContextTargetV1::LinkApproval(
                iroha_data_model::isi::asset_fi::SetAssetFiLinkV1 {
                    link: iroha_data_model::asset::fi_link::AssetFiLinkV1 {
                        asset_definition_id: universal_owned.clone(),
                        account_id: wallet.clone(),
                        fi_account_id: fi.clone(),
                        reference_digest: [9; 32],
                        fi_revision: 1,
                        attestation_revision: 1,
                        revision: 1,
                        enabled: true,
                        controller: None,
                        waterfall: false,
                        reverse_waterfall: false,
                    },
                    expected_policy_revision: 1,
                },
            ),
            expected_incarnation: asset_home_test_incarnation(),
            expected_uid: [7; 32],
        }),
    );
    assert_eq!(
        scopes(&link_approval),
        SignedQueryScope::TargetDomain(universal_domain.clone())
    );
    // A payload must be the exact canonical query of its own item kind.
    let enrollment_payload = norito::codec::Encode::encode(&FindAssetUidEnrollmentV1 {
        asset_definition_id: domainless.clone(),
    });
    let mut trailing = enrollment_payload.clone();
    trailing.push(0);
    let link_payload = norito::codec::Encode::encode(&FindAssetFiLinkV1 {
        asset_definition_id: domainless.clone(),
        account_id: wallet.clone(),
        fi_account_id: fi.clone(),
        reference_digest: [9; 32],
    });
    for (item, payload) in [
        (
            QueryItemKind::AssetUidPolicyViewV1,
            enrollment_payload.clone(),
        ),
        (
            QueryItemKind::AssetFiRelationshipViewV1,
            enrollment_payload.clone(),
        ),
        (
            QueryItemKind::AssetFiTransferReceiptV1,
            link_payload.clone(),
        ),
        (
            QueryItemKind::AssetFiContextSnapshotV1,
            link_payload.clone(),
        ),
        (
            QueryItemKind::AssetEscrowScopeOriginalV1,
            enrollment_payload,
        ),
        (QueryItemKind::AssetUidEnrollmentViewV1, trailing),
        (QueryItemKind::AssetUidEnrollmentViewV1, link_payload),
    ] {
        let request = request_for_test(
            &ALICE_ID,
            QueryRequest::Start(iroha_data_model::query::QueryWithParams {
                query: (),
                query_payload: payload,
                item,
                predicate_bytes: Vec::new(),
                selector_bytes: Vec::new(),
                params: iroha_data_model::query::parameters::QueryParams::default(),
            }),
        );
        assert_eq!(
            scopes(&request),
            SignedQueryScope::CrossDataspaceFanout,
            "{item:?} with a foreign payload keeps the fanout classification"
        );
    }
    grant_account_permission_for_test(
        &app,
        &ALICE_ID,
        CanReadRestrictedDataspace {
            dataspace: restricted,
        }
        .into(),
    );
    for definition in [&restricted_owned, &direct_restricted] {
        for (name, _, start) in
            asset_home_starts_for_test(definition, &wallet, &fi, asset_home_test_incarnation())
        {
            let request = request_for_test(&ALICE_ID, start);
            let scope = scopes(&request);
            let routes =
                super::torii_authorized_signed_query_routes(app.as_ref(), &request, &scope)
                    .unwrap_or_else(|response| {
                        panic!(
                            "{name}: the exact home grant authorizes it: {}",
                            response.status()
                        )
                    });
            assert_eq!(
                route_dataspaces_for_test(&routes),
                vec![restricted],
                "{name}"
            );
        }
    }
}

#[tokio::test]
async fn other_signed_query_scopes_are_unchanged_by_asset_home_routing() {
    use super::SignedQueryScope;
    use iroha_data_model::query::{
        QueryRequest,
        asset::prelude::{FindAssetDefinitions, FindAssets},
        escrow::prelude::{FindAssetEscrowsByBuyer, FindAssetEscrowsBySeller},
    };
    let account = checked_torii_test_account_id(0x61, "unchanged-scope target account");
    let universal_domain = DomainId::try_new("boi", "universal").expect("universal domain");
    let domain_owned = AssetDefinitionId::derive_from_components(
        universal_domain.clone(),
        "ds-universal-domain".parse().expect("asset name"),
    );
    let domainless = AssetDefinitionId::derive_from_components(
        universal_domain.clone(),
        "ds-domainless".parse().expect("asset name"),
    );
    let world = World::with(
        [Domain::new(universal_domain.clone()).build(&ALICE_ID)],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(account.clone()).build(&account),
        ],
        [
            AssetDefinition::numeric(
                domain_owned.clone(),
                "DS",
                AssetBalancePolicy::Global,
                Some(universal_domain.clone()),
            )
            .build(&ALICE_ID),
            AssetDefinition::numeric(domainless.clone(), "DS", AssetBalancePolicy::Global, None)
                .build(&ALICE_ID),
        ],
    );
    let mut app =
        mk_app_state_for_tests_with_world_and_nexus(world, private_ingress_nexus_for_test());
    configure_private_ingress_routes_for_test(&mut app);
    let limits = app.query_ingress_envelope.query_scope_limits();
    let cursor = iroha_data_model::query::parameters::ForwardCursor {
        query: "00".repeat(32),
        cursor: NonZeroU64::new(1).expect("one is non-zero"),
        gas_budget: None,
    };
    let cases = [
        (
            QueryRequest::Start(build_find_accounts_with_asset_query_for_test(
                domain_owned.clone(),
            )),
            SignedQueryScope::TargetDomain(universal_domain),
        ),
        (
            QueryRequest::Start(build_find_accounts_with_asset_query_for_test(domainless)),
            SignedQueryScope::CrossDataspaceFanout,
        ),
        (
            QueryRequest::Start(build_find_assets_by_account_query_for_test(account.clone())),
            SignedQueryScope::TargetAccount(account.clone()),
        ),
        (
            QueryRequest::Start(build_find_domains_by_account_query_for_test(
                account.clone(),
            )),
            SignedQueryScope::TargetAccount(account.clone()),
        ),
        (
            QueryRequest::Start(build_find_nfts_by_account_query_for_test(account.clone())),
            SignedQueryScope::TargetAccount(account.clone()),
        ),
        (
            QueryRequest::Start(build_find_permissions_by_account_query_for_test(
                account.clone(),
            )),
            SignedQueryScope::TargetAccount(account.clone()),
        ),
        (
            QueryRequest::Start(build_find_roles_by_account_query_for_test(account.clone())),
            SignedQueryScope::TargetAccount(account.clone()),
        ),
        (
            asset_home_start_for_test!(FindAssetEscrowsBySeller {
                seller: account.clone(),
            }),
            SignedQueryScope::TargetAccount(account.clone()),
        ),
        (
            asset_home_start_for_test!(FindAssetEscrowsByBuyer {
                buyer: account.clone(),
            }),
            SignedQueryScope::TargetAccount(account),
        ),
        (
            QueryRequest::Start(build_find_transactions_query_for_test()),
            SignedQueryScope::AuthorityRouted,
        ),
        (
            QueryRequest::Start(build_find_peers_query_for_test()),
            SignedQueryScope::PublicControlPlane,
        ),
        (
            QueryRequest::Start(build_find_triggers_query_for_test()),
            SignedQueryScope::LocalReplicated,
        ),
        (
            QueryRequest::Start(build_find_active_trigger_ids_query_for_test()),
            SignedQueryScope::LocalReplicated,
        ),
        (
            asset_home_start_for_test!(FindAssetDefinitions),
            SignedQueryScope::CrossDataspaceFanout,
        ),
        (
            asset_home_start_for_test!(FindAssets),
            SignedQueryScope::CrossDataspaceFanout,
        ),
        (
            QueryRequest::Continue(cursor),
            SignedQueryScope::AuthorityRouted,
        ),
    ];
    for (query, expected) in cases {
        let request = request_for_test(&ALICE_ID, query);
        assert_eq!(
            super::signed_query_scope_for_app(app.as_ref(), &request),
            expected
        );
        assert_eq!(
            super::signed_query_scope_for_app_bounded(app.as_ref(), &request, limits)
                .expect("bounded scope classification"),
            expected
        );
    }
}

fn initialize_asset_escrow_scope_for_test(
    app: &SharedAppState,
    asset_definition_id: &AssetDefinitionId,
) -> iroha_data_model::nexus::AxtAssetIncarnationV1 {
    let incarnation = app
        .state
        .view()
        .world()
        .axt_asset_incarnations()
        .get(asset_definition_id)
        .copied()
        .expect("genesis definitions carry a live incarnation");
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
    iroha_data_model::isi::asset_escrow_scope::InitializeAssetEscrowReadScopeV1 {
        asset_definition_id: asset_definition_id.clone(),
        expected_incarnation: incarnation,
        expected_revision: 0,
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("the owner initializes its escrow read scope");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit escrow read scope");
    incarnation
}

async fn asset_home_signed_query_for_test(
    app: &SharedAppState,
    key: &KeyPair,
    request: iroha_data_model::query::QueryRequest,
    cursor_mode: Option<&str>,
) -> Result<Response, Error> {
    let mut request = authorize_query_for_test(request, AccountId::new(key.public_key().clone()));
    request.network_id = *app.state.network_id_ref();
    super::handler_signed_query(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        )),
        crate::NoritoQuery(QueryOptions {
            cursor_mode: cursor_mode.map(str::to_owned),
            count_mode: cursor_mode.map(|_| "bounded".to_owned()),
            ..QueryOptions::default()
        }),
        versioned_query_for_test(request.sign(key)),
    )
    .await
}

#[tokio::test]
async fn owner_signed_asset_keyed_iterable_reads_execute_at_their_home() {
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
    let stranger_key = checked_torii_test_ed25519_keypair(0x5d, "asset-home stranger");
    let stranger = AccountId::new(stranger_key.public_key().clone());
    let wallet_key = checked_torii_test_ed25519_keypair(0x5e, "asset-home wallet");
    let wallet = AccountId::new(wallet_key.public_key().clone());
    let fi = checked_torii_test_account_id(0x5f, "asset-home FI");
    let fee_sink = checked_torii_test_account_id(0x60, "asset-home fee sink");
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
            Account::new(wallet.clone()).build(&wallet),
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
        vec![Log::new(Level::INFO, "asset-home routing fixture".to_owned()).into()],
        true,
    );
    Arc::get_mut(&mut app).unwrap().local_peer_id = Some(chain.validators()[0].0.clone());
    let universal_route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    assert!(super::should_execute_route_locally(
        app.as_ref(),
        universal_route
    ));
    let restricted = DataSpaceId::new(10);
    assert!(
        route_dataspaces_for_test(&super::torii_all_dataspace_routes(app.as_ref()))
            .contains(&restricted),
        "a fanout would reach the unrelated restricted dataspace"
    );
    // Rows for each admitted Start, in `asset_home_starts_for_test` order. No FI
    // receipt is stored yet, and the FI context reports absent observations.
    let expected_rows = [0, 1, 1, 0, 1, 1];
    for definition in [&domainless, &universal_owned] {
        let incarnation = initialize_asset_escrow_scope_for_test(&app, definition);
        for ((name, cursor_mode, start), rows) in
            asset_home_starts_for_test(definition, &wallet, &fi, incarnation)
                .into_iter()
                .zip(expected_rows)
        {
            let outcome =
                asset_home_signed_query_for_test(&app, &ALICE_KEYPAIR, start, cursor_mode).await;
            assert_asset_home_outcome_for_test(name, outcome, rows, "0").await;
        }
        // The executor's exact-owner gate is unchanged.
        for (name, cursor_mode, start) in
            asset_home_starts_for_test(definition, &wallet, &fi, incarnation)
        {
            let refused =
                asset_home_signed_query_for_test(&app, &stranger_key, start, cursor_mode).await;
            assert!(
                matches!(refused, Err(Error::Query(ValidationFail::NotPermitted(_)))),
                "{name}: a non-owner is refused by the native executor"
            );
        }
        // The exact wallet may read its own FI relationship.
        let [_, _, (name, cursor_mode, start), ..] =
            asset_home_starts_for_test(definition, &wallet, &fi, incarnation);
        let outcome = asset_home_signed_query_for_test(&app, &wallet_key, start, cursor_mode).await;
        assert_asset_home_outcome_for_test(name, outcome, 1, "0").await;
    }
    // A stored FI receipt and a populated FI context each exceed one 1 KiB
    // source row. Core still reads both at their home under Torii's policy.
    let incarnation = app
        .state
        .view()
        .world()
        .axt_asset_incarnations()
        .get(&domainless)
        .copied()
        .expect("genesis definitions carry a live incarnation");
    let ([receipt, context], policy) =
        seed_asset_home_fi_rows_for_test(&app, &domainless, &wallet, &fi, incarnation);
    let [roster_start, _, _, receipt_start, context_start, _] =
        asset_home_starts_for_test(&domainless, &wallet, &fi, incarnation);
    for ((name, cursor_mode, start), expected) in
        [(receipt_start, receipt), (context_start, context)]
    {
        let outcome =
            asset_home_signed_query_for_test(&app, &ALICE_KEYPAIR, start, cursor_mode).await;
        assert_eq!(
            assert_asset_home_outcome_for_test(name, outcome, 1, "0").await,
            iroha_data_model::query::QueryOutputBatchBoxTuple::from_batch(expected),
            "{name} returns the stored native rows"
        );
    }
    // A populated UID policy view also exceeds one source row, both in the
    // roster and in a wallet-selected read.
    let wallet_start = (
        "FindAssetUidPoliciesV1 for one wallet",
        None,
        asset_home_start_for_test!(iroha_data_model::query::asset_uid::FindAssetUidPoliciesV1 {
            asset_definition_id: domainless.clone(),
            uid: Some(policy.uid),
            wallet_account_id: Some(wallet.clone()),
        }),
    );
    for ((name, cursor_mode, start), wallet_product) in
        [(roster_start, false), (wallet_start, true)]
    {
        let outcome =
            asset_home_signed_query_for_test(&app, &ALICE_KEYPAIR, start, cursor_mode).await;
        let batch = assert_asset_home_outcome_for_test(name, outcome, 1, "0").await;
        let [iroha_data_model::query::QueryOutputBatchBox::AssetUidPolicyViewV1(views)] =
            batch.columns()
        else {
            panic!("{name} returns UID policy views");
        };
        assert_eq!(views[0].policy, policy, "{name} returns the stored policy");
        assert_eq!(
            views[0]
                .wallet_holdings
                .as_ref()
                .map(|holdings| &holdings.account_id),
            wallet_product.then_some(&wallet),
            "{name}"
        );
    }
    let incarnation = initialize_asset_escrow_scope_for_test(&app, &restricted_owned);
    for (name, cursor_mode, start) in
        asset_home_starts_for_test(&restricted_owned, &wallet, &fi, incarnation)
    {
        let response = asset_home_signed_query_for_test(&app, &ALICE_KEYPAIR, start, cursor_mode)
            .await
            .unwrap_or_else(IntoResponse::into_response);
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{name}");
        assert_eq!(
            torii_response_header(&response, "x-iroha-reject-code"),
            Some("permission_denied"),
            "{name}: a restricted home still needs its read grant"
        );
    }
    grant_account_permission_for_test(
        &app,
        &ALICE_ID,
        CanReadRestrictedDataspace {
            dataspace: restricted,
        }
        .into(),
    );
    for ((name, cursor_mode, start), rows) in
        asset_home_starts_for_test(&restricted_owned, &wallet, &fi, incarnation)
            .into_iter()
            .zip(expected_rows)
    {
        let outcome =
            asset_home_signed_query_for_test(&app, &ALICE_KEYPAIR, start, cursor_mode).await;
        assert_asset_home_outcome_for_test(name, outcome, rows, "10").await;
    }
}

/// Check one admitted Start: its rows, complete at its `home` route.
async fn assert_asset_home_outcome_for_test(
    name: &str,
    outcome: Result<Response, Error>,
    rows: usize,
    home: &str,
) -> iroha_data_model::query::QueryOutputBatchBoxTuple {
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
    assert_eq!(output.batch.len(), rows, "{name}");
    assert!(
        output.continue_cursor.is_none(),
        "{name} completes in its Start"
    );
    output.batch
}

/// Store one FI receipt and a populated FI context for `wallet` under their
/// native keys, whose spellings Core pins. Returns the batches Core reads back
/// and the stored UID policy.
fn seed_asset_home_fi_rows_for_test(
    app: &SharedAppState,
    asset_definition_id: &AssetDefinitionId,
    wallet: &AccountId,
    fi: &AccountId,
    incarnation: iroha_data_model::nexus::AxtAssetIncarnationV1,
) -> (
    [iroha_data_model::query::QueryOutputBatchBox; 2],
    iroha_data_model::asset::uid::AssetUidPolicyV1,
) {
    use iroha_data_model::{
        asset::{
            fi_context::AssetFiContextSnapshotV1,
            fi_link::{
                AssetFiLinkAttestationV1, AssetFiLinkV1, AssetFiTransferReceiptV1, AssetFiV1,
            },
            uid::{AssetUidBindingV1, AssetUidDailyWindowV1, AssetUidPolicyV1},
        },
        isi::{InstructionBox, asset_fi::TransferToLinkedFiV1},
        query::QueryOutputBatchBox,
    };
    let digest = |text: String| hex::encode(Hash::new(text).as_ref());
    let definition = digest(asset_definition_id.to_string());
    let account = digest(wallet.to_string());
    let fi_digest = digest(fi.to_string());
    let instruction = TransferToLinkedFiV1 {
        asset_definition_id: asset_definition_id.clone(),
        account_id: wallet.clone(),
        amount: Quantity::from(5_u32),
        expected_link_revision: 1,
        expected_policy_revision: 1,
    };
    let fi_row = AssetFiV1 {
        asset_definition_id: asset_definition_id.clone(),
        fi_account_id: fi.clone(),
        revision: 1,
        enabled: true,
    };
    let attestation = AssetFiLinkAttestationV1 {
        asset_definition_id: asset_definition_id.clone(),
        account_id: wallet.clone(),
        fi_account_id: fi.clone(),
        reference_digest: [9; 32],
        revision: 1,
        enabled: true,
    };
    let link = AssetFiLinkV1 {
        asset_definition_id: asset_definition_id.clone(),
        account_id: wallet.clone(),
        fi_account_id: fi.clone(),
        reference_digest: [9; 32],
        fi_revision: 1,
        attestation_revision: 1,
        revision: 1,
        enabled: true,
        controller: None,
        waterfall: false,
        reverse_waterfall: false,
    };
    let binding = AssetUidBindingV1 {
        asset_definition_id: asset_definition_id.clone(),
        account_id: wallet.clone(),
        uid: [7; 32],
    };
    let policy = AssetUidPolicyV1 {
        asset_definition_id: asset_definition_id.clone(),
        uid: [7; 32],
        revision: 1,
        accounts: std::collections::BTreeSet::from([wallet.clone()]),
        holding_limit: Some(Quantity::from(10_u32)),
        daily_window: AssetUidDailyWindowV1::UtcDay,
        daily_spend_limit: Some(Quantity::from(5_u32)),
        daily_outgoing_limit: Some(Quantity::from(7_u32)),
    };
    let receipt = AssetFiTransferReceiptV1 {
        asset_incarnation: incarnation,
        network: *app.state.network_id_ref(),
        execution_call_hash: Hash::new(b"torii::asset-home-receipt-call"),
        instruction_hash: Hash::new(
            norito::encode_canonical(&InstructionBox::from(instruction.clone()))
                .expect("canonical FI transfer"),
        ),
        instruction,
        authority: wallet.clone(),
        controller: None,
        fi: fi_row.clone(),
        attestation: attestation.clone(),
        link: link.clone(),
        uid: [7; 32],
        uid_policy_revision: 1,
        uid_policy_hash: Hash::new(b"torii::asset-home-uid-policy"),
        execution_height: 1,
        execution_time_ms: 1_000,
        transcript_index: 0,
    };
    let snapshot = AssetFiContextSnapshotV1 {
        asset_definition_id: asset_definition_id.clone(),
        asset_incarnation: incarnation,
        account_id: wallet.clone(),
        uid: [7; 32],
        fi_account_id: fi.clone(),
        reference_digest: [9; 32],
        binding: Some(binding.clone()),
        policy: Some(policy.clone()),
        fi: Some(fi_row.clone()),
        attestation: Some(attestation.clone()),
        link: Some(link.clone()),
        controller: None,
    };
    let source_bytes =
        usize::try_from(iroha_core::smartcontracts::isi::query::ORDINARY_NAME_ID_SOURCE_BYTES)
            .expect("source row bytes fit usize");
    for frame in [
        norito::encode_canonical(&receipt).expect("canonical FI receipt"),
        norito::encode_canonical(&snapshot).expect("canonical FI context"),
    ] {
        assert!(
            frame.len() > source_bytes,
            "each row exceeds one source row"
        );
    }
    let rows = [
        (
            format!(
                "sc/native/asset_fi_transfer_receipt_v1/{definition}/{account}/{}/0",
                hex::encode(receipt.execution_call_hash.as_ref())
            ),
            norito::encode_canonical(&receipt),
        ),
        (
            format!("asset_uid_binding_v1/{definition}/{account}"),
            norito::encode_canonical(&binding),
        ),
        (
            format!(
                "asset_uid_policy_v1/{definition}/{}",
                hex::encode([7_u8; 32])
            ),
            norito::encode_canonical(&policy),
        ),
        (
            format!("asset_fi_v1/{definition}/{fi_digest}"),
            norito::encode_canonical(&fi_row),
        ),
        (
            format!(
                "asset_fi_link_attestation_v1/{definition}/{account}/{fi_digest}/{}",
                hex::encode([9_u8; 32])
            ),
            norito::encode_canonical(&attestation),
        ),
        (
            format!("asset_fi_link_v1/{definition}/{account}"),
            norito::encode_canonical(&link),
        ),
    ];
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
    for (key, frame) in rows {
        tx.world_mut_for_testing()
            .smart_contract_state_mut_for_testing()
            .insert(
                key.parse().expect("native FI key"),
                frame.expect("canonical FI row"),
            );
    }
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit FI rows");
    (
        [
            QueryOutputBatchBox::AssetFiTransferReceiptV1(vec![receipt]),
            QueryOutputBatchBox::AssetFiContextSnapshotV1(vec![snapshot]),
        ],
        policy,
    )
}
