#[cfg(all(test, feature = "app_api"))]
mod accounts_query_tests {
    use std::sync::Arc;

    use axum::http::StatusCode;
    use http_body_util::BodyExt as _;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::Algorithm;
    use iroha_data_model::prelude as dm;

    use super::*;

    fn checked_accounts_query_authority(seed: u8, context: &'static str) -> dm::AccountId {
        dm::AccountId::new(
            checked_routing_fixture_keypair(seed, Algorithm::Ed25519, context)
                .public_key()
                .clone(),
        )
    }

    #[tokio::test]
    async fn accounts_query_streams_without_sort() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let domain_id: dm::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let exec_authority =
            checked_accounts_query_authority(0xB0, "derive accounts-query executor key");
        let exec_id = exec_authority.clone();
        let domain = dm::Domain::new(domain_id.clone()).build(&exec_authority);
        let mut accounts = vec![dm::Account::new(exec_id.account().clone()).build(&exec_authority)];
        for seed in 0xB1..=0xB5 {
            let authority = checked_accounts_query_authority(
                seed,
                "derive accounts-query streamed account key",
            );
            let account_id = authority.clone();
            accounts.push(dm::Account::new(account_id.account().clone()).build(&authority));
        }
        let state = Arc::new(State::new_for_testing(
            World::with([domain], accounts, []),
            kura,
            query,
        ));

        // Query with limit + fetch_size smaller than number of accounts.
        let env = crate::filter::QueryEnvelope {
            query: None,
            filter: None,
            select: None,
            aggregate: None,
            sort: Vec::new(),
            pagination: crate::filter::Pagination {
                limit: Some(2),
                offset: 0,
            },
            fetch_size: Some(2),
            count_mode: Some("exact".to_owned()),
        };
        let resp = handle_v1_accounts_query(
            state,
            crate::utils::extractors::NoritoJson(env),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok")
        .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let doc: norito::json::Value = norito::json::from_slice(&body).unwrap();
        assert_eq!(doc["items"].as_array().unwrap().len(), 2);
        assert_eq!(doc["total"].as_u64(), Some(4));
    }

    #[tokio::test]
    async fn accounts_query_filter_accepts_canonical_and_alias_and_rejects_non_canonical_i105_literals()
     {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let domain_id: dm::DomainId =
            DomainId::try_new("aliases", "universal").expect("valid domain");
        let exec_authority =
            checked_accounts_query_authority(0xB6, "derive accounts-query alias executor key");
        let exec_id = exec_authority.clone();
        let domain = dm::Domain::new(domain_id.clone()).build(&exec_authority);

        let labelled_authority =
            checked_accounts_query_authority(0xB7, "derive accounts-query labelled account key");
        let account_id = labelled_authority.clone();
        let label = dm::AccountAlias::new(
            "primary".parse().expect("valid label name"),
            Some(iroha_data_model::account::rekey::AccountAliasDomain::new(
                domain_id.name().clone(),
            )),
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        );
        let state = Arc::new(State::new_for_testing(
            World::with(
                [domain],
                [
                    dm::Account::new(exec_id.account().clone()).build(&exec_authority),
                    dm::Account::new(account_id.account().clone()).build(&labelled_authority),
                ],
                [],
            ),
            kura,
            query,
        ));

        let expected = account_id.account().to_string();
        let non_canonical_i105_literal = expected.replacen("sora", "ｓｏｒａ", 1);

        let alias_literal = label
            .to_literal(&state.nexus_snapshot().dataspace_catalog)
            .expect("canonical alias literal");
        bind_account_alias_for_test(&state, &account_id, &alias_literal);
        let alias_env = crate::filter::QueryEnvelope {
            query: None,
            filter: Some(crate::filter::FilterExpr::Eq(
                crate::filter::FieldPath("id".to_string()),
                Value::String(alias_literal.clone()),
            )),
            select: None,
            aggregate: None,
            sort: Vec::new(),
            pagination: crate::filter::Pagination {
                limit: Some(8),
                offset: 0,
            },
            fetch_size: None,
            count_mode: None,
        };
        let alias_result = handle_v1_accounts_query(
            state.clone(),
            crate::utils::extractors::NoritoJson(alias_env),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await
        .expect("alias handler ok")
        .into_response();
        assert_eq!(
            alias_result.status(),
            StatusCode::OK,
            "alias literal `{alias_literal}` should resolve to the canonical account id"
        );
        let alias_body = alias_result
            .into_body()
            .collect()
            .await
            .expect("alias body bytes")
            .to_bytes();
        let alias_doc: norito::json::Value =
            norito::json::from_slice(&alias_body).expect("valid alias JSON");
        let alias_ids: Vec<String> = alias_doc
            .get("items")
            .and_then(norito::json::Value::as_array)
            .expect("alias items array")
            .iter()
            .filter_map(|item| {
                item.get("id")
                    .and_then(norito::json::Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        assert!(
            alias_ids.iter().any(|id| id == &expected),
            "alias literal `{alias_literal}` should resolve to `{expected}`, got {alias_ids:?}"
        );
        assert!(
            alias_ids.iter().all(|id| !id.contains('@')),
            "alias queries must still return canonical account ids, got {alias_ids:?}"
        );

        let canonical_env = crate::filter::QueryEnvelope {
            query: None,
            filter: Some(crate::filter::FilterExpr::Eq(
                crate::filter::FieldPath("id".to_string()),
                Value::String(expected.clone()),
            )),
            select: None,
            aggregate: None,
            sort: Vec::new(),
            pagination: crate::filter::Pagination {
                limit: Some(8),
                offset: 0,
            },
            fetch_size: None,
            count_mode: None,
        };
        let resp = handle_v1_accounts_query(
            state.clone(),
            crate::utils::extractors::NoritoJson(canonical_env),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok")
        .into_response();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "canonical literal `{expected}` should be accepted"
        );
        let body = resp
            .into_body()
            .collect()
            .await
            .expect("body bytes")
            .to_bytes();
        let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid JSON");
        let items = doc
            .get("items")
            .and_then(norito::json::Value::as_array)
            .expect("items array");
        let ids: Vec<String> = items
            .iter()
            .filter_map(|item| {
                item.get("id")
                    .and_then(norito::json::Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        assert!(
            ids.iter().any(|id| id == &expected),
            "canonical literal `{expected}` should resolve to `{expected}`, got {ids:?}"
        );
        assert!(
            ids.iter().all(|id| !id.contains('@')),
            "response should expose canonical ids, got {ids:?}"
        );

        let i105_env = crate::filter::QueryEnvelope {
            query: None,
            filter: Some(crate::filter::FilterExpr::Eq(
                crate::filter::FieldPath("id".to_string()),
                Value::String(non_canonical_i105_literal.clone()),
            )),
            select: None,
            aggregate: None,
            sort: Vec::new(),
            pagination: crate::filter::Pagination {
                limit: Some(8),
                offset: 0,
            },
            fetch_size: None,
            count_mode: None,
        };
        let i105_result = handle_v1_accounts_query(
            state.clone(),
            crate::utils::extractors::NoritoJson(i105_env),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await;
        assert!(
            i105_result.is_err(),
            "non-canonical I105 literal `{non_canonical_i105_literal}` must be rejected"
        );
    }

    #[tokio::test]
    async fn accounts_list_filter_accepts_alias_and_returns_canonical_i105_ids() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let domain_id: dm::DomainId =
            DomainId::try_new("aliases-list", "universal").expect("valid domain");
        let exec_authority =
            checked_accounts_query_authority(0xB8, "derive accounts-list alias executor key");
        let exec_id = exec_authority.clone();
        let domain = dm::Domain::new(domain_id.clone()).build(&exec_authority);

        let labelled_authority =
            checked_accounts_query_authority(0xB9, "derive accounts-list labelled account key");
        let account_id = labelled_authority.clone();
        let label = dm::AccountAlias::new(
            "primary".parse().expect("valid label name"),
            Some(iroha_data_model::account::rekey::AccountAliasDomain::new(
                domain_id.name().clone(),
            )),
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        );
        let state = Arc::new(State::new_for_testing(
            World::with(
                [domain],
                [
                    dm::Account::new(exec_id.account().clone()).build(&exec_authority),
                    dm::Account::new(account_id.account().clone()).build(&labelled_authority),
                ],
                [],
            ),
            kura,
            query,
        ));

        let expected = account_id.account().to_string();
        let alias_literal = label
            .to_literal(&state.nexus_snapshot().dataspace_catalog)
            .expect("canonical alias literal");
        bind_account_alias_for_test(&state, &account_id, &alias_literal);
        let filter = crate::filter::FilterExpr::Eq(
            crate::filter::FieldPath("id".to_owned()),
            Value::String(alias_literal.clone()),
        );
        let params = ListFilterParams {
            filter: Some(
                norito::json::to_string(
                    &norito::json::to_value(&filter).expect("filter should encode to JSON value"),
                )
                .expect("filter JSON"),
            ),
            limit: Some(8),
            offset: 0,
            sort: None,
            count_mode: None,
        };

        let response = handle_v1_accounts(
            state,
            crate::NoritoQuery(params),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await
        .expect("list handler ok")
        .into_response();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "alias literal `{alias_literal}` should resolve on GET /v1/accounts"
        );
        let body = response
            .into_body()
            .collect()
            .await
            .expect("response bytes")
            .to_bytes();
        let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid list JSON");
        let ids: Vec<String> = doc
            .get("items")
            .and_then(norito::json::Value::as_array)
            .expect("items array")
            .iter()
            .filter_map(|item| {
                item.get("id")
                    .and_then(norito::json::Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        assert!(
            ids.iter().any(|id| id == &expected),
            "alias literal `{alias_literal}` should resolve to `{expected}`, got {ids:?}"
        );
        assert!(
            ids.iter().all(|id| !id.contains('@')),
            "GET /v1/accounts must still emit canonical account ids, got {ids:?}"
        );
    }

    #[tokio::test]
    async fn accounts_query_aggregate_groups_by_primary_alias_domain() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let domain_id: dm::DomainId =
            DomainId::try_new("aggregate-aliases", "universal").expect("valid domain");
        let exec_authority =
            checked_accounts_query_authority(0xBA, "derive accounts-aggregate executor key");
        let exec_id = exec_authority.clone();
        let domain = dm::Domain::new(domain_id).build(&exec_authority);

        let hbl_authority =
            checked_accounts_query_authority(0xBB, "derive accounts-aggregate hbl account key");
        let hbl_id = hbl_authority.clone();
        let ubl_authority =
            checked_accounts_query_authority(0xBC, "derive accounts-aggregate ubl account key");
        let ubl_id = ubl_authority.clone();

        let state = Arc::new(State::new_for_testing(
            World::with(
                [domain],
                [
                    dm::Account::new(exec_id.account().clone()).build(&exec_authority),
                    dm::Account::new(hbl_id.account().clone()).build(&hbl_authority),
                    dm::Account::new(ubl_id.account().clone()).build(&ubl_authority),
                ],
                [],
            ),
            kura,
            query,
        ));
        bind_account_alias_for_test(&state, &hbl_id, "alice@hbl.universal");
        bind_account_alias_for_test(&state, &ubl_id, "bob@ubl.universal");

        let env = crate::filter::QueryEnvelope {
            query: None,
            filter: Some(crate::filter::FilterExpr::In(
                crate::filter::FieldPath("primary_alias_domain".into()),
                vec![
                    norito::json::Value::from("hbl.universal"),
                    norito::json::Value::from("ubl.universal"),
                ],
            )),
            select: None,
            aggregate: Some(crate::filter::AggregateSpec {
                group_by: vec![crate::filter::FieldPath("primary_alias_domain".into())],
                metrics: vec![
                    crate::filter::AggregateMetric {
                        alias: "row_count".into(),
                        r#fn: crate::filter::AggregateFn::Count,
                        field: None,
                    },
                    crate::filter::AggregateMetric {
                        alias: "user_count".into(),
                        r#fn: crate::filter::AggregateFn::DistinctCount,
                        field: Some(crate::filter::FieldPath("id".into())),
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
            count_mode: Some("exact".to_owned()),
        };

        let response = handle_v1_accounts_query(
            state,
            crate::utils::extractors::NoritoJson(env),
            crate::routing::MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("response bytes")
            .to_bytes();
        let doc: norito::json::Value = norito::json::from_slice(&body).expect("valid JSON");

        assert_eq!(doc["total"].as_u64(), Some(2));
        assert!(doc["indexed_height"].as_u64().is_some());
        assert!(doc["indexed_block_hash"].is_string() || doc["indexed_block_hash"].is_null());

        let items = doc["items"].as_array().expect("items array");
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0]["primary_alias_domain"].as_str(),
            Some("hbl.universal")
        );
        assert_eq!(items[0]["row_count"].as_u64(), Some(1));
        assert_eq!(items[0]["user_count"].as_u64(), Some(1));
        assert_eq!(
            items[1]["primary_alias_domain"].as_str(),
            Some("ubl.universal")
        );
        assert_eq!(items[1]["row_count"].as_u64(), Some(1));
        assert_eq!(items[1]["user_count"].as_u64(), Some(1));
    }
}
