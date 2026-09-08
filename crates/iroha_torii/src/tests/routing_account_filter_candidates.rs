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

#[cfg(all(test, feature = "app_api"))]
#[test]
fn exact_field_filter_candidates_extracts_safe_nft_constraints() {
    let first: NftId = "ticket$art".parse().expect("valid first NFT id");
    let second: NftId = "receipt$art".parse().expect("valid second NFT id");
    let exact = FilterExpr::Eq(
        FieldPath("id".to_owned()),
        norito::json::Value::from(first.to_string()),
    );
    let candidates = exact_field_filter_candidates::<NftId>(Some(&exact), "id", &|value| {
        value.as_str()?.parse().ok()
    })
    .expect("NFT id equality should produce direct lookup candidates");
    assert_eq!(candidates, BTreeSet::from([first.clone()]));
    let combined = FilterExpr::And(vec![
        exact.clone(),
        FilterExpr::Eq(
            FieldPath("has_metadata".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    let candidates = exact_field_filter_candidates::<NftId>(Some(&combined), "id", &|value| {
        value.as_str()?.parse().ok()
    })
    .expect("AND should preserve safe NFT id candidates");
    assert_eq!(candidates, BTreeSet::from([first]));
    let many = FilterExpr::In(
        FieldPath("id".to_owned()),
        vec![
            norito::json::Value::from("not-an-nft-id"),
            norito::json::Value::from(second.to_string()),
        ],
    );
    let candidates = exact_field_filter_candidates::<NftId>(Some(&many), "id", &|value| {
        value.as_str()?.parse().ok()
    })
    .expect("NFT id IN should produce candidates");
    assert_eq!(candidates, BTreeSet::from([second]));
    let unsafe_or = FilterExpr::Or(vec![
        exact,
        FilterExpr::Eq(
            FieldPath("has_metadata".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    assert!(
        exact_field_filter_candidates::<NftId>(Some(&unsafe_or), "id", &|value| value
            .as_str()?
            .parse()
            .ok())
        .is_none()
    );
}
