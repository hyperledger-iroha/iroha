#[cfg(all(test, feature = "app_api"))]
#[allow(clippy::await_holding_lock)]
mod app_api_integration_tests {
    use super::*;
    use crate::tests_runtime_handlers::mk_app_state_for_tests;
    use axum::{Router, routing::post};
    use http_body_util::BodyExt as _;
    use iroha_core::{
        block::{BlockBuilder, ValidBlock},
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute as _,
        state::{State, World},
        sumeragi::network_topology::Topology,
    };
    use norito::json;
    use std::borrow::Cow;
    use std::sync::{LazyLock, Mutex, MutexGuard};
    use tower::ServiceExt;
    static APP_QUERY_LIMITS_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    fn app_query_limits_guard() -> MutexGuard<'static, ()> {
        APP_QUERY_LIMITS_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    struct AppQueryLimitsOverride {
        _guard: MutexGuard<'static, ()>,
        previous: AppQueryLimits,
    }
    impl AppQueryLimitsOverride {
        fn new(limits: AppQueryLimits) -> Self {
            let guard = app_query_limits_guard();
            let previous = app_query_limits();
            set_app_query_limits(limits);
            Self {
                _guard: guard,
                previous,
            }
        }
    }
    impl Drop for AppQueryLimitsOverride {
        fn drop(&mut self) {
            set_app_query_limits(self.previous);
        }
    }
    #[test]
    fn manifest_fanout_window_is_limited_to_one_configured_page() {
        let _limits = AppQueryLimitsOverride::new(AppQueryLimits::new(1, 3, 10, 1));
        assert_eq!(
            space_directory_manifest_pagination(Some(2), 2)
                .expect("the direct route may use the larger fetch window"),
            (2, 2),
        );
        let error = space_directory_manifest_fanout_window(2, 2)
            .expect_err("fanout currently requires one shard page for the global prefix");
        assert!(matches!(
            error,
            Error::AppQueryValidation {
                code: "invalid_pagination",
                ..
            }
        ));
    }
    fn obj(pairs: Vec<(&'static str, Value)>) -> Value {
        crate::json_object(pairs)
    }
    fn arr(values: Vec<Value>) -> Value {
        crate::json_array(values)
    }
    fn val<T: json::JsonSerialize + ?Sized>(value: &T) -> Value {
        crate::json_value(value)
    }
    fn checked_app_api_keypair(
        seed: u8,
        algorithm: iroha_crypto::Algorithm,
        context: &'static str,
    ) -> iroha_crypto::KeyPair {
        checked_routing_fixture_keypair(seed, algorithm, context)
    }
    fn checked_app_api_account_id(seed: u8, context: &'static str) -> AccountId {
        AccountId::new(
            checked_app_api_keypair(seed, iroha_crypto::Algorithm::Ed25519, context)
                .public_key()
                .clone(),
        )
    }
    fn state_with_assets(
        domain_id: DomainId,
        authority: AccountId,
        accounts: Vec<AccountId>,
        asset_definitions: Vec<(AssetDefinitionId, String)>,
        assets: Vec<Asset>,
    ) -> Arc<State> {
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let accounts: Vec<Account> = accounts
            .into_iter()
            .map(|id| Account::new(id.clone()).build(&authority))
            .collect();
        let asset_definitions: Vec<AssetDefinition> = asset_definitions
            .into_iter()
            .map(|(id, name)| {
                AssetDefinition::numeric(
                    id,
                    name,
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
                .build(&authority)
            })
            .collect();
        let world = World::with_assets([domain], accounts, asset_definitions, assets, []);
        Arc::new(iroha_core::state::State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ))
    }
    #[test]
    fn collect_projected_account_assets_reads_only_scoped_account_assets() {
        let _guard = app_query_limits_guard();
        let alice_id =
            checked_app_api_account_id(0x75, "derive projected account assets Alice fixture key");
        let bob_id =
            checked_app_api_account_id(0x76, "derive projected account assets Bob fixture key");
        let domain_id = DomainId::try_new("wonderland", "universal").unwrap();
        let rose_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "rose".parse().unwrap());
        let lily_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "lily".parse().unwrap());
        let assets = vec![
            Asset::new(
                AssetId::new(rose_def.clone(), alice_id.clone()),
                Quantity::from(10_u32),
            ),
            Asset::new(
                AssetId::new(lily_def.clone(), alice_id.clone()),
                Quantity::from(7_u32),
            ),
            Asset::new(
                AssetId::new(rose_def.clone(), bob_id.clone()),
                Quantity::from(99_u32),
            ),
        ];
        let state = state_with_assets(
            domain_id,
            alice_id.clone(),
            vec![alice_id.clone(), bob_id],
            vec![
                (rose_def.clone(), "rose".to_owned()),
                (lily_def, "lily".to_owned()),
            ],
            assets,
        );
        let world = state.world_view();
        let scoped_accounts = vec![alice_id.clone()];
        let projected = collect_projected_account_assets(
            state.as_ref(),
            &world,
            &alice_id,
            &scoped_accounts,
            Some(&rose_def),
            None,
        );
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].account_id, alice_id.to_string());
        assert_eq!(projected[0].asset, rose_def.to_string());
        assert_eq!(projected[0].quantity, Quantity::from(10_u32));
    }
    #[test]
    fn accumulate_asset_holder_quantity_respects_scope_filter() {
        let account_id =
            checked_app_api_account_id(0x77, "derive asset holder quantity fixture account key");
        let domain_id = DomainId::try_new("wonderland", "universal").unwrap();
        let asset_def =
            AssetDefinitionId::derive_from_components(domain_id, "rose".parse().unwrap());
        let global_asset_id = AssetId::new(asset_def.clone(), account_id.clone());
        let scoped_asset_id = AssetId::with_scope(
            asset_def,
            account_id.clone(),
            AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
        );
        let scope_filter = AssetBalanceScope::Global;
        let mut map = BTreeMap::new();
        accumulate_asset_holder_quantity(
            &mut map,
            &global_asset_id,
            &Quantity::from(10_u32),
            Some(&scope_filter),
        )
        .expect("global holder quantity accumulation");
        accumulate_asset_holder_quantity(
            &mut map,
            &scoped_asset_id,
            &Quantity::from(99_u32),
            Some(&scope_filter),
        )
        .expect("filtered holder quantity accumulation");
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get(&(account_id, AssetBalanceScope::Global)),
            Some(&Quantity::from(10_u32))
        );
    }
    #[test]
    fn explorer_circulating_quantity_rejects_inconsistent_locked_supply() {
        use iroha_primitives::numeric::Quantity;
        assert_eq!(
            explorer_circulating_quantity(&Quantity::from(100_u32), &Quantity::from(40_u32))
                .expect("locked supply is within total"),
            Quantity::from(60_u32)
        );
        assert!(
            explorer_circulating_quantity(&Quantity::from(100_u32), &Quantity::from(101_u32))
                .is_err()
        );
    }
    #[test]
    fn asset_holder_filter_account_candidates_extracts_safe_exact_constraints() {
        let alice_id =
            checked_app_api_account_id(0x78, "derive asset holder candidate Alice fixture key");
        let bob_id =
            checked_app_api_account_id(0x79, "derive asset holder candidate Bob fixture key");
        let expr = FilterExpr::And(vec![
            FilterExpr::In(
                FieldPath("account_id".into()),
                vec![
                    Value::from(alice_id.to_string()),
                    Value::from(bob_id.to_string()),
                ],
            ),
            FilterExpr::Eq(
                FieldPath("account_id".into()),
                Value::from(alice_id.to_string()),
            ),
            FilterExpr::Gt(FieldPath("quantity".into()), Value::from(1_u64)),
        ]);
        let candidates = asset_holder_filter_account_candidates(Some(&expr)).unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates.contains(&alice_id));
        let unsafe_or = FilterExpr::Or(vec![
            FilterExpr::Eq(
                FieldPath("account_id".into()),
                Value::from(alice_id.to_string()),
            ),
            FilterExpr::Gt(FieldPath("quantity".into()), Value::from(1_u64)),
        ]);
        assert!(asset_holder_filter_account_candidates(Some(&unsafe_or)).is_none());
    }
    #[tokio::test]
    async fn tx_query_empty_ok() {
        let _guard = app_query_limits_guard();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/accounts/{account_id}/transactions/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_transactions(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::for_tests(),
                    )
                    .await
                }
            }),
        );
        let req_body = json_string(crate::json_object(vec![
            json_entry("filter", Value::Null),
            json_entry(
                "pagination",
                crate::json_object(vec![json_entry("limit", 10u64)]),
            ),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/transactions/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(req_body))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        assert_eq!(json["total"].as_u64(), Some(0));
        assert!(json["items"].as_array().unwrap().is_empty());
    }
    #[tokio::test]
    async fn tx_query_sorted_total_counts_matches() {
        let _guard = app_query_limits_guard();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::default(),
            kura.clone(),
            query,
        ));
        // Build three transactions with distinct timestamps
        let network_id = *state.network_id_ref();
        let kp_a = checked_app_api_keypair(
            0x7A,
            iroha_crypto::Algorithm::Ed25519,
            "derive sorted transaction count account A fixture key",
        );
        let kp_b = checked_app_api_keypair(
            0x7B,
            iroha_crypto::Algorithm::Ed25519,
            "derive sorted transaction count account B fixture key",
        );
        let acc_a = AccountId::new(kp_a.public_key().clone());
        let acc_b = AccountId::new(kp_b.public_key().clone());
        let account_literal = acc_b.to_string();
        let (_max_clock_drift, _tx_limits) = {
            let v = state.view();
            let p = v.world().parameters();
            (p.sumeragi().max_clock_drift(), p.transaction())
        };
        let mut b1 = TransactionBuilder::new(
            network_id,
            acc_a.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        b1.set_creation_time(core::time::Duration::from_millis(1000));
        let tx1 = b1
            .with_instructions::<InstructionBox>([])
            .sign(kp_a.private_key());
        let tx1 = AcceptedTransaction::new_unchecked(Cow::Owned(tx1));
        let mut b2 = TransactionBuilder::new(
            network_id,
            acc_b.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        b2.set_creation_time(core::time::Duration::from_millis(2000));
        let tx2 = b2
            .with_instructions::<InstructionBox>([])
            .sign(kp_b.private_key());
        let tx2 = AcceptedTransaction::new_unchecked(Cow::Owned(tx2));
        let mut b3 = TransactionBuilder::new(
            network_id,
            acc_b.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        b3.set_creation_time(core::time::Duration::from_millis(3000));
        let tx3 = b3
            .with_instructions::<InstructionBox>([])
            .sign(kp_b.private_key());
        let tx3 = AcceptedTransaction::new_unchecked(Cow::Owned(tx3));
        // Commit
        let leader = checked_app_api_keypair(
            0x7C,
            iroha_crypto::Algorithm::BlsNormal,
            "derive sorted transaction count block leader fixture key",
        );
        let _topo = Topology::new(vec![PeerId::new(leader.public_key().clone())]);
        let unverified = BlockBuilder::new(vec![tx1, tx2, tx3])
            .chain(0, state.view().latest_block().as_deref())
            .sign(leader.private_key())
            .unpack(|_| {});
        let mut st_block = state.block(unverified.header());
        let valid: ValidBlock = unverified
            .validate_and_record_transactions(&mut st_block)
            .unpack(|_| {});
        let committed = valid.clone().commit_unchecked().unpack(|_| {});
        crate::test_utils::finalize_committed_block(&state, st_block, committed);
        // Sorted by timestamp, request middle page (offset=1, limit=1)
        let env = crate::filter::QueryEnvelope {
            query: None,
            filter: None,
            select: None,
            aggregate: None,
            sort: vec![crate::filter::SortKey {
                key: crate::filter::FieldPath("timestamp_ms".into()),
                order: crate::filter::Order::Asc,
            }],
            pagination: crate::filter::Pagination {
                limit: Some(1),
                offset: 1,
            },
            fetch_size: None,
            count_mode: Some("exact".to_owned()),
        };
        let resp = handle_v1_account_transactions(
            state.clone(),
            axum::extract::Path(account_literal),
            crate::utils::extractors::NoritoJson(env),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok")
        .into_response();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        assert_eq!(json["total"].as_u64(), Some(2));
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
        assert_eq!(json["items"][0]["timestamp_ms"].as_u64(), Some(3000));
    }
    #[tokio::test]
    async fn tx_query_rejects_invalid_authority_value() {
        let _guard = app_query_limits_guard();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/accounts/{account_id}/transactions/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_transactions(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::for_tests(),
                    )
                    .await
                }
            }),
        );
        let body = json_string(obj(vec![
            (
                "filter",
                obj(vec![
                    ("op", val("eq")),
                    (
                        "args",
                        arr(vec![val("authority"), val("not-an-account-id")]),
                    ),
                ]),
            ),
            ("pagination", obj(vec![("limit", val(&10u64))])),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/transactions/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    }
    #[tokio::test]
    async fn tx_query_rejects_invalid_entrypoint_hash_value() {
        let _guard = app_query_limits_guard();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/accounts/{account_id}/transactions/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_transactions(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::for_tests(),
                    )
                    .await
                }
            }),
        );
        // Not-hex
        let body = json_string(obj(vec![(
            "filter",
            obj(vec![
                ("op", val("eq")),
                ("args", arr(vec![val("entrypoint_hash"), val("not-a-hex")])),
            ]),
        )]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/transactions/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
        // Wrong length
        let body2 = json_string(obj(vec![(
            "filter",
            obj(vec![
                ("op", val("eq")),
                ("args", arr(vec![val("entrypoint_hash"), val("abcd")])),
            ]),
        )]));
        let req2 = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/transactions/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body2))
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    }
    #[tokio::test]
    async fn tx_query_rejects_excessive_set_size_and_depth() {
        let _guard = app_query_limits_guard();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/accounts/{account_id}/transactions/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_transactions(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::for_tests(),
                    )
                    .await
                }
            }),
        );
        // Build a large IN set (> 256)
        let mut set = Vec::new();
        for _ in 0..300 {
            set.push(norito::json::Value::String(
                "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".into(),
            ));
        }
        let body = json_string(obj(vec![(
            "filter",
            obj(vec![
                ("op", val("in")),
                ("args", arr(vec![val("authority"), arr(set)])),
            ]),
        )]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/transactions/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
        // Excessive nesting depth (> 10)
        fn nest(mut inner: Value, depth: usize) -> Value {
            for _ in 0..depth {
                inner = obj(vec![("op", val("not")), ("args", arr(vec![inner]))]);
            }
            inner
        }
        let base = obj(vec![("op", val("exists")), ("args", val("authority"))]);
        let deep = nest(base, 12);
        let body2 = json_string(obj(vec![("filter", deep)]));
        let req2 = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/transactions/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body2))
            .unwrap();
        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    }
    #[tokio::test]
    async fn tx_query_rejects_invalid_operator_for_authority() {
        let _guard = app_query_limits_guard();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/accounts/{account_id}/transactions/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_transactions(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::for_tests(),
                    )
                    .await
                }
            }),
        );
        // Attempt to use a numeric comparison on a string field: authority < 1
        let body = json_string(obj(vec![(
            "filter",
            obj(vec![
                ("op", val("lt")),
                ("args", arr(vec![val("authority"), val(&1u64)])),
            ]),
        )]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/transactions/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    }
    #[tokio::test]
    async fn account_assets_query_rejects_invalid_operator_for_quantity() {
        let _guard = app_query_limits_guard();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/accounts/{account_id}/assets/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_assets_query(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::disabled(),
                    )
                    .await
                }
            }),
        );
        // Attempt to compare quantity using a string
        let body = json_string(obj(vec![(
            "filter",
            obj(vec![
                ("op", val("lt")),
                ("args", arr(vec![val("quantity"), val("x")])),
            ]),
        )]));
        // For invalid filter tests, the specific account id is irrelevant
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/assets/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    }
    #[tokio::test]
    async fn asset_holders_query_rejects_invalid_operator_for_account_id() {
        let _guard = app_query_limits_guard();
        let (state, _, _) = build_asset_holder_fixture_state();
        let telemetry = MaybeTelemetry::disabled();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders/query",
            post({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    handle_v1_asset_holders_query(
                        state,
                        axum::extract::Path(def_id),
                        crate::utils::extractors::NoritoJson(env),
                        telemetry,
                    )
                    .await
                }
            }),
        );
        // Attempt to compare account_id numerically
        let body = json_string(obj(vec![(
            "filter",
            obj(vec![
                ("op", val("lt")),
                ("args", arr(vec![val("account_id"), val(&1u64)])),
            ]),
        )]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/assets/rose%23centralbank/holders/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    }
    #[tokio::test]
    async fn account_assets_query_pagination_preserves_total() {
        let _guard = app_query_limits_guard();
        let alice_id = checked_app_api_account_id(
            0x7D,
            "derive account assets query pagination fixture account key",
        );
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let rose_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "rose".parse().unwrap());
        let lily_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "lily".parse().unwrap());
        let assets = vec![
            Asset::new(
                AssetId::new(rose_def.clone(), alice_id.clone()),
                Quantity::from(10_u32),
            ),
            Asset::new(
                AssetId::new(lily_def.clone(), alice_id.clone()),
                Quantity::from(7_u32),
            ),
        ];
        let state = state_with_assets(
            domain_id,
            alice_id.clone(),
            vec![alice_id.clone()],
            vec![(rose_def, "rose".to_owned()), (lily_def, "lily".to_owned())],
            assets,
        );
        // Route under test
        let app = Router::new().route(
            "/v1/accounts/{account_id}/assets/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_assets_query(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::disabled(),
                    )
                    .await
                }
            }),
        );
        // Ask for only 1 item, expect total to be 2
        let body = json_string(obj(vec![
            ("filter", Value::Null),
            ("pagination", obj(vec![("limit", val(&1u64))])),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri(&format!("/v1/accounts/{}/assets/query", alice_id))
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        assert_eq!(json["total"].as_u64(), Some(2));
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn account_assets_query_sort_by_quantity_desc() {
        let _guard = app_query_limits_guard();
        let alice_id = checked_app_api_account_id(
            0x7E,
            "derive account assets query sort fixture account key",
        );
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let rose_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "rose".parse().unwrap());
        let lily_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "lily".parse().unwrap());
        let assets = vec![
            Asset::new(
                AssetId::new(rose_def.clone(), alice_id.clone()),
                Quantity::from(10_u32),
            ),
            Asset::new(
                AssetId::new(lily_def.clone(), alice_id.clone()),
                Quantity::from(7_u32),
            ),
        ];
        let state = state_with_assets(
            domain_id,
            alice_id.clone(),
            vec![alice_id.clone()],
            vec![(rose_def, "rose".to_owned()), (lily_def, "lily".to_owned())],
            assets,
        );
        let app = Router::new().route(
            "/v1/accounts/{account_id}/assets/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_assets_query(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::disabled(),
                    )
                    .await
                }
            }),
        );
        let body = json_string(obj(vec![
            ("filter", Value::Null),
            (
                "sort",
                arr(vec![obj(vec![
                    ("key", val("quantity")),
                    ("order", val("desc")),
                ])]),
            ),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri(format!("/v1/accounts/{}/assets/query", alice_id))
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        let first_qty = items[0]["quantity"].as_str().unwrap();
        let second_qty = items[1]["quantity"].as_str().unwrap();
        assert_eq!(first_qty, "10");
        assert_eq!(second_qty, "7");
    }
    #[tokio::test]
    async fn account_assets_query_rejects_limit_above_max_config() {
        let _limits = AppQueryLimitsOverride::new(AppQueryLimits::new(1, 3, 10, 1));
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/accounts/{account_id}/assets/query",
            post({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_account_assets_query(
                        state,
                        axum::extract::Path(account_id),
                        crate::utils::extractors::NoritoJson(env),
                        crate::routing::MaybeTelemetry::for_tests(),
                    )
                    .await
                }
            }),
        );
        let body = json_string(obj(vec![
            ("filter", Value::Null),
            ("pagination", obj(vec![("limit", val(&10u64))])),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/accounts/sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE/assets/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }
    #[tokio::test]
    async fn domains_query_respects_desc_sort() {
        let _guard = app_query_limits_guard();
        let alice_id =
            checked_app_api_account_id(0x7F, "derive domains query fixture authority key");
        let alpha: DomainId = DomainId::try_new("alpha", "universal").unwrap();
        let omega: DomainId = DomainId::try_new("omega", "universal").unwrap();
        let gamma: DomainId = DomainId::try_new("gamma", "universal").unwrap();
        let domain_alpha = Domain::new(alpha.clone()).build(&alice_id);
        let domain_omega = Domain::new(omega).build(&alice_id);
        let domain_gamma = Domain::new(gamma).build(&alice_id);
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let world = World::with(
            [domain_alpha, domain_omega, domain_gamma],
            [account],
            Vec::<AssetDefinition>::new(),
        );
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/domains/query",
            post({
                let state = state.clone();
                move |crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    handle_v1_domains_query(state.clone(), crate::utils::extractors::NoritoJson(env))
                        .await
                }
            }),
        );
        let body = json_string(obj(vec![
            ("filter", Value::Null),
            (
                "sort",
                arr(vec![obj(vec![("key", val("id")), ("order", val("desc"))])]),
            ),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/domains/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        let items = json["items"].as_array().unwrap();
        assert!(
            items.len() >= 3,
            "expected at least three domains in response"
        );
        let ids: Vec<_> = items
            .iter()
            .map(|item| item["id"].as_str().unwrap().to_string())
            .collect();
        assert!(
            ids.windows(2).all(|w| w[0] >= w[1]),
            "domain ids are not sorted descending: {ids:?}"
        );
    }
    #[tokio::test]
    async fn asset_holders_query_pagination_preserves_total() {
        let _guard = app_query_limits_guard();
        let (state, _, _) = build_asset_holder_fixture_state();
        // Route under test
        let telemetry = MaybeTelemetry::disabled();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders/query",
            post({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    handle_v1_asset_holders_query(
                        state,
                        axum::extract::Path(def_id),
                        crate::utils::extractors::NoritoJson(env),
                        telemetry,
                    )
                    .await
                }
            }),
        );
        // Ask for only 1 item, expect total to be 2
        let body = json_string(obj(vec![
            ("filter", Value::Null),
            ("pagination", obj(vec![("limit", val(&1u64))])),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/assets/rose%23centralbank/holders/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        assert_eq!(json["total"].as_u64(), Some(2));
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn asset_holders_query_sort_by_quantity_desc() {
        let _guard = app_query_limits_guard();
        let (state, _, _) = build_asset_holder_fixture_state();
        let telemetry = MaybeTelemetry::disabled();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders/query",
            post({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    handle_v1_asset_holders_query(
                        state,
                        axum::extract::Path(def_id),
                        crate::utils::extractors::NoritoJson(env),
                        telemetry,
                    )
                    .await
                }
            }),
        );
        let body = json_string(obj(vec![
            ("filter", Value::Null),
            (
                "sort",
                arr(vec![obj(vec![
                    ("key", val("quantity")),
                    ("order", val("desc")),
                ])]),
            ),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/assets/rose%23centralbank/holders/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        let first_qty = items[0]["quantity"].as_str().unwrap();
        let second_qty = items[1]["quantity"].as_str().unwrap();
        assert_eq!(first_qty, "20");
        assert_eq!(second_qty, "10");
    }
    #[tokio::test]
    async fn asset_holders_query_filters_by_account_id() {
        let _guard = app_query_limits_guard();
        let (state, alice_id, _bob_id) = build_asset_holder_fixture_state();
        let telemetry = MaybeTelemetry::disabled();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders/query",
            post({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    handle_v1_asset_holders_query(
                        state,
                        axum::extract::Path(def_id),
                        crate::utils::extractors::NoritoJson(env),
                        telemetry,
                    )
                    .await
                }
            }),
        );
        let body = json_string(obj(vec![(
            "filter",
            obj(vec![
                ("op", val("eq")),
                (
                    "args",
                    arr(vec![val("account_id"), val(&alice_id.to_string())]),
                ),
            ]),
        )]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/assets/rose%23centralbank/holders/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        let account_id = items[0]["account_id"].as_str().unwrap();
        assert_eq!(account_id, alice_id.to_string());
    }
    #[tokio::test]
    async fn account_assets_get_pagination_preserves_total() {
        let _guard = app_query_limits_guard();
        let alice_id = checked_app_api_account_id(
            0x80,
            "derive account assets GET pagination fixture account key",
        );
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let rose_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "rose".parse().unwrap());
        let lily_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "lily".parse().unwrap());
        let assets = vec![
            Asset::new(
                AssetId::new(rose_def.clone(), alice_id.clone()),
                Quantity::from(10_u32),
            ),
            Asset::new(
                AssetId::new(lily_def.clone(), alice_id.clone()),
                Quantity::from(7_u32),
            ),
        ];
        let state = state_with_assets(
            domain_id,
            alice_id.clone(),
            vec![alice_id.clone()],
            vec![(rose_def, "rose".to_owned()), (lily_def, "lily".to_owned())],
            assets,
        );
        use axum::routing::get;
        let app = Router::new().route(
            "/v1/accounts/{account_id}/assets",
            get({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                          crate::NoritoQuery(p): crate::NoritoQuery<AccountAssetsGetParams>| async move {
                        handle_v1_account_assets(
                            state,
                            axum::extract::Path(account_id),
                            crate::NoritoQuery(p),
                            crate::routing::MaybeTelemetry::disabled(),
                        )
                        .await
                    }
            }),
        );
        // Preserve the account literal textual representation (I105 by default)
        let req = http::Request::builder()
            .method("GET")
            .uri(format!("/v1/accounts/{}/assets?limit=1", alice_id))
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        assert_eq!(json["total"].as_u64(), Some(2));
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn account_assets_get_filters_by_asset_and_scope() {
        let _guard = app_query_limits_guard();
        let alice_id = checked_app_api_account_id(
            0x81,
            "derive account assets GET filter fixture account key",
        );
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let rose_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "rose".parse().unwrap());
        let lily_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "lily".parse().unwrap());
        let assets = vec![
            Asset::new(
                AssetId::new(rose_def.clone(), alice_id.clone()),
                Quantity::from(10_u32),
            ),
            Asset::new(
                AssetId::new(lily_def.clone(), alice_id.clone()),
                Quantity::from(7_u32),
            ),
        ];
        let state = state_with_assets(
            domain_id,
            alice_id.clone(),
            vec![alice_id.clone()],
            vec![
                (rose_def.clone(), "rose".to_owned()),
                (lily_def, "lily".to_owned()),
            ],
            assets,
        );
        use axum::routing::get;
        let app = Router::new().route(
            "/v1/accounts/{account_id}/assets",
            get({
                let state = state.clone();
                move |axum::extract::Path(account_id): axum::extract::Path<String>,
                      crate::NoritoQuery(p): crate::NoritoQuery<AccountAssetsGetParams>| async move {
                    handle_v1_account_assets(
                        state,
                        axum::extract::Path(account_id),
                        crate::NoritoQuery(p),
                        crate::routing::MaybeTelemetry::disabled(),
                    )
                    .await
                }
            }),
        );
        let asset = rose_def.to_string();
        let req = http::Request::builder()
            .method("GET")
            .uri(format!(
                "/v1/accounts/{}/assets?asset={}&scope=global",
                alice_id,
                urlencoding::encode(&asset)
            ))
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        assert_eq!(json["total"].as_u64(), Some(1));
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["asset"].as_str(), Some(asset.as_str()));
        assert_eq!(items[0]["scope"].as_str(), Some("global"));
        let account_literal = alice_id.to_string();
        assert_eq!(
            items[0]["account_id"].as_str(),
            Some(account_literal.as_str())
        );
        assert!(
            !items[0]["asset_name"].is_null(),
            "asset_name should be populated"
        );
        assert!(items[0]["asset_alias"].is_null());
    }
    #[tokio::test]
    async fn account_assets_routes_return_dataspace_scoped_asset_holder_without_account_record() {
        let _guard = app_query_limits_guard();
        let authority_id =
            checked_app_api_account_id(0x82, "derive dataspace-scoped asset authority key");
        let holder_id =
            checked_app_api_account_id(0x83, "derive dataspace-scoped asset holder key");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let kina_def = test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400dd");
        let dataspace_scope = iroha_data_model::asset::AssetBalanceScope::Dataspace(
            iroha_data_model::nexus::DataSpaceId::new(10),
        );
        let assets = vec![Asset::new(
            AssetId::with_scope(kina_def.clone(), holder_id.clone(), dataspace_scope),
            Quantity::from(81_u32),
        )];
        let state = state_with_assets(
            domain_id,
            authority_id,
            Vec::new(),
            vec![(kina_def.clone(), "kina".to_owned())],
            assets,
        );
        use axum::routing::get;
        let app = Router::new()
            .route(
                "/v1/accounts/{account_id}/assets",
                get({
                    let state = state.clone();
                    move |axum::extract::Path(account_id): axum::extract::Path<String>,
                          crate::NoritoQuery(p): crate::NoritoQuery<AccountAssetsGetParams>| async move {
                        handle_v1_account_assets(
                            state,
                            axum::extract::Path(account_id),
                            crate::NoritoQuery(p),
                            crate::routing::MaybeTelemetry::disabled(),
                        )
                        .await
                    }
                }),
            )
            .route(
                "/v1/accounts/{account_id}/assets/query",
                post({
                    let state = state.clone();
                    move |axum::extract::Path(account_id): axum::extract::Path<String>,
                          crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<QueryEnvelope>| async move {
                        handle_v1_account_assets_query(
                            state,
                            axum::extract::Path(account_id),
                            crate::utils::extractors::NoritoJson(env),
                            crate::routing::MaybeTelemetry::disabled(),
                        )
                        .await
                    }
                }),
            );
        let holder_literal = holder_id.to_string();
        let asset_literal = kina_def.to_string();
        let get_req = http::Request::builder()
            .method("GET")
            .uri(format!(
                "/v1/accounts/{holder_literal}/assets?scope=dataspace:10"
            ))
            .body(axum::body::Body::empty())
            .unwrap();
        let get_resp = app.clone().oneshot(get_req).await.unwrap();
        assert_eq!(get_resp.status(), http::StatusCode::OK);
        let get_bytes = get_resp.into_body().collect().await.unwrap().to_bytes();
        let get_json: norito::json::Value = norito::json::from_slice(&get_bytes).unwrap();
        assert_eq!(get_json["total"].as_u64(), Some(1));
        let get_items = get_json["items"].as_array().unwrap();
        assert_eq!(
            get_items[0]["account_id"].as_str(),
            Some(holder_literal.as_str())
        );
        assert_eq!(get_items[0]["asset"].as_str(), Some(asset_literal.as_str()));
        assert_eq!(get_items[0]["scope"].as_str(), Some("dataspace:10"));
        assert_eq!(get_items[0]["quantity"].as_str(), Some("81"));
        let query_body = json_string(obj(vec![
            ("filter", Value::Null),
            ("pagination", obj(vec![("limit", val(&10u64))])),
            ("count_mode", val("exact")),
        ]));
        let query_req = http::Request::builder()
            .method("POST")
            .uri(format!("/v1/accounts/{holder_literal}/assets/query"))
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(query_body))
            .unwrap();
        let query_resp = app.oneshot(query_req).await.unwrap();
        assert_eq!(query_resp.status(), http::StatusCode::OK);
        let query_bytes = query_resp.into_body().collect().await.unwrap().to_bytes();
        let query_json: norito::json::Value = norito::json::from_slice(&query_bytes).unwrap();
        assert_eq!(query_json["total"].as_u64(), Some(1));
        assert_eq!(
            query_json["items"][0]["account_id"].as_str(),
            Some(holder_literal.as_str())
        );
    }
    #[tokio::test]
    async fn account_assets_get_rejects_limit_above_cap() {
        let _guard = app_query_limits_guard();
        let alice_id = checked_app_api_account_id(
            0x84,
            "derive account assets GET limit validation fixture account key",
        );
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let rose_def =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "rose".parse().unwrap());
        let assets = vec![Asset::new(
            AssetId::new(rose_def.clone(), alice_id.clone()),
            Quantity::from(1_u32),
        )];
        let state = state_with_assets(
            domain_id,
            alice_id.clone(),
            vec![alice_id.clone()],
            vec![(rose_def, "rose".to_owned())],
            assets,
        );
        let cap = app_query_limits().max_page_limit;
        let params = AccountAssetsGetParams {
            limit: Some(cap + 1),
            offset: 0,
            asset: None,
            scope: None,
            count_mode: None,
        };
        let err = handle_v1_account_assets(
            state,
            axum::extract::Path(alice_id.to_string()),
            crate::NoritoQuery(params),
            MaybeTelemetry::for_tests(),
        )
        .await;
        match err {
            Err(Error::AppQueryValidation { code, .. }) => assert_eq!(code, "invalid_pagination"),
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("expected error for limit above cap"),
        }
    }
    #[tokio::test]
    async fn asset_holders_get_pagination_preserves_total() {
        let _guard = app_query_limits_guard();
        let (state, _, _) = build_asset_holder_fixture_state();
        use axum::routing::get;
        let telemetry = MaybeTelemetry::for_tests();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders",
            get({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::NoritoQuery(p): crate::NoritoQuery<AssetHolderGetParams>| {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    async move {
                        handle_v1_asset_holders(
                            state,
                            axum::extract::Path(def_id),
                            crate::NoritoQuery(p),
                            telemetry,
                        )
                        .await
                    }
                }
            }),
        );
        let req = http::Request::builder()
            .method("GET")
            .uri("/v1/assets/rose%23centralbank/holders?limit=1&count_mode=exact")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        assert_eq!(json["total"].as_u64(), Some(2));
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn asset_holders_get_filters_by_account_id() {
        let _guard = app_query_limits_guard();
        let (state, alice_id, _) = build_asset_holder_fixture_state();
        use axum::routing::get;
        let telemetry = MaybeTelemetry::for_tests();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders",
            get({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::NoritoQuery(p): crate::NoritoQuery<AssetHolderGetParams>| {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    async move {
                        handle_v1_asset_holders(
                            state,
                            axum::extract::Path(def_id),
                            crate::NoritoQuery(p),
                            telemetry,
                        )
                        .await
                    }
                }
            }),
        );
        let expected_account = alice_id.to_string();
        let req = http::Request::builder()
            .method("GET")
            .uri(format!(
                "/v1/assets/rose%23centralbank/holders?account_id={}&count_mode=exact",
                urlencoding::encode(&expected_account)
            ))
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            status,
            http::StatusCode::OK,
            "unexpected response body: {}",
            String::from_utf8_lossy(&bytes)
        );
        let json: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        assert_eq!(json["total"].as_u64(), Some(1));
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0]["account_id"].as_str(),
            Some(expected_account.as_str())
        );
    }
    #[tokio::test]
    async fn asset_holders_get_filters_by_account_alias() {
        let _guard = app_query_limits_guard();
        let (state, alice_id, _) = build_asset_holder_fixture_state();
        use axum::routing::get;
        let telemetry = MaybeTelemetry::for_tests();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders",
            get({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::NoritoQuery(p): crate::NoritoQuery<AssetHolderGetParams>| {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    async move {
                        handle_v1_asset_holders(
                            state,
                            axum::extract::Path(def_id),
                            crate::NoritoQuery(p),
                            telemetry,
                        )
                        .await
                    }
                }
            }),
        );
        let req = http::Request::builder()
            .method("GET")
            .uri("/v1/assets/rose%23centralbank/holders?account_id=treasury%40universal&count_mode=exact")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            status,
            http::StatusCode::OK,
            "unexpected response body: {}",
            String::from_utf8_lossy(&bytes)
        );
        let json: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        assert_eq!(json["total"].as_u64(), Some(1));
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        let expected_account = alice_id.to_string();
        assert_eq!(
            items[0]["account_id"].as_str(),
            Some(expected_account.as_str())
        );
    }
    #[tokio::test]
    async fn asset_holders_get_rejects_limit_above_cap() {
        let _guard = app_query_limits_guard();
        let (state, _, _) = build_asset_holder_fixture_state();
        let cap = app_query_limits().max_page_limit;
        let params = AssetHolderGetParams {
            limit: Some(cap + 1),
            offset: 0,
            account_id: None,
            scope: None,
            count_mode: None,
        };
        let err = handle_v1_asset_holders(
            state,
            axum::extract::Path("rose#centralbank".to_string()),
            crate::NoritoQuery(params),
            MaybeTelemetry::for_tests(),
        )
        .await;
        match err {
            Err(Error::AppQueryValidation { code, .. }) => assert_eq!(code, "invalid_pagination"),
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("expected error for limit above cap"),
        }
    }
    #[tokio::test]
    async fn asset_holders_get_rejects_legacy_aid_path_selector() {
        let _guard = app_query_limits_guard();
        let (state, _, _) = build_asset_holder_fixture_state();
        let params = AssetHolderGetParams {
            limit: Some(10),
            offset: 0,
            account_id: None,
            scope: None,
            count_mode: None,
        };
        let result = handle_v1_asset_holders(
            state,
            axum::extract::Path("prefix:550e8400e29b41d4a7164466554400dd".to_string()),
            crate::NoritoQuery(params),
            MaybeTelemetry::for_tests(),
        )
        .await;
        match result {
            Err(Error::Query(iroha_data_model::ValidationFail::NotPermitted(_))) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("prefixed selector must be rejected"),
        }
    }
    #[tokio::test]
    async fn asset_holders_get_uses_canonical_i105_literals() {
        let _guard = app_query_limits_guard();
        use axum::routing::get;
        let (state, alice_id, bob_id) = build_asset_holder_fixture_state();
        let telemetry = MaybeTelemetry::for_tests();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders",
            get({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::NoritoQuery(p): crate::NoritoQuery<AssetHolderGetParams>| {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    async move {
                        handle_v1_asset_holders(
                            state,
                            axum::extract::Path(def_id),
                            crate::NoritoQuery(p),
                            telemetry,
                        )
                        .await
                    }
                }
            }),
        );
        let req = http::Request::builder()
            .method("GET")
            .uri("/v1/assets/rose%23centralbank/holders?limit=4")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let parsed: norito::json::Value = norito::json::from_slice(&body).expect("json response");
        let items = parsed["items"]
            .as_array()
            .cloned()
            .expect("items array should exist");
        assert_eq!(parsed["total"].as_u64(), Some(2));
        assert_eq!(items.len(), 2);
        let expected = [i105_literal(&alice_id), i105_literal(&bob_id)];
        for literal in expected {
            assert!(
                items.iter().any(|item| item
                    .as_object()
                    .and_then(|obj| obj.get("account_id"))
                    .and_then(norito::json::Value::as_str)
                    .map(|value| value == literal)
                    .unwrap_or(false)),
                "i105 literal {literal} missing from holders response"
            );
        }
    }
    #[tokio::test]
    async fn confidential_asset_transitions_reports_pending_window_metadata() {
        let _guard = app_query_limits_guard();
        use axum::routing::get;
        let alice_id = checked_app_api_account_id(
            0x85,
            "derive confidential asset transition fixture account key",
        );
        let vk_hash = Hash::new(b"vk-set-hash");
        let transition_id = Hash::new(b"transition-window");
        let expected_vk_hex = encode_hash_hex(&vk_hash);
        let expected_transition_hex = encode_hash_hex(&transition_id);
        let pending_transition =
            iroha_data_model::asset::definition::ConfidentialPolicyTransition {
                new_mode: iroha_data_model::asset::definition::ConfidentialPolicyMode::ShieldedOnly,
                previous_mode:
                    iroha_data_model::asset::definition::ConfidentialPolicyMode::Convertible,
                effective_height: 1_200,
                transition_id,
                conversion_window: Some(200),
            };
        let policy = iroha_data_model::asset::definition::AssetConfidentialPolicy {
            mode: iroha_data_model::asset::definition::ConfidentialPolicyMode::Convertible,
            vk_set_hash: Some(vk_hash),
            poseidon_params_id: Some(7),
            pedersen_params_id: Some(11),
            pending_transition: Some(pending_transition),
        };
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&alice_id);
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let mut asset_def = AssetDefinition::numeric(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            "rose".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&alice_id);
        asset_def.set_confidential_policy(policy);
        let asset_def_id = asset_def.id().clone();
        let expected_asset_id = asset_def_id.to_string();
        let world = World::with([domain], [account], [asset_def]);
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        bind_permanent_asset_alias_for_test(&state, &alice_id, &asset_def_id, "rose#centralbank");
        let app = Router::new().route(
            "/v1/confidential/assets/{definition_id}/transitions",
            get({
                let state = state.clone();
                move |path: axum::extract::Path<String>| {
                    let state = state.clone();
                    async move { handle_v1_confidential_asset_transitions(state, path).await }
                }
            }),
        );
        let req = http::Request::builder()
            .method("GET")
            .uri("/v1/confidential/assets/rose%23centralbank/transitions")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        let json: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        assert_eq!(json["asset_id"].as_str(), Some(expected_asset_id.as_str()));
        assert_eq!(json["current_mode"].as_str(), Some("Convertible"));
        assert_eq!(json["effective_mode"].as_str(), Some("Convertible"));
        assert_eq!(json["vk_set_hash"].as_str(), Some(expected_vk_hex.as_str()));
        assert_eq!(json["poseidon_params_id"].as_u64(), Some(7));
        assert_eq!(json["pedersen_params_id"].as_u64(), Some(11));
        assert_eq!(
            json["pending_transition"]["transition_id"].as_str(),
            Some(expected_transition_hex.as_str())
        );
        assert_eq!(
            json["pending_transition"]["conversion_window"].as_u64(),
            Some(200)
        );
        assert_eq!(
            json["pending_transition"]["window_open_height"].as_u64(),
            Some(1_000)
        );
        assert_eq!(
            json["pending_transition"]["new_mode"].as_str(),
            Some("ShieldedOnly")
        );
        assert!(json["block_height"].as_u64().is_some());
    }
    #[tokio::test]
    async fn confidential_asset_transitions_rejects_prefixed_path_selector() {
        let _guard = app_query_limits_guard();
        let (state, _, _) = build_asset_holder_fixture_state();
        let result = handle_v1_confidential_asset_transitions(
            state,
            axum::extract::Path("prefix:550e8400e29b41d4a7164466554400dd".to_string()),
        )
        .await;
        match result {
            Err(Error::Query(iroha_data_model::ValidationFail::NotPermitted(_))) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("prefixed selector must be rejected"),
        }
    }
    #[tokio::test]
    async fn get_parameters_returns_json() {
        let _guard = app_query_limits_guard();
        use axum::routing::get;
        use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::State};
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let app = Router::new().route(
            "/v1/parameters",
            get({
                let state = state.clone();
                move || async move { handle_v1_parameters(state.clone()).await }
            }),
        );
        let req = http::Request::builder()
            .method("GET")
            .uri("/v1/parameters")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        let json: norito::json::Value = norito::json::from_str(&s).unwrap();
        assert!(json.get("sumeragi").is_some());
        assert!(json.get("block").is_some());
        assert!(json.get("transaction").is_some());
    }
    #[tokio::test]
    async fn asset_holders_query_uses_canonical_i105_literals() {
        let _guard = app_query_limits_guard();
        use axum::routing::post;
        let (state, alice_id, bob_id) = build_asset_holder_fixture_state();
        let telemetry = MaybeTelemetry::for_tests();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders/query",
            post({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
                        QueryEnvelope,
                    >| {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    async move {
                        handle_v1_asset_holders_query(
                            state,
                            axum::extract::Path(def_id),
                            crate::utils::extractors::NoritoJson(env),
                            telemetry,
                        )
                        .await
                    }
                }
            }),
        );
        let body = json_string(obj(vec![(
            "pagination",
            obj(vec![("limit", val(&8u64)), ("offset", val(&0u64))]),
        )]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/assets/rose%23centralbank/holders/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let payload = resp.into_body().collect().await.unwrap().to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_slice(&payload).expect("json response");
        let items = parsed["items"]
            .as_array()
            .cloned()
            .expect("items array should exist");
        assert_eq!(parsed["total"].as_u64(), Some(2));
        assert_eq!(items.len(), 2);
        let expected = [i105_literal(&alice_id), i105_literal(&bob_id)];
        for literal in expected {
            assert!(
                items.iter().any(|item| item
                    .as_object()
                    .and_then(|obj| obj.get("account_id"))
                    .and_then(norito::json::Value::as_str)
                    .map(|value| value == literal)
                    .unwrap_or(false)),
                "i105 literal {literal} missing from holders query response"
            );
        }
    }
    #[tokio::test]
    async fn asset_holders_query_filter_accepts_account_alias() {
        let _guard = app_query_limits_guard();
        use axum::routing::post;
        let (state, alice_id, _) = build_asset_holder_fixture_state();
        let telemetry = MaybeTelemetry::for_tests();
        let app = Router::new().route(
            "/v1/assets/{definition_id}/holders/query",
            post({
                let state = state.clone();
                let telemetry = telemetry.clone();
                move |axum::extract::Path(def_id): axum::extract::Path<String>,
                      crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
                        QueryEnvelope,
                    >| {
                    let telemetry = telemetry.clone();
                    let state = state.clone();
                    async move {
                        handle_v1_asset_holders_query(
                            state,
                            axum::extract::Path(def_id),
                            crate::utils::extractors::NoritoJson(env),
                            telemetry,
                        )
                        .await
                    }
                }
            }),
        );
        let body = json_string(obj(vec![
            (
                "filter",
                obj(vec![
                    ("op", val("eq")),
                    (
                        "args",
                        arr(vec![val("account_id"), val("treasury@universal")]),
                    ),
                ]),
            ),
            (
                "pagination",
                obj(vec![("limit", val(&8u64)), ("offset", val(&0u64))]),
            ),
        ]));
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/assets/rose%23centralbank/holders/query")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let payload = resp.into_body().collect().await.unwrap().to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_slice(&payload).expect("json response");
        let items = parsed["items"]
            .as_array()
            .expect("items array should exist");
        assert_eq!(parsed["total"].as_u64(), Some(1));
        assert_eq!(items.len(), 1);
        let expected_account = alice_id.to_string();
        assert_eq!(
            items[0]["account_id"].as_str(),
            Some(expected_account.as_str())
        );
    }
    #[tokio::test]
    async fn asset_holders_query_aggregate_groups_pkrs_by_primary_alias_domain() {
        let _guard = app_query_limits_guard();
        let (state, _, _) = build_asset_holder_aggregate_fixture_state();
        let parsed = run_asset_holder_alias_aggregate_query(None, state).await;
        assert_eq!(parsed["total"].as_u64(), Some(2));
        assert_eq!(parsed["query_source"].as_str(), Some("live_debug"));
        assert!(parsed["indexed_height"].as_u64().is_some());
        assert!(parsed["indexed_block_hash"].is_string() || parsed["indexed_block_hash"].is_null());
        let items = parsed["items"].as_array().expect("items array");
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0]["primary_alias_domain"].as_str(),
            Some("hbl.paynet")
        );
        assert_eq!(items[0]["user_count"].as_u64(), Some(2));
        assert_eq!(items[0]["pkr_total"].as_str(), Some("15"));
        assert_eq!(
            items[1]["primary_alias_domain"].as_str(),
            Some("ubl.paynet")
        );
        assert_eq!(items[1]["user_count"].as_u64(), Some(1));
        assert_eq!(items[1]["pkr_total"].as_str(), Some("5"));
    }
    #[tokio::test]
    async fn asset_holders_query_aggregate_uses_cached_projection_shards_when_published() {
        let _guard = app_query_limits_guard();
        clear_query_projection_archive_cache_for_tests();
        let (state, _, _) = build_asset_holder_aggregate_fixture_state();
        let published = publish_asset_holder_checkpoint_with_real_manifests(&state).await;
        for (archive, _) in &published {
            cache_query_projection_archive_for_query(archive.clone());
        }
        let parsed = run_asset_holder_alias_aggregate_query(None, state).await;
        assert_eq!(parsed["query_source"].as_str(), Some("projection_da_cache"));
        let items = parsed["items"].as_array().expect("items array");
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0]["primary_alias_domain"].as_str(),
            Some("hbl.paynet")
        );
        assert_eq!(items[0]["user_count"].as_u64(), Some(2));
        assert_eq!(items[0]["pkr_total"].as_str(), Some("15"));
        assert_eq!(
            items[1]["primary_alias_domain"].as_str(),
            Some("ubl.paynet")
        );
        assert_eq!(items[1]["user_count"].as_u64(), Some(1));
        assert_eq!(items[1]["pkr_total"].as_str(), Some("5"));
        clear_query_projection_archive_cache_for_tests();
    }
    #[tokio::test]
    async fn asset_holders_query_aggregate_requires_capability_for_remote_projection_hydration() {
        use axum::{
            Router,
            extract::Path as AxumPath,
            routing::get,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};
        let _guard = app_query_limits_guard();
        clear_query_projection_archive_cache_for_tests();
        let (state, _, _) = build_asset_holder_aggregate_fixture_state();
        let published = publish_asset_holder_checkpoint_with_real_manifests(&state).await;
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("tcp bind failed: {err}"),
        };
        let remote_origin = format!("http://{}", listener.local_addr().expect("listener addr"));
        let fixture = make_projection_provider_fixture(&remote_origin);
        let manifest_requests = Arc::new(AtomicUsize::new(0));
        let mut manifest_responses = std::collections::HashMap::new();
        for (index, (archive, manifest)) in published.iter().enumerate() {
            let (payload, plan, manifest_for_storage) =
                query_projection_archive_storage_artifacts(archive)
                    .expect("projection archive storage artifacts");
            let manifest_digest_hex = hex::encode(
                manifest_for_storage
                    .digest()
                    .expect("digest projection archive manifest")
                    .as_bytes(),
            );
            let manifest_response =
                norito::json::to_value(&crate::sorafs::api::StorageManifestResponseDto {
                    manifest_id_hex: manifest_digest_hex.clone(),
                    manifest_b64: base64::engine::general_purpose::STANDARD
                        .encode(norito::to_bytes(manifest).expect("encode manifest")),
                    manifest_digest_hex: manifest_digest_hex.clone(),
                    payload_digest_hex: hex::encode(blake3::hash(&payload).as_bytes()),
                    content_length: plan.content_length,
                    chunk_count: plan.chunks.len() as u64,
                    chunk_profile_handle: format!(
                        "{}.{}@{}",
                        manifest.chunking.namespace,
                        manifest.chunking.name,
                        manifest.chunking.semver
                    ),
                    stored_at_unix_secs: 1_700_000_000,
                    files: Vec::new(),
                })
                .expect("serialize manifest response");
            manifest_responses.insert(manifest_digest_hex.clone(), manifest_response);
            seed_projection_registry_manifest_for_test(
                state.as_ref(),
                manifest,
                fixture.provider_id(),
                index.wrapping_add(1) as u8,
            );
        }
        let remote_router = Router::new().route(
                "/v1/sorafs/storage/manifest/{manifest_id_hex}",
                get({
                    let manifest_requests = Arc::clone(&manifest_requests);
                    let manifest_responses = manifest_responses.clone();
                    move |AxumPath(manifest_id_hex): AxumPath<String>| {
                        let manifest_requests = Arc::clone(&manifest_requests);
                        let manifest_responses = manifest_responses.clone();
                        async move {
                            manifest_requests.fetch_add(1, Ordering::SeqCst);
                            let Some(response) = manifest_responses.get(&manifest_id_hex) else {
                                return axum::http::StatusCode::NOT_FOUND.into_response();
                            };
                            crate::JsonBody(response.clone()).into_response()
                        }
                    }
                }),
            );
        let remote_server = tokio::spawn(async move {
            axum::serve(listener, remote_router)
                .await
                .expect("serve remote storage routes");
        });
        let (app, _storage_dir) = app_state_with_projection_provider_fixture(&fixture);
        let invoke = || {
            handle_v1_asset_holders_query_with_app(
                Some(app.clone()),
                state.clone(),
                axum::extract::Path("pkr#paynet".to_owned()),
                crate::utils::extractors::NoritoJson(asset_holder_alias_aggregate_query()),
                MaybeTelemetry::for_tests(),
            )
        };
        let error = match invoke().await {
            Err(error) => error,
            Ok(_) => panic!("unsigned remote projection hydration must remain disabled"),
        };
        match error {
            Error::AppServiceUnavailable { code, message } => {
                assert_eq!(code, "projection_archive_unavailable");
                assert!(message.contains(crate::sorafs::api::REMOTE_HYDRATION_CAPABILITY_REQUIRED));
            }
            other => panic!("unexpected remote hydration error: {other:?}"),
        }
        assert_eq!(
            manifest_requests.load(Ordering::SeqCst),
            1,
            "the first missing shard manifest may be verified before capability rejection",
        );
        let second_error = match invoke().await {
            Err(error) => error,
            Ok(_) => panic!("capability rejection must not create a projection cache entry"),
        };
        assert!(matches!(
            second_error,
            Error::AppServiceUnavailable {
                code: "projection_archive_unavailable",
                ..
            }
        ));
        assert_eq!(
            manifest_requests.load(Ordering::SeqCst),
            2,
            "a rejected remote payload must not populate the local cache",
        );
        remote_server.abort();
        clear_query_projection_archive_cache_for_tests();
    }
    fn build_asset_holder_fixture_state() -> (Arc<iroha_core::state::State>, AccountId, AccountId) {
        let alice_id = checked_app_api_account_id(0x86, "derive asset holder fixture Alice key");
        let bob_id = checked_app_api_account_id(0x87, "derive asset holder fixture Bob key");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let rose_def: AssetDefinitionId =
            test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400dd");
        let rose_definition = AssetDefinition::numeric(
            rose_def.clone(),
            "rose".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&alice_id);
        let assets = vec![
            Asset::new(
                AssetId::new(rose_def.clone(), alice_id.clone()),
                Quantity::from(10_u32),
            ),
            Asset::new(
                AssetId::new(rose_def.clone(), bob_id.clone()),
                Quantity::from(20_u32),
            ),
        ];
        let domain = Domain::new(domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let bob_account = Account::new(bob_id.clone()).build(&alice_id);
        let world = World::with_assets(
            [domain],
            [alice_account, bob_account],
            [rose_definition],
            assets,
            [],
        );
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        bind_permanent_asset_alias_for_test(&state, &alice_id, &rose_def, "rose#centralbank");
        bind_account_alias_for_test(&state, &alice_id, "treasury@universal");
        (state, alice_id, bob_id)
    }
    fn build_asset_holder_aggregate_fixture_state()
    -> (Arc<iroha_core::state::State>, AccountId, AccountId) {
        let alice_id = checked_app_api_account_id(0x88, "derive aggregate asset holder Alice key");
        let bob_id = checked_app_api_account_id(0x89, "derive aggregate asset holder Bob key");
        let hbl_settlement_id =
            checked_app_api_account_id(0x8A, "derive HBL settlement asset holder key");
        let ubl_settlement_id =
            checked_app_api_account_id(0x8B, "derive UBL settlement asset holder key");
        let ubl_user_id = checked_app_api_account_id(0x8C, "derive UBL user asset holder key");
        let domain_id: DomainId = DomainId::try_new("aggregate-holders", "universal").unwrap();
        let pkr_def: AssetDefinitionId =
            test_asset_definition_id_from_hex("550e8400e29b41d4a7164466554400de");
        let pkr_definition = AssetDefinition::numeric(
            pkr_def.clone(),
            "pkr".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&alice_id);
        let paynet_dataspace_id = iroha_data_model::nexus::DataSpaceId::new(92);
        let assets = vec![
            Asset::new(
                AssetId::new(pkr_def.clone(), alice_id.clone()),
                Quantity::from(10_u32),
            ),
            Asset::new(
                AssetId::with_scope(
                    pkr_def.clone(),
                    alice_id.clone(),
                    iroha_data_model::asset::AssetBalanceScope::Dataspace(paynet_dataspace_id),
                ),
                Quantity::from(3_u32),
            ),
            Asset::new(
                AssetId::new(pkr_def.clone(), bob_id.clone()),
                Quantity::from(2_u32),
            ),
            Asset::new(
                AssetId::new(pkr_def.clone(), ubl_user_id.clone()),
                Quantity::from(5_u32),
            ),
            Asset::new(
                AssetId::new(pkr_def.clone(), hbl_settlement_id.clone()),
                Quantity::from(125_u32),
            ),
            Asset::new(
                AssetId::new(pkr_def.clone(), ubl_settlement_id.clone()),
                Quantity::from(75_u32),
            ),
        ];
        let hbl_domain_id = DomainId::try_new("hbl", "paynet").expect("HBL domain");
        let ubl_domain_id = DomainId::try_new("ubl", "paynet").expect("UBL domain");
        let domain = Domain::new(domain_id).build(&alice_id);
        let hbl_domain = Domain::new(hbl_domain_id.clone()).build(&alice_id);
        let ubl_domain = Domain::new(ubl_domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let bob_account = Account::new(bob_id.clone()).build(&alice_id);
        let hbl_settlement_account = Account::new(hbl_settlement_id.clone()).build(&alice_id);
        let ubl_settlement_account = Account::new(ubl_settlement_id.clone()).build(&alice_id);
        let ubl_user_account = Account::new(ubl_user_id.clone()).build(&alice_id);
        let mut world = World::with_assets(
            [domain, hbl_domain, ubl_domain],
            [
                alice_account,
                bob_account,
                ubl_user_account,
                hbl_settlement_account,
                ubl_settlement_account,
            ],
            [pkr_definition],
            assets,
            [],
        );
        install_asset_holder_alias_parent_leases_for_test(
            &mut world,
            &alice_id,
            paynet_dataspace_id,
            &[&hbl_domain_id, &ubl_domain_id],
        );
        let mut state = Arc::new(iroha_core::state::State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let dataspace_catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: paynet_dataspace_id,
                alias: "paynet".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("paynet dataspace catalog");
        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            std::num::NonZeroU32::new(2).expect("nonzero lane count"),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                iroha_data_model::nexus::LaneConfig {
                    id: iroha_data_model::nexus::LaneId::new(1),
                    dataspace_id: paynet_dataspace_id,
                    alias: "paynet".to_owned(),
                    visibility: iroha_data_model::nexus::LaneVisibility::Public,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
            ],
        )
        .expect("paynet lane catalog");
        Arc::get_mut(&mut state)
            .expect("unique state")
            .set_nexus(iroha_config::parameters::actual::Nexus {
                lane_catalog,
                dataspace_catalog,
                ..iroha_config::parameters::actual::Nexus::default()
            })
            .expect("install paynet nexus config");
        bind_permanent_asset_alias_for_test(&state, &alice_id, &pkr_def, "pkr#paynet");
        bind_account_alias_for_test(&state, &alice_id, "alice@hbl.paynet");
        bind_account_alias_for_test(&state, &bob_id, "bilal@hbl.paynet");
        bind_account_alias_for_test(&state, &ubl_user_id, "amir@ubl.paynet");
        bind_account_alias_for_test(&state, &hbl_settlement_id, "cbdc@hbl.paynet");
        bind_account_alias_for_test(&state, &ubl_settlement_id, "cbdc@ubl.paynet");
        (state, alice_id, bob_id)
    }
    fn install_asset_holder_alias_parent_leases_for_test(
        world: &mut World,
        owner: &AccountId,
        dataspace_id: iroha_data_model::nexus::DataSpaceId,
        domains: &[&DomainId],
    ) {
        let controller = iroha_data_model::sns::NameControllerV1::account(
            &iroha_data_model::account::AccountAddress::from_account_id(owner)
                .expect("parent lease owner address"),
        );
        let dataspace_selector =
            iroha_core::sns::selector_for_dataspace_alias("paynet").expect("paynet selector");
        let mut dataspace_metadata = iroha_data_model::metadata::Metadata::default();
        dataspace_metadata.insert(
            iroha_core::sns::SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace metadata key"),
            iroha_primitives::json::Json::new(dataspace_id.as_u64()),
        );
        let dataspace_record = iroha_data_model::sns::NameRecordV1::new(
            dataspace_selector.clone(),
            owner.clone(),
            vec![controller.clone()],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            dataspace_metadata,
        );
        world.smart_contract_state_mut_for_testing().insert(
            iroha_core::sns::record_storage_key(&dataspace_selector),
            norito::codec::Encode::encode(&dataspace_record),
        );
        for domain in domains {
            let selector =
                iroha_core::sns::selector_for_domain(domain).expect("parent domain selector");
            let record = iroha_data_model::sns::NameRecordV1::new(
                selector.clone(),
                owner.clone(),
                vec![controller.clone()],
                0,
                0,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                iroha_data_model::metadata::Metadata::default(),
            );
            world.smart_contract_state_mut_for_testing().insert(
                iroha_core::sns::record_storage_key(&selector),
                norito::codec::Encode::encode(&record),
            );
        }
    }
    fn asset_holder_alias_aggregate_query() -> QueryEnvelope {
        QueryEnvelope {
            query: None,
            filter: Some(crate::filter::FilterExpr::And(vec![
                crate::filter::FilterExpr::In(
                    crate::filter::FieldPath("primary_alias_domain".into()),
                    vec![
                        norito::json::Value::from("hbl.paynet"),
                        norito::json::Value::from("ubl.paynet"),
                    ],
                ),
                crate::filter::FilterExpr::Nin(
                    crate::filter::FieldPath("primary_alias".into()),
                    vec![
                        norito::json::Value::from("cbdc@hbl.paynet"),
                        norito::json::Value::from("cbdc@ubl.paynet"),
                    ],
                ),
            ])),
            select: None,
            aggregate: Some(crate::filter::AggregateSpec {
                group_by: vec![crate::filter::FieldPath("primary_alias_domain".into())],
                metrics: vec![
                    crate::filter::AggregateMetric {
                        alias: "user_count".into(),
                        r#fn: crate::filter::AggregateFn::DistinctCount,
                        field: Some(crate::filter::FieldPath("account_id".into())),
                    },
                    crate::filter::AggregateMetric {
                        alias: "pkr_total".into(),
                        r#fn: crate::filter::AggregateFn::Sum,
                        field: Some(crate::filter::FieldPath("quantity".into())),
                    },
                ],
                having: None,
            }),
            sort: vec![crate::filter::SortKey {
                key: crate::filter::FieldPath("primary_alias_domain".into()),
                order: crate::filter::Order::Asc,
            }],
            pagination: crate::filter::Pagination {
                limit: Some(8),
                offset: 0,
            },
            fetch_size: None,
            count_mode: None,
        }
    }
    async fn run_asset_holder_alias_aggregate_query(
        app: Option<crate::SharedAppState>,
        state: Arc<iroha_core::state::State>,
    ) -> norito::json::Value {
        let response = handle_v1_asset_holders_query_with_app(
            app,
            state,
            axum::extract::Path("pkr#paynet".to_owned()),
            crate::utils::extractors::NoritoJson(asset_holder_alias_aggregate_query()),
            MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok")
        .into_response();
        assert_eq!(response.status(), http::StatusCode::OK);
        let payload = response
            .into_body()
            .collect()
            .await
            .expect("body bytes")
            .to_bytes();
        norito::json::from_slice(&payload).expect("json response")
    }
    fn default_projection_registry_block_header() -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("non-zero block height"),
            None,
            None,
            None,
            0,
            0,
        )
    }
    fn default_projection_registry_chunker_handle()
    -> iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle {
        let descriptor = sorafs_manifest::chunker_registry::default_descriptor();
        iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle {
            profile_id: descriptor.id.0,
            namespace: descriptor.namespace.to_owned(),
            name: descriptor.name.to_owned(),
            semver: descriptor.semver.to_owned(),
            multihash_code: descriptor.multihash_code,
        }
    }
    fn projection_query_sorafs_node_with_temp_storage()
    -> (sorafs_node::NodeHandle, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let cfg = sorafs_node::config::StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("storage"))
            .build();
        (sorafs_node::NodeHandle::new(cfg), temp_dir)
    }
    fn seed_projection_registry_manifest_for_test(
        state: &iroha_core::state::State,
        manifest: &sorafs_manifest::ManifestV1,
        provider_id: [u8; 32],
        order_seed: u8,
    ) {
        let mut block = state.block(default_projection_registry_block_header());
        let mut tx = block.transaction();
        let manifest_digest = iroha_data_model::sorafs::pin_registry::ManifestDigest::new(
            manifest
                .digest()
                .expect("compute manifest digest for registry seed")
                .into(),
        );
        let manifest_root_cid =
            iroha_data_model::sorafs::pin_registry::ManifestRootCid::try_from_slice(
                &manifest.root_cid,
            )
            .expect("projection manifest must use a canonical root CID");
        let issuer = checked_app_api_account_id(0x8D, "derive projection registry issuer key");
        let policy = iroha_data_model::sorafs::pin_registry::PinPolicy::default();
        let content_length = manifest.content_length;
        let amount = state
            .view()
            .world()
            .sorafs_pricing()
            .public_pin_fee(
                policy.storage_class,
                content_length,
                policy.min_replicas,
                5,
                policy.retention_epoch,
            )
            .expect("projection registry fixture pin fee");
        let mut manifest_record = iroha_data_model::sorafs::pin_registry::PinManifestRecord::new(
            manifest_digest.clone(),
            manifest_root_cid.clone(),
            default_projection_registry_chunker_handle(),
            manifest.chunk_digest_sha3_256,
            manifest.por_root,
            content_length,
            policy,
            issuer.clone(),
            5,
            None,
            None,
            iroha_data_model::metadata::Metadata::default(),
        );
        manifest_record.record_pin_fee_payment(
            iroha_data_model::sorafs::pin_registry::PinFeePayment {
                paid_by: issuer.clone(),
                fee_asset_id: state.gov.sorafs_pin_fee_asset_id.clone(),
                treasury_account_id: state.gov.sorafs_pin_fee_treasury_account.clone(),
                amount,
            },
        );
        manifest_record.approve(7, None);
        tx.world_mut_for_testing()
            .pin_manifests_mut_for_testing()
            .insert(manifest_digest.clone(), manifest_record);
        let order_id =
            iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new([order_seed; 32]);
        let canonical_order = norito::to_bytes(&sorafs_manifest::capacity::ReplicationOrderV1 {
            version: sorafs_manifest::capacity::REPLICATION_ORDER_VERSION_V1,
            order_id: *order_id.as_bytes(),
            manifest_cid: manifest.root_cid.clone(),
            manifest_digest: *manifest_digest.as_bytes(),
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            target_replicas: 1,
            assignments: vec![sorafs_manifest::capacity::ReplicationAssignmentV1 {
                provider_id,
                slice_gib: 1,
                lane: None,
            }],
            issued_at: 8,
            deadline_at: 24,
            sla: sorafs_manifest::capacity::ReplicationOrderSlaV1 {
                ingest_deadline_secs: 16,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: Vec::new(),
        })
        .expect("encode replication order");
        tx.world_mut_for_testing()
            .replication_orders_mut_for_testing()
            .insert(
                order_id,
                iroha_data_model::sorafs::pin_registry::ReplicationOrderRecord {
                    order_id,
                    manifest_digest,
                    manifest_root_cid,
                    musubi_archive: None,
                    issued_by: issuer.clone(),
                    issued_epoch: 8,
                    deadline_epoch: 24,
                    canonical_order,
                    assignment_revision: 1,
                    provider_completions: vec![
                        iroha_data_model::sorafs::pin_registry::ReplicationOrderCompletionRecord {
                            provider_id: iroha_data_model::sorafs::capacity::ProviderId::new(
                                provider_id,
                            ),
                            completed_by: issuer.clone(),
                            completion_epoch: 9,
                            assignment_revision: 1,
                            completion_authority: iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionAuthorityV1::new(
                                issuer,
                                iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1 {
                                    policy_id: [0xA1; 32],
                                    revision: 1,
                                    predecessor_digest: None,
                                    policy_digest: [0xA2; 32],
                                },
                            ),
                            finalized_anchor: iroha_data_model::sorafs::pin_registry::ProviderIngestFinalizedAnchorV1 {
                                height: 9,
                                block_hash: [0xA3; 32],
                            },
                        },
                    ],
                    status: iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Completed(9),
                },
            );
        tx.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit projection registry seed block");
    }
    #[derive(Clone)]
    struct ProjectionProviderFixture {
        advert: sorafs_manifest::ProviderAdvertV1,
        envelope: sorafs_manifest::ProviderAdmissionEnvelopeV1,
    }
    impl ProjectionProviderFixture {
        fn provider_id(&self) -> [u8; 32] {
            self.advert.body.provider_id
        }
        fn issued_at(&self) -> u64 {
            self.advert.issued_at
        }
    }
    fn make_projection_provider_fixture(host_pattern: &str) -> ProjectionProviderFixture {
        use ed25519_dalek::{Signer as _, SigningKey};
        let signing_key = SigningKey::from_bytes(&[0xA5; 32]);
        let provider_id = [0x11; 32];
        let stake_pool_id = [0x21; 32];
        let issued_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::ZERO)
            .as_secs()
            .saturating_sub(300);
        let expires_at = issued_at + 3_600;
        let body = sorafs_manifest::ProviderAdvertBodyV1 {
            provider_id,
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
            stake: sorafs_manifest::StakePointer {
                pool_id: stake_pool_id,
                stake_amount: "1000".parse().expect("canonical stake quantity"),
            },
            qos: sorafs_manifest::QosHints {
                availability: sorafs_manifest::AvailabilityTier::Hot,
                max_retrieval_latency_ms: 1_000,
                max_concurrent_streams: 8,
            },
            capabilities: vec![
                sorafs_manifest::CapabilityTlv {
                    cap_type: sorafs_manifest::CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                sorafs_manifest::CapabilityTlv {
                    cap_type: sorafs_manifest::CapabilityType::ChunkRangeFetch,
                    payload: sorafs_manifest::ProviderCapabilityRangeV1 {
                        max_chunk_span: 32,
                        min_granularity: 8,
                        supports_sparse_offsets: true,
                        requires_alignment: false,
                        supports_merkle_proof: true,
                    }
                    .to_bytes()
                    .expect("encode range capability"),
                },
            ],
            endpoints: vec![sorafs_manifest::AdvertEndpoint {
                kind: sorafs_manifest::EndpointKind::Torii,
                host_pattern: host_pattern.to_owned(),
                metadata: vec![sorafs_manifest::EndpointMetadata {
                    key: sorafs_manifest::EndpointMetadataKey::Region,
                    value: b"global".to_vec(),
                }],
            }],
            rendezvous_topics: vec![sorafs_manifest::RendezvousTopic {
                topic: "sorafs.sf1.primary".to_owned(),
                region: "global".to_owned(),
            }],
            path_policy: sorafs_manifest::PathDiversityPolicy {
                min_guard_weight: 5,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: Some(sorafs_manifest::StreamBudgetV1 {
                max_in_flight: 8,
                max_bytes_per_sec: 8_388_608,
                burst_bytes: Some(1_048_576),
            }),
            transport_hints: Some(vec![sorafs_manifest::TransportHintV1 {
                protocol: sorafs_manifest::TransportProtocol::ToriiHttpRange,
                priority: 0,
            }]),
        };
        body.validate().expect("fixture body must validate");
        let mut advert = sorafs_manifest::ProviderAdvertV1 {
            version: sorafs_manifest::PROVIDER_ADVERT_VERSION_V1,
            issued_at,
            expires_at,
            body: body.clone(),
            signature: sorafs_manifest::AdvertSignature {
                algorithm: sorafs_manifest::SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: vec![0; 64],
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let signature_payload = advert
            .signature_payload_bytes()
            .expect("serialize advert signature envelope");
        advert.signature.signature = signing_key.sign(&signature_payload).to_bytes().to_vec();
        let (vrf_public, vrf_private) = iroha_crypto::BlsNormal::try_keypair(
            iroha_crypto::KeyGenOption::UseSeed(provider_id.to_vec()),
        )
        .expect("derive provider VRF fixture key");
        let vrf_pair: iroha_crypto::KeyPair = (vrf_public, vrf_private).into();
        let proposal = sorafs_manifest::ProviderAdmissionProposalV1 {
            version: sorafs_manifest::PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
            provider_id,
            profile_id: body.profile_id.clone(),
            profile_aliases: body.profile_aliases.clone(),
            stake: body.stake.clone(),
            capabilities: body.capabilities.clone(),
            endpoints: vec![sorafs_manifest::EndpointAdmissionV1 {
                endpoint: body.endpoints.first().cloned().expect("advert endpoint"),
                attestation: sorafs_manifest::EndpointAttestationV1 {
                    version: sorafs_manifest::ENDPOINT_ATTESTATION_VERSION_V1,
                    kind: sorafs_manifest::EndpointAttestationKind::Mtls,
                    attested_at: issued_at.saturating_sub(60),
                    expires_at: expires_at + 60,
                    leaf_certificate: vec![0xAA],
                    intermediate_certificates: Vec::new(),
                    alpn_ids: vec!["h2".to_owned()],
                    report: Vec::new(),
                },
            }],
            advert_key: signing_key.verifying_key().to_bytes(),
            por_vrf_key: sorafs_manifest::ProviderVrfPublicKeyV1::BlsNormal(
                vrf_pair
                    .public_key()
                    .to_bytes()
                    .1
                    .try_into()
                    .expect("Normal BLS public key is 48 bytes"),
            ),
            jurisdiction_code: "US".to_owned(),
            contact_uri: Some("mailto:ops@example.test".to_owned()),
            stream_budget: Some(sorafs_manifest::StreamBudgetV1 {
                max_in_flight: 8,
                max_bytes_per_sec: 8_388_608,
                burst_bytes: Some(1_048_576),
            }),
            transport_hints: Some(vec![sorafs_manifest::TransportHintV1 {
                protocol: sorafs_manifest::TransportProtocol::ToriiHttpRange,
                priority: 0,
            }]),
        };
        let proposal_digest =
            sorafs_manifest::compute_proposal_digest(&proposal).expect("proposal digest");
        let advert_body_digest =
            sorafs_manifest::compute_advert_body_digest(&body).expect("advert body digest");
        let council_key = SigningKey::from_bytes(&[0x42; 32]);
        let mut envelope = sorafs_manifest::ProviderAdmissionEnvelopeV1 {
            version: sorafs_manifest::PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
            proposal,
            proposal_digest,
            advert_body: body,
            advert_body_digest,
            issued_at,
            retention_epoch: expires_at + 600,
            council_signatures: Vec::new(),
            notes: None,
        };
        let authorization_digest =
            sorafs_manifest::compute_envelope_authorization_digest(&envelope)
                .expect("envelope authorization digest");
        let council_signature = council_key.sign(&authorization_digest);
        envelope
            .council_signatures
            .push(sorafs_manifest::CouncilSignature {
                signer: council_key.verifying_key().to_bytes(),
                signature: council_signature.to_bytes().to_vec(),
            });
        ProjectionProviderFixture { advert, envelope }
    }
    fn app_state_with_projection_provider_fixture(
        fixture: &ProjectionProviderFixture,
    ) -> (crate::SharedAppState, tempfile::TempDir) {
        let policy = sorafs_manifest::ProviderAdmissionCouncilPolicy::new(
            fixture
                .envelope
                .council_signatures
                .iter()
                .map(|signature| signature.signer),
            1,
        )
        .expect("fixture council policy");
        let admission =
            crate::sorafs::AdmissionRegistry::from_envelopes(policy, [fixture.envelope.clone()])
                .expect("fixture envelope must validate");
        let mut cache = crate::sorafs::ProviderAdvertCache::new(
            vec![
                sorafs_manifest::CapabilityType::ToriiGateway,
                sorafs_manifest::CapabilityType::ChunkRangeFetch,
            ],
            Arc::new(admission),
        );
        let prepared = cache
            .validation_policy()
            .prepare(fixture.advert.clone(), fixture.issued_at())
            .expect("prepare fixture advert");
        cache
            .commit_prepared(prepared, fixture.issued_at())
            .expect("ingest fixture advert");
        let (node, dir) = projection_query_sorafs_node_with_temp_storage();
        (
            crate::tests_runtime_handlers::reconfigure_sorafs_runtime_for_tests(
                crate::mk_app_state_for_tests(),
                Some(Arc::new(tokio::sync::RwLock::new(cache))),
                node,
            ),
            dir,
        )
    }
    async fn publish_asset_holder_checkpoint_with_real_manifests(
        state: &Arc<iroha_core::state::State>,
    ) -> Vec<(QueryProjectionShardArchive, sorafs_manifest::ManifestV1)> {
        let catalog = crate::runtime::handle_node_query_projection_shard_catalog(
            state.clone(),
            "asset_holders".to_owned(),
            crate::runtime::NodeProjectionShardCatalogQuery {
                asset_definition_id: Some("pkr#paynet".to_owned()),
                offset: None,
                limit: None,
            },
        )
        .await
        .expect("projection catalog");
        assert!(
            !catalog.entries.is_empty(),
            "fixture should produce holder shards"
        );
        let mut checkpoint_shards = Vec::new();
        let mut published = Vec::new();
        for (index, entry) in catalog.entries.iter().enumerate() {
            let archive = crate::runtime::handle_node_query_projection_shard_export(
                state.clone(),
                "asset_holders".to_owned(),
                entry.partition_id,
                crate::runtime::NodeProjectionShardExportQuery {
                    asset_definition_id: Some("pkr#paynet".to_owned()),
                },
            )
            .await
            .expect("projection shard export");
            let (_, _, manifest) = query_projection_archive_storage_artifacts(&archive)
                .expect("projection archive storage artifacts");
            let manifest_digest = iroha_data_model::da::types::BlobDigest::new(
                manifest
                    .digest()
                    .expect("digest projection archive manifest")
                    .into(),
            );
            checkpoint_shards.push(
                archive
                    .clone()
                    .into_checkpoint_shard(
                        manifest_digest,
                        iroha_data_model::da::types::StorageTicketId::new(
                            [index.wrapping_add(1) as u8; 32],
                        ),
                    )
                    .expect("checkpoint shard"),
            );
            published.push((archive, manifest));
        }
        state.publish_query_projection_checkpoint(1_714_111_000, checkpoint_shards);
        published
    }
    #[test]
    fn query_projection_archive_decoder_rejects_noncanonical_norito() {
        let archive = QueryProjectionShardArchive::from_index_status(
            iroha_core::query::index_status::QueryIndexStatus::default(),
            1,
            QueryProjectionResourceKind::Accounts,
            0,
            None,
            1,
            b"row".to_vec(),
        );
        let alternate = {
            let flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            norito::to_bytes(&archive).expect("encode alternate-layout archive")
        };
        assert!(norito::decode_from_bytes::<QueryProjectionShardArchive>(&alternate).is_ok());
        let compressed = zstd::bulk::compress(&alternate, 3).expect("compress archive");
        assert!(decode_query_projection_archive_payload(&compressed).is_err());
    }
    fn i105_literal(account_id: &AccountId) -> String {
        let i105 = account_id
            .to_account_address()
            .and_then(|address| address.to_i105())
            .expect("i105 encoding should succeed");
        i105
    }
}
