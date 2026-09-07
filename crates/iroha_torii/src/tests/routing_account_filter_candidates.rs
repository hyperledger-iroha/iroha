#[cfg(all(test, feature = "app_api"))]
#[test]
fn asset_holder_filter_candidates_extracts_safe_account_constraints() {
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
    let candidates = asset_holder_filter_account_candidates(Some(&exact))
        .expect("account id equality should produce direct lookup candidates");
    assert_eq!(candidates, BTreeSet::from([first.clone()]));
    let combined = FilterExpr::And(vec![
        exact.clone(),
        FilterExpr::Eq(
            FieldPath("has_primary_alias".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    let candidates = asset_holder_filter_account_candidates(Some(&combined))
        .expect("AND should preserve safe account id candidates");
    assert_eq!(candidates, BTreeSet::from([first]));
    let many = FilterExpr::In(
        FieldPath("account_id".to_owned()),
        vec![
            norito::json::Value::from("not-an-account-id"),
            norito::json::Value::from(second.to_string()),
        ],
    );
    let candidates = asset_holder_filter_account_candidates(Some(&many))
        .expect("account id IN should produce candidates");
    assert_eq!(candidates, BTreeSet::from([second]));
    let unsafe_or = FilterExpr::Or(vec![
        exact,
        FilterExpr::Eq(
            FieldPath("has_primary_alias".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    assert!(asset_holder_filter_account_candidates(Some(&unsafe_or)).is_none());
}
