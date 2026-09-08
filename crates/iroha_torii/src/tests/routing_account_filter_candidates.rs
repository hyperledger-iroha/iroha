#[cfg(all(test, feature = "app_api"))]
#[test]
fn exact_field_filter_candidates_extracts_safe_nft_constraints() {
    let first: NftId = "ticket$art".parse().expect("valid first NFT id");
    let second: NftId = "receipt$art".parse().expect("valid second NFT id");
    let exact = FilterExpr::Eq(
        FieldPath("id".to_owned()),
        norito::json::Value::from(first.to_string()),
    );
    let candidates = exact_field_filter_candidates::<NftId>(Some(&exact), "id")
        .expect("NFT id equality should produce direct lookup candidates");
    assert_eq!(candidates, BTreeSet::from([first.clone()]));
    let combined = FilterExpr::And(vec![
        exact.clone(),
        FilterExpr::Eq(
            FieldPath("has_metadata".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    let candidates = exact_field_filter_candidates::<NftId>(Some(&combined), "id")
        .expect("AND should preserve safe NFT id candidates");
    assert_eq!(candidates, BTreeSet::from([first]));
    let many = FilterExpr::In(
        FieldPath("id".to_owned()),
        vec![
            norito::json::Value::from("not-an-nft-id"),
            norito::json::Value::from(second.to_string()),
        ],
    );
    let candidates = exact_field_filter_candidates::<NftId>(Some(&many), "id")
        .expect("NFT id IN should produce candidates");
    assert_eq!(candidates, BTreeSet::from([second]));
    let unsafe_or = FilterExpr::Or(vec![
        exact,
        FilterExpr::Eq(
            FieldPath("has_metadata".to_owned()),
            norito::json::Value::from(false),
        ),
    ]);
    assert!(exact_field_filter_candidates::<NftId>(Some(&unsafe_or), "id").is_none());
}
