mod account_asset_visibility {
    //! Account balance reads preserve public ownership, exact targets, and live grants.

    use super::*;
    use iroha_data_model::{Identifiable as _, asset::AssetBalanceScope};
    use iroha_executor_data_model::permission::query::CanReadAccountData;

    struct Fixture {
        app: SharedAppState,
        target: AccountId,
        target_key: KeyPair,
        sibling: AccountId,
        reader: AccountId,
        reader_key: KeyPair,
        public: AssetDefinitionId,
        other_public: AssetDefinitionId,
        unscoped: AssetDefinitionId,
    }

    impl Fixture {
        fn new(public_dataspace_home: bool) -> Self {
            let target_key = checked_torii_test_ed25519_keypair(0xe1, "asset visibility target");
            let reader_key = checked_torii_test_ed25519_keypair(0xe2, "asset visibility reader");
            let target = AccountId::new(target_key.public_key().clone());
            let reader = AccountId::new(reader_key.public_key().clone());
            let sibling = checked_torii_test_account_id(0xe3, "asset visibility sibling");
            let domain = DomainId::try_new("universal", "universal").expect("public domain");
            let public = AssetDefinitionId::derive_from_components(
                domain.clone(),
                "public_protocol".parse().expect("asset name"),
            );
            let other_domain = DomainId::try_new(
                "treasury",
                if public_dataspace_home {
                    "governance"
                } else {
                    "universal"
                },
            )
            .expect("public domain");
            let other_public = AssetDefinitionId::derive_from_components(
                other_domain.clone(),
                "public_global".parse().expect("asset name"),
            );
            let unscoped = AssetDefinitionId::derive_from_components(
                domain.clone(),
                "unscoped_control".parse().expect("asset name"),
            );
            let world = World::with_assets(
                [
                    Domain::new(domain.clone()).build(&target),
                    Domain::new(other_domain.clone()).build(&target),
                ],
                [
                    Account::new(target.clone()).build(&target),
                    Account::new(sibling.clone()).build(&sibling),
                    Account::new(reader.clone()).build(&reader),
                ],
                [
                    AssetDefinition::numeric(
                        public.clone(),
                        "Public protocol asset",
                        AssetBalancePolicy::Global,
                        Some(domain),
                    )
                    .build(&target),
                    AssetDefinition::numeric(
                        other_public.clone(),
                        "Public DS1 global asset",
                        AssetBalancePolicy::Global,
                        Some(other_domain),
                    )
                    .build(&target),
                    AssetDefinition::numeric(
                        unscoped.clone(),
                        "Unscoped control",
                        AssetBalancePolicy::Global,
                        None,
                    )
                    .build(&target),
                ],
                [
                    Asset::new(
                        AssetId::new(public.clone(), target.clone()),
                        Quantity::from(10_u32),
                    ),
                    Asset::new(
                        AssetId::new(other_public.clone(), target.clone()),
                        Quantity::from(30_u32),
                    ),
                    Asset::new(
                        AssetId::new(unscoped.clone(), target.clone()),
                        Quantity::from(20_u32),
                    ),
                    Asset::new(
                        AssetId::new(unscoped.clone(), sibling.clone()),
                        Quantity::from(99_u32),
                    ),
                ],
                [],
            );
            let app = if public_dataspace_home {
                let mut app = mk_app_state_for_tests_with_world_and_nexus(
                    world,
                    private_ingress_nexus_for_test(),
                );
                configure_private_ingress_routes_for_test(&mut app);
                app
            } else {
                let mut app = mk_app_state_for_tests_with_world(world);
                configure_account_asset_routes_for_test(&mut app);
                app
            };
            Self {
                app,
                target,
                target_key,
                sibling,
                reader,
                reader_key,
                public,
                other_public,
                unscoped,
            }
        }

        async fn read(&self, caller: Option<(&AccountId, &KeyPair)>) -> Value {
            let uri: Uri = format!("/v1/accounts/{}/assets", self.target)
                .parse()
                .expect("URI");
            let headers = caller.map_or_else(HeaderMap::new, |(account, key)| {
                signed_network_app_headers(
                    self.app.state.network_id_ref(),
                    account,
                    key,
                    &Method::GET,
                    &uri,
                    &[],
                )
            });
            let response = handler_account_assets(
                State(self.app.clone()),
                Method::GET,
                uri,
                headers,
                crate::loopback_connect_info(),
                AxPath(self.target.to_string()),
                AxQuery(crate::routing::AccountAssetsGetParams::default()),
            )
            .await
            .expect("account assets handler")
            .into_response();
            let status = response.status();
            let body = torii_body_bytes(response, "account assets body").await;
            assert_eq!(
                status,
                StatusCode::OK,
                "account assets response: {}",
                String::from_utf8_lossy(&body),
            );
            norito::json::from_slice(&body).expect("account assets JSON")
        }

        fn assert_rows(&self, response: &Value, include_unscoped: bool) {
            let rows = response["items"].as_array().expect("asset rows");
            let mut expected_assets =
                BTreeSet::from([self.public.to_string(), self.other_public.to_string()]);
            if include_unscoped {
                expected_assets.insert(self.unscoped.to_string());
            }
            assert_eq!(rows.len(), expected_assets.len());
            for row in rows {
                assert_eq!(
                    row["account_id"].as_str(),
                    Some(self.target.to_string().as_str())
                );
                let asset = row["asset"].as_str().expect("asset ID");
                assert!(
                    expected_assets.remove(asset),
                    "unexpected or duplicate asset {asset}"
                );
                let expected_quantity = if asset == self.public.to_string() {
                    "10"
                } else if asset == self.other_public.to_string() {
                    "30"
                } else {
                    assert!(include_unscoped);
                    assert_eq!(asset, self.unscoped.to_string());
                    "20"
                };
                assert_eq!(row["quantity"].as_str(), Some(expected_quantity));
                assert_eq!(row["scope"].as_str(), Some("global"));
            }
            assert!(expected_assets.is_empty());
        }
    }

    #[tokio::test]
    async fn account_assets_get_preserves_public_ownership_and_rechecks_global_reader_revocation() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let fixture = Fixture::new(false);
        for caller in [
            None,
            Some((&fixture.target, &fixture.target_key)),
            Some((&fixture.reader, &fixture.reader_key)),
        ] {
            fixture.assert_rows(&fixture.read(caller).await, false);
        }
        grant_account_permission_for_test(
            &fixture.app,
            &fixture.reader,
            CanReadAccountData {
                account: fixture.target.clone(),
            }
            .into(),
        );
        fixture.assert_rows(
            &fixture
                .read(Some((&fixture.reader, &fixture.reader_key)))
                .await,
            false,
        );

        let global_permission: Permission = CanReadAllLedgerData.into();
        grant_account_permission_for_test(&fixture.app, &fixture.reader, global_permission.clone());
        fixture.assert_rows(
            &fixture
                .read(Some((&fixture.reader, &fixture.reader_key)))
                .await,
            true,
        );

        let next_height = fixture
            .app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let mut block = fixture.app.state.block(BlockHeader::new(
            NonZeroU64::new(next_height).expect("height"),
            None,
            None,
            0,
            0,
        ));
        let mut tx = block.transaction();
        assert!(
            tx.world_mut_for_testing()
                .remove_account_permission(&fixture.reader, &global_permission)
        );
        tx.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit global read revocation");
        fixture.assert_rows(
            &fixture
                .read(Some((&fixture.reader, &fixture.reader_key)))
                .await,
            false,
        );
    }

    #[test]
    fn exact_account_global_visibility_never_opens_sibling_balances() {
        let fixture = Fixture::new(false);
        let world = fixture.app.state.world_view();
        let target_asset = AssetId::new(fixture.unscoped.clone(), fixture.target.clone());
        let sibling_asset = AssetId::new(fixture.unscoped.clone(), fixture.sibling.clone());
        for can_read_all in [false, true] {
            let visibility = crate::routing::DataspaceReadVisibility::exact_account(
                BTreeSet::from([DataSpaceId::UNIVERSAL]),
                DataSpaceId::UNIVERSAL,
                fixture.target.clone(),
                can_read_all,
            );
            assert_eq!(visibility.can_read_all(), can_read_all);
            assert_eq!(visibility.exact_account_id(), Some(&fixture.target));
            assert_eq!(visibility.allows_asset(&world, &target_asset), can_read_all);
            assert!(!visibility.allows_account(&world, &fixture.sibling));
            assert!(!visibility.allows_asset(&world, &sibling_asset));
        }
    }

    #[tokio::test]
    async fn account_asset_definition_visibility_is_independent_of_its_global_balance_route() {
        let fixture = Fixture::new(true);
        let universal_route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        for (caller, grant, global) in [
            (&fixture.target, None, false),
            (&fixture.reader, None, false),
            (
                &fixture.reader,
                Some(Permission::from(CanReadAccountData {
                    account: fixture.target.clone(),
                })),
                false,
            ),
            (
                &fixture.reader,
                Some(Permission::from(CanReadAllLedgerData)),
                true,
            ),
        ] {
            if let Some(grant) = grant {
                grant_account_permission_for_test(&fixture.app, caller, grant);
            }
            let scope = ToriiFanoutRouteScopeV1::TargetAccount {
                account_id: fixture.target.to_string(),
                caller_account_id: Some(caller.to_string()),
            };
            let visibility = torii_account_assets_route_visibility(
                fixture.app.as_ref(),
                &scope,
                &fixture.target.to_string(),
                universal_route,
            )
            .expect("authorized exact-account producer");
            assert_eq!(visibility.can_read_all(), global);
            let response = crate::routing::handle_v1_account_assets_with_visibility(
                fixture.app.state.clone(),
                AxPath(fixture.target.to_string()),
                AxQuery(crate::routing::AccountAssetsGetParams::default()),
                fixture.app.telemetry_handle(),
                visibility.clone(),
            )
            .await
            .expect("global balances GET")
            .into_response();
            let response =
                decode_torii_json(response, "global balances body", "global balances JSON").await;
            fixture.assert_rows(&response, global);
            let query = crate::routing::handle_v1_account_assets_query_with_visibility(
                fixture.app.state.clone(),
                AxPath(fixture.target.to_string()),
                NoritoJson(crate::filter::QueryEnvelope::default()),
                fixture.app.telemetry_handle(),
                visibility,
            )
            .await
            .expect("global balances query")
            .into_response();
            let query = decode_torii_json(
                query,
                "global balances query body",
                "global balances query JSON",
            )
            .await;
            fixture.assert_rows(&query, global);

            let definition_home_visibility = torii_account_assets_route_visibility(
                fixture.app.as_ref(),
                &scope,
                &fixture.target.to_string(),
                RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1)),
            )
            .expect("authorized definition-home producer");
            let world = fixture.app.state.world_view();
            assert!(
                definition_home_visibility.allows_asset_definition(&world, &fixture.other_public)
            );
            assert!(!definition_home_visibility.allows_asset(
                &world,
                &AssetId::new(fixture.other_public.clone(), fixture.target.clone()),
            ));
        }
    }

    #[tokio::test]
    async fn exact_account_global_reader_keeps_each_producer_within_its_balance_dataspace() {
        let target = checked_torii_test_account_id(0xe5, "routed asset visibility target");
        let owner_domain = DomainId::try_new("vault", "restricted").expect("owner domain");
        let restricted = AssetDefinitionId::derive_from_components(
            owner_domain.clone(),
            "partitioned".parse().expect("name"),
        );
        let unscoped = AssetDefinitionId::derive_from_components(
            owner_domain.clone(),
            "unscoped".parse().expect("name"),
        );
        let buckets = [
            AssetId::new(unscoped.clone(), target.clone()),
            AssetId::with_scope(
                restricted.clone(),
                target.clone(),
                AssetBalanceScope::Dataspace(DataSpaceId::new(10)),
            ),
            // Balance authority follows this bucket, even though its definition belongs to DS10.
            AssetId::with_scope(
                restricted.clone(),
                target.clone(),
                AssetBalanceScope::Dataspace(DataSpaceId::new(1)),
            ),
        ];
        let world = World::with_assets(
            [Domain::new(owner_domain.clone()).build(&target)],
            [Account::new(target.clone()).build(&target)],
            [
                AssetDefinition::numeric(
                    restricted.clone(),
                    "Partitioned",
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(owner_domain),
                )
                .build(&target),
                AssetDefinition::numeric(unscoped, "Unscoped", AssetBalancePolicy::Global, None)
                    .build(&target),
            ],
            buckets
                .iter()
                .cloned()
                .map(|id| Asset::new(id, Quantity::from(1_u32))),
            [],
        );
        let app =
            mk_app_state_for_tests_with_world_and_nexus(world, private_ingress_nexus_for_test());
        for (index, dataspace) in [
            DataSpaceId::UNIVERSAL,
            DataSpaceId::new(10),
            DataSpaceId::new(1),
        ]
        .into_iter()
        .enumerate()
        {
            let visibility = crate::routing::DataspaceReadVisibility::exact_account(
                BTreeSet::from([
                    DataSpaceId::UNIVERSAL,
                    DataSpaceId::new(1),
                    DataSpaceId::new(10),
                ]),
                dataspace,
                target.clone(),
                true,
            );
            {
                let world = app.state.world_view();
                assert!(visibility.allows_asset_definition(&world, &restricted));
                for (bucket_index, bucket) in buckets.iter().enumerate() {
                    assert_eq!(
                        visibility.allows_asset(&world, bucket),
                        bucket_index == index
                    );
                }
            }
            let response = crate::routing::handle_v1_account_assets_with_visibility(
                app.state.clone(),
                AxPath(target.to_string()),
                AxQuery(crate::routing::AccountAssetsGetParams::default()),
                app.telemetry_handle(),
                visibility.clone(),
            )
            .await
            .expect("routed assets")
            .into_response();
            let response =
                decode_torii_json(response, "routed assets body", "routed assets JSON").await;
            let rows = response["items"].as_array().expect("routed asset rows");
            assert_eq!(
                rows.len(),
                1,
                "producer for {dataspace} must emit only its bucket"
            );
            assert_eq!(
                rows[0]["asset"].as_str(),
                Some(buckets[index].definition().to_string().as_str())
            );
            assert_eq!(rows[0]["quantity"].as_str(), Some("1"));
            assert_eq!(
                rows[0]["scope"].as_str(),
                Some(crate::routing::asset_balance_scope_literal(buckets[index].scope()).as_str()),
            );
            let query_response = crate::routing::handle_v1_account_assets_query_with_visibility(
                app.state.clone(),
                AxPath(target.to_string()),
                NoritoJson(crate::filter::QueryEnvelope::default()),
                app.telemetry_handle(),
                visibility,
            )
            .await
            .expect("routed assets query")
            .into_response();
            let query_response = decode_torii_json(
                query_response,
                "routed assets query body",
                "routed assets query JSON",
            )
            .await;
            assert_eq!(query_response["items"], response["items"]);
        }
    }

    #[test]
    fn public_asset_visibility_rejects_restricted_and_unknown_owning_dataspaces() {
        let target = checked_torii_test_account_id(0xe4, "private asset visibility target");
        let mut domains = Vec::new();
        let mut definitions = Vec::new();
        let mut assets = Vec::new();
        for (dataspace_alias, dataspace) in [
            ("restricted", DataSpaceId::new(10)),
            ("unknown", DataSpaceId::new(11)),
        ] {
            let domain = DomainId::try_new("vault", dataspace_alias).expect("domain");
            let definition = AssetDefinitionId::derive_from_components(
                domain.clone(),
                "unit".parse().expect("name"),
            );
            domains.push(Domain::new(domain.clone()).build(&target));
            definitions.push(
                AssetDefinition::numeric(
                    definition.clone(),
                    "Private asset",
                    AssetBalancePolicy::DataspaceRestricted,
                    Some(domain),
                )
                .build(&target),
            );
            assets.push(Asset::new(
                AssetId::with_scope(
                    definition,
                    target.clone(),
                    AssetBalanceScope::Dataspace(dataspace),
                ),
                Quantity::from(1_u32),
            ));
        }
        let world = World::with_assets(
            domains,
            [Account::new(target.clone()).build(&target)],
            definitions,
            assets.clone(),
            [],
        );
        let app =
            mk_app_state_for_tests_with_world_and_nexus(world, private_ingress_nexus_for_test());
        let world = app.state.world_view();
        assert_eq!(
            world
                .dataspace_catalog()
                .by_alias("restricted")
                .map(|entry| entry.id),
            Some(DataSpaceId::new(10)),
        );
        assert!(world.dataspace_catalog().by_alias("unknown").is_none());
        for visibility in [
            crate::routing::DataspaceReadVisibility::new(
                BTreeSet::from([DataSpaceId::UNIVERSAL]),
                false,
            ),
            crate::routing::DataspaceReadVisibility::exact_account(
                BTreeSet::from([DataSpaceId::UNIVERSAL]),
                DataSpaceId::UNIVERSAL,
                target,
                false,
            ),
        ] {
            for asset in &assets {
                assert!(!visibility.allows_asset_definition(&world, asset.id().definition()));
                assert!(!visibility.allows_asset(&world, asset.id()));
            }
        }
    }
}
