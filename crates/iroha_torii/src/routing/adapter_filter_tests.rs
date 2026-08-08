// App-API query adapter filter regressions.

#[cfg(all(test, feature = "app_api"))]
mod adapter_filter_tests {
    use super::*;
    #[cfg(feature = "app_api")]
    use crate::filter::FieldPath;
    use crate::{json_array, json_object, json_value};
    #[cfg(feature = "app_api")]
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};

    fn obj(pairs: Vec<(&'static str, Value)>) -> Value {
        json_object(pairs)
    }

    fn arr(values: Vec<Value>) -> Value {
        json_array(values)
    }

    fn val<T: json::JsonSerialize + ?Sized>(value: &T) -> Value {
        json_value(value)
    }

    #[test]
    fn accounts_filter_adapter_accepts_id_eq_and_rejects_lt() {
        let ok = obj(vec![
            ("op", val("eq")),
            (
                "args",
                arr(vec![
                    val("id"),
                    val("sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"),
                ]),
            ),
        ]);
        let expr: FilterExpr = norito::json::value::from_value(ok).unwrap();
        crate::filter::validate_filter(&expr).unwrap();
        #[cfg(feature = "app_api")]
        validate_accounts_filter_adapter(&expr).unwrap();

        let bad = obj(vec![
            ("op", val("lt")),
            ("args", arr(vec![val("id"), val(&5u64)])),
        ]);
        let expr2: FilterExpr = norito::json::value::from_value(bad).unwrap();
        crate::filter::validate_filter(&expr2).unwrap();
        #[cfg(feature = "app_api")]
        assert!(validate_accounts_filter_adapter(&expr2).is_err());
    }

    #[test]
    fn defs_filter_adapter_accepts_in_and_rejects_numeric() {
        let ok = obj(vec![
            ("op", val("in")),
            (
                "args",
                arr(vec![
                    val("id"),
                    arr(vec![
                        val(&test_asset_definition_literal_from_hex(
                            "550e8400e29b41d4a7164466554400dd",
                        )),
                        val(&test_asset_definition_literal_from_hex(
                            "550e8400e29b41d4a7164466554400ee",
                        )),
                    ]),
                ]),
            ),
        ]);
        let expr: FilterExpr = norito::json::value::from_value(ok).unwrap();
        crate::filter::validate_filter(&expr).unwrap();
        #[cfg(feature = "app_api")]
        validate_defs_filter_adapter(&expr).unwrap();

        let bad = obj(vec![
            ("op", val("gte")),
            ("args", arr(vec![val("id"), val(&1u64)])),
        ]);
        let expr2: FilterExpr = norito::json::value::from_value(bad).unwrap();
        crate::filter::validate_filter(&expr2).unwrap();
        #[cfg(feature = "app_api")]
        assert!(validate_defs_filter_adapter(&expr2).is_err());
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn defs_filter_adapter_accepts_name_alias_and_metadata_nullability() {
        let name_eq = FilterExpr::Eq(FieldPath("name".into()), Value::from("CBDC"));
        validate_defs_filter_adapter(&name_eq).unwrap();

        let alias_null = FilterExpr::IsNull(FieldPath("alias".into()));
        validate_defs_filter_adapter(&alias_null).unwrap();

        let alias_binding_status = FilterExpr::Eq(
            FieldPath("alias_binding.status".into()),
            Value::from("leased_grace"),
        );
        validate_defs_filter_adapter(&alias_binding_status).unwrap();

        let alias_binding_bound_at = FilterExpr::Gt(
            FieldPath("alias_binding.bound_at_ms".into()),
            Value::from(10_u64),
        );
        validate_defs_filter_adapter(&alias_binding_bound_at).unwrap();

        let metadata_lt = FilterExpr::Lt(FieldPath("metadata.rank".into()), Value::from(2_u64));
        validate_defs_filter_adapter(&metadata_lt).unwrap();

        let bad = FilterExpr::IsNull(FieldPath("name".into()));
        assert!(validate_defs_filter_adapter(&bad).is_err());

        let bad_alias_binding = FilterExpr::Eq(
            FieldPath("alias_binding.bound_at_ms".into()),
            Value::from("10"),
        );
        assert!(validate_defs_filter_adapter(&bad_alias_binding).is_err());
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn defs_filter_projection_matches_name_alias_and_metadata_passthrough() {
        let authority = AccountId::new(
            checked_routing_fixture_keypair(
                0xF2,
                Algorithm::Ed25519,
                "derive asset-definition projection authority fixture key",
            )
            .public_key()
            .clone(),
        );
        let definition = AssetDefinition::numeric(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("issuer", "universal").expect("domain"),
                "cbdc".parse().expect("name"),
            ),
            "CBDC".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority);
        let item = AssetDefinitionListItem {
            definition,
            id: "66owaQmAQMuHxPzxUN3bqZ6FJfDa".to_owned(),
            name: "CBDC".to_owned(),
            alias: Some("CBDC#centralbank".to_owned()),
            alias_binding: Some(AssetAliasBindingDto {
                alias: "CBDC#centralbank".to_owned(),
                status: "leased_grace".to_owned(),
                lease_expiry_ms: Some(100),
                grace_until_ms: Some(200),
                bound_at_ms: 50,
            }),
        };

        assert!(asset_definition_filter_projection(
            &FilterExpr::Eq(FieldPath("name".into()), Value::from("CBDC")),
            &item,
        ));
        assert!(asset_definition_filter_projection(
            &FilterExpr::Eq(FieldPath("alias".into()), Value::from("CBDC#centralbank"),),
            &item,
        ));
        assert!(asset_definition_filter_projection(
            &FilterExpr::Eq(
                FieldPath("alias_binding.status".into()),
                Value::from("leased_grace"),
            ),
            &item,
        ));
        assert!(asset_definition_filter_projection(
            &FilterExpr::Gt(
                FieldPath("alias_binding.bound_at_ms".into()),
                Value::from(10_u64)
            ),
            &item,
        ));
        assert!(asset_definition_filter_projection(
            &FilterExpr::Exists(FieldPath("metadata.rank".into())),
            &item,
        ));
        assert!(asset_definition_filter_projection(
            &FilterExpr::Lt(FieldPath("metadata.rank".into()), Value::from(2_u64)),
            &item,
        ));
        assert!(!asset_definition_filter_projection(
            &FilterExpr::IsNull(FieldPath("alias".into())),
            &item,
        ));
        assert!(!asset_definition_filter_projection(
            &FilterExpr::IsNull(FieldPath("alias_binding.status".into())),
            &item,
        ));
    }

    #[test]
    fn nfts_filter_adapter_accepts_exists_and_rejects_is_null() {
        let ok = obj(vec![("op", val("exists")), ("args", val("id"))]);
        let expr: FilterExpr = norito::json::value::from_value(ok).unwrap();
        crate::filter::validate_filter(&expr).unwrap();
        #[cfg(feature = "app_api")]
        validate_nfts_filter_adapter(&expr).unwrap();

        let bad = obj(vec![("op", val("is_null")), ("args", val("id"))]);
        let expr2: FilterExpr = norito::json::value::from_value(bad).unwrap();
        crate::filter::validate_filter(&expr2).unwrap();
        #[cfg(feature = "app_api")]
        assert!(validate_nfts_filter_adapter(&expr2).is_err());
    }

    #[test]
    fn rwas_filter_adapter_accepts_exists_and_rejects_is_null() {
        let ok = obj(vec![("op", val("exists")), ("args", val("id"))]);
        let expr: FilterExpr = norito::json::value::from_value(ok).unwrap();
        crate::filter::validate_filter(&expr).unwrap();
        #[cfg(feature = "app_api")]
        validate_rwas_filter_adapter(&expr).unwrap();

        let bad = obj(vec![("op", val("is_null")), ("args", val("id"))]);
        let expr2: FilterExpr = norito::json::value::from_value(bad).unwrap();
        crate::filter::validate_filter(&expr2).unwrap();
        #[cfg(feature = "app_api")]
        assert!(validate_rwas_filter_adapter(&expr2).is_err());
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn asset_holder_filter_adapter_accepts_asset_and_scope_eq() {
        use iroha_test_samples::ALICE_ID;

        let asset_def = AssetDefinitionId::derive_from_components(
            DomainId::try_new("issuer", "universal").expect("domain"),
            "cbdc".parse().expect("name"),
        );
        let expr = FilterExpr::Eq(
            FieldPath("asset".into()),
            Value::from(asset_def.to_string()),
        );
        validate_holders_filter_adapter(&expr).unwrap();
        let scope_expr = FilterExpr::Eq(FieldPath("scope".into()), Value::from("global"));
        validate_holders_filter_adapter(&scope_expr).unwrap();

        let bad = FilterExpr::Eq(FieldPath("asset".into()), Value::from("not-an-asset"));
        assert!(validate_holders_filter_adapter(&bad).is_err());
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn asset_holder_filter_matches_asset_and_scope() {
        use iroha_test_samples::ALICE_ID;

        let asset_def = AssetDefinitionId::derive_from_components(
            DomainId::try_new("issuer", "universal").expect("domain"),
            "cbdc".parse().expect("name"),
        );
        let item = AssetHolderListItem {
            account_id: ALICE_ID.clone(),
            canonical_id: ALICE_ID.to_string(),
            asset: asset_def.to_string(),
            asset_alias: Some("cbdc#issuer.main".to_owned()),
            scope: "global".to_owned(),
            quantity: iroha_primitives::numeric::Quantity::from(10_u32),
            primary_alias: PrimaryAliasProjection::default(),
        };
        let expr = FilterExpr::Eq(
            FieldPath("asset".into()),
            Value::from(asset_def.to_string()),
        );
        assert!(filter_asset_holder_item(&expr, &item));
        let scope_expr = FilterExpr::Eq(FieldPath("scope".into()), Value::from("global"));
        assert!(filter_asset_holder_item(&scope_expr, &item));

        let other_def = AssetDefinitionId::derive_from_components(
            DomainId::try_new("issuer", "universal").expect("domain"),
            "usd".parse().expect("name"),
        );
        let expr2 = FilterExpr::Eq(
            FieldPath("asset".into()),
            Value::from(other_def.to_string()),
        );
        assert!(!filter_asset_holder_item(&expr2, &item));
    }

    #[test]
    fn sort_spec_parser_parses_keys_and_orders() {
        let spec = "metadata.display_name:desc,id:asc,unknown";
        #[cfg(feature = "app_api")]
        let parsed = parse_sort_spec(spec);
        #[cfg(feature = "app_api")]
        {
            assert_eq!(parsed.len(), 3);
            assert_eq!(parsed[0].key.0, "metadata.display_name");
            assert!(matches!(parsed[0].order, crate::filter::Order::Desc));
            assert_eq!(parsed[1].key.0, "id");
            assert!(matches!(parsed[1].order, crate::filter::Order::Asc));
            assert_eq!(parsed[2].key.0, "unknown");
        }
    }
}
