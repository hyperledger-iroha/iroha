use iroha_data_model::query::error::QueryExecutionFail;

#[tokio::test]
async fn block_and_header_queries_fail_on_hash_only_history_gap() -> Result<()> {
    let state = state_with_test_blocks_and_transactions(3, 1, 1)?;
    state
        .kura()
        .force_hash_only_block_for_testing(nonzero!(2_usize))
        .expect("convert middle query fixture block to hash-only form");
    let state_view = state.view();
    let blocks_error = ValidQuery::execute(FindBlocks, CompoundPredicate::PASS, &state_view)
        .err()
        .expect("block query must reject a canonical history gap");
    let headers_error = ValidQuery::execute(FindBlockHeaders, CompoundPredicate::PASS, &state_view)
        .err()
        .expect("header query must reject a canonical history gap");
    assert!(matches!(
        &blocks_error,
        QueryExecutionFail::CanonicalHistory(
            iroha_data_model::query::error::CanonicalHistoryError::HashOnlyBodyUnavailable {
                height: 2,
                ..
            }
        )
    ));
    assert_eq!(headers_error, blocks_error);
    Ok(())
}

#[tokio::test]
async fn find_block_header_by_hash() -> Result<()> {
    let state = state_with_test_blocks_and_transactions(1, 1, 1)?;
    let state_view = state.view();
    let block = state_view
        .all_blocks(nonzero!(1_usize))
        .last()
        .expect("state is empty")
        .expect("canonical block history must be available");
    let mut headers = FindBlockHeaders::new()
        .execute(CompoundPredicate::PASS, &state_view)
        .expect("Query execution should not fail");
    let found = headers.any(|header| header.hash() == block.hash());
    assert!(found, "Query should return the block header");
    let unexpected_hash = HashOf::from_untyped_unchecked(Hash::new([42]));
    let missing = FindBlockHeaders::new()
        .execute(CompoundPredicate::PASS, &state_view)
        .expect("Query execution should not fail")
        .any(|header| header.hash() == unexpected_hash);
    assert!(!missing, "Block header should not be found");
    Ok(())
}
