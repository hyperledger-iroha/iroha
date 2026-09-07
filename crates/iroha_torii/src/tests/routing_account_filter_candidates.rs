#[cfg(all(test, feature = "app_api"))]
#[test]
fn exact_field_filter_candidates_extracts_safe_account_constraints() {
    let extractors: [fn(Option<&FilterExpr>) -> Option<BTreeSet<AccountId>>; 2] =
        [asset_holder_filter_account_candidates, |expr| {
            exact_field_filter_candidates(expr, "account_id", &account_id_from_filter_value)
        }];
    for extract_candidates in extractors {
        let first = AccountId::new(
            checked_routing_fixture_keypair(
                0xF0,
                Algorithm::Ed25519,
                "derive account-filter first candidate fixture key",
            )
            .public_key()
            .clone(),
        );
        let second = AccountId::new(
            checked_routing_fixture_keypair(
                0xF1,
                Algorithm::Ed25519,
                "derive account-filter second candidate fixture key",
            )
            .public_key()
            .clone(),
        );
        let exact = FilterExpr::Eq(
            FieldPath("account_id".to_owned()),
            norito::json::Value::from(first.to_string()),
        );
        let candidates = extract_candidates(Some(&exact))
            .expect("account id equality should produce direct lookup candidates");
        assert_eq!(candidates, BTreeSet::from([first.clone()]));
        let combined = FilterExpr::And(vec![
            exact.clone(),
            FilterExpr::Eq(
                FieldPath("has_primary_alias".to_owned()),
                norito::json::Value::from(false),
            ),
        ]);
        let candidates = extract_candidates(Some(&combined))
            .expect("AND should preserve safe account id candidates");
        assert_eq!(candidates, BTreeSet::from([first]));
        let many = FilterExpr::In(
            FieldPath("account_id".to_owned()),
            vec![
                norito::json::Value::from("not-an-account-id"),
                norito::json::Value::from(second.to_string()),
            ],
        );
        let candidates =
            extract_candidates(Some(&many)).expect("account id IN should produce candidates");
        assert_eq!(candidates, BTreeSet::from([second]));
        let unsafe_or = FilterExpr::Or(vec![
            exact,
            FilterExpr::Eq(
                FieldPath("has_primary_alias".to_owned()),
                norito::json::Value::from(false),
            ),
        ]);
        assert!(extract_candidates(Some(&unsafe_or)).is_none());
        for invalid_value in [
            norito::json::Value::from("not-an-account-id"),
            norito::json::Value::from(false),
        ] {
            let invalid_exact = FilterExpr::Eq(FieldPath("account_id".to_owned()), invalid_value);
            assert_eq!(
                extract_candidates(Some(&invalid_exact)),
                Some(BTreeSet::new()),
                "invalid exact account predicates cannot match any account",
            );
            let invalid_or = FilterExpr::Or(vec![
                invalid_exact,
                FilterExpr::Eq(
                    FieldPath("has_primary_alias".to_owned()),
                    norito::json::Value::from(false),
                ),
            ]);
            assert!(
                extract_candidates(Some(&invalid_or)).is_none(),
                "invalid account literals must not hide an unconstrained OR branch",
            );
        }
    }
}
