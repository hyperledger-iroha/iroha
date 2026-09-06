#[cfg(all(test, feature = "app_api"))]
#[test]
fn exact_field_filter_candidates_extracts_safe_account_constraints() {
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
        FieldPath("id".to_owned()),
        norito::json::Value::from(first.to_string()),
    );
    let candidates = exact_field_filter_candidates::<AccountId>(Some(&exact), "id")
        .expect("account id equality should produce direct lookup candidates");
    assert_eq!(candidates, BTreeSet::from([first.clone()]));
    let combined = FilterExpr::And(vec![
        exact.clone(),
        FilterExpr::Eq(
            FieldPath("has_primary_alias".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    let candidates = exact_field_filter_candidates::<AccountId>(Some(&combined), "id")
        .expect("AND should preserve safe account id candidates");
    assert_eq!(candidates, BTreeSet::from([first]));
    let many = FilterExpr::In(
        FieldPath("id".to_owned()),
        vec![
            norito::json::Value::from("not-an-account-id"),
            norito::json::Value::from(second.to_string()),
        ],
    );
    let candidates = exact_field_filter_candidates::<AccountId>(Some(&many), "id")
        .expect("account id IN should produce candidates");
    assert_eq!(candidates, BTreeSet::from([second]));
    let unsafe_or = FilterExpr::Or(vec![
        exact,
        FilterExpr::Eq(
            FieldPath("has_primary_alias".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    assert!(exact_field_filter_candidates::<AccountId>(Some(&unsafe_or), "id").is_none());
}
