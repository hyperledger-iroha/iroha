#[cfg(all(test, feature = "app_api"))]
#[test]
fn account_filter_projection_preserves_exact_and_boolean_constraints() {
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
    let first_projection = AccountListItem {
        canonical_id: first.to_string(),
        display_id: crate::account_literal::display_literal(&first),
        primary_alias: Default::default(),
    };
    let second_projection = AccountListItem {
        canonical_id: second.to_string(),
        display_id: crate::account_literal::display_literal(&second),
        primary_alias: Default::default(),
    };
    let exact = FilterExpr::Eq(
        FieldPath("id".to_owned()),
        norito::json::Value::from(first.to_string()),
    );
    assert!(account_filter_projection(&exact, &first_projection));
    assert!(!account_filter_projection(&exact, &second_projection));
    let combined = FilterExpr::And(vec![
        exact.clone(),
        FilterExpr::Eq(
            FieldPath("has_primary_alias".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    assert!(account_filter_projection(&combined, &first_projection));
    assert!(!account_filter_projection(&combined, &second_projection));
    let many = FilterExpr::In(
        FieldPath("id".to_owned()),
        vec![
            norito::json::Value::from("not-an-account-id"),
            norito::json::Value::from(second.to_string()),
        ],
    );
    assert!(!account_filter_projection(&many, &first_projection));
    assert!(account_filter_projection(&many, &second_projection));
    let disjunction = FilterExpr::Or(vec![
        exact,
        FilterExpr::Eq(
            FieldPath("has_primary_alias".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    assert!(account_filter_projection(&disjunction, &first_projection));
    assert!(account_filter_projection(&disjunction, &second_projection));
}
