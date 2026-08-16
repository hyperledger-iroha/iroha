// Exact first-release merge-certificate signer-cardinality tests.

#[test]
fn merge_reference_rejects_equal_vote_subquorum() {
    let (state, block, bundle, profile) = equal_vote_merge_reference_fixture(&[1, 2]);
    let error = ValidBlock::validate_execution_context_merge_reference(
        &block,
        state.network_id_ref(),
        &bundle,
        &profile,
    )
    .expect_err("two signers do not satisfy the three-vote quorum");
    assert!(matches!(
        error,
        BlockValidationError::ExecutionContextInvalid(reason)
            if reason.contains("signer count mismatch")
                && reason.contains("expected exactly 3, got 2")
    ));
}

#[test]
fn merge_reference_rejects_equal_vote_superset() {
    let (state, block, bundle, profile) = equal_vote_merge_reference_fixture(&[0, 1, 2, 3]);
    let error = ValidBlock::validate_execution_context_merge_reference(
        &block,
        state.network_id_ref(),
        &bundle,
        &profile,
    )
    .expect_err("four signers must not pad a three-vote wire certificate");
    assert!(matches!(
        error,
        BlockValidationError::ExecutionContextInvalid(reason)
            if reason.contains("signer count mismatch")
                && reason.contains("expected exactly 3, got 4")
    ));
}
