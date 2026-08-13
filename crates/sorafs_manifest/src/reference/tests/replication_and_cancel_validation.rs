// Replication-order and CancelAssetLock validation regressions.

#[test]
fn validate_appeal_finance_cancel_asset_lock_bytes_rejects_trailing_bytes() {
    let mut bytes =
        fixture_bytes("fixtures/sorafs_manifest/appeal_finance/cancel_asset_lock_v1.to");
    bytes.push(0);
    let outcome = validate_appeal_finance_cancel_asset_lock_bytes(&bytes, "trailing.to", 42);

    assert_failure(&outcome, "SFS-NORITO-001", CATEGORY_NORITO);
}

#[test]
fn validate_appeal_finance_cancel_asset_lock_bytes_rejects_zero_quantity() {
    let bytes = fixture_bytes(
        "fixtures/sorafs_manifest/appeal_finance/negative/cancel_asset_lock_zero_expected_v1.to",
    );
    let outcome = validate_appeal_finance_cancel_asset_lock_bytes(&bytes, "zero-quantity.to", 43);

    assert_failure(&outcome, "SFS-VAL-001", CATEGORY_VALIDATION);
}

#[test]
fn validate_signed_replication_order_bytes_accepts_signed_order() {
    let envelope = signed_replication_order();
    let outcome = signed_replication_order_outcome(&envelope, "signed-order.to", 7);
    assert_success(&outcome);
    assert_context(&outcome, "signature_algorithm", "ed25519");
}

#[test]
fn validate_signed_replication_order_bytes_rejects_bad_signature() {
    let mut envelope = signed_replication_order();
    envelope.order.deadline_at += 1;
    let outcome = signed_replication_order_outcome(&envelope, "bad-signed-order.to", 8);
    assert_failure(&outcome, "SFS-SIG-006", CATEGORY_SIGNATURE);
}

#[test]
fn validate_replication_order_bytes_rejects_malformed_norito() {
    let outcome = validate_replication_order_bytes(b"not norito", "bad.to", 2);
    assert_failure(&outcome, "SFS-NORITO-001", CATEGORY_NORITO);
}

#[test]
fn validate_replication_order_bytes_rejects_manifest_digest_failure() {
    let mut order = replication_order();
    order.manifest_digest = [0; 32];
    let outcome = replication_order_outcome(&order, "bad-digest.to", 3);
    assert_failure(&outcome, "SFS-VAL-001", CATEGORY_VALIDATION);
}

#[test]
fn validate_replication_order_bytes_rejects_chunker_failure() {
    let mut order = replication_order();
    order.chunking_profile = "sorafs-sf1".to_owned();
    let outcome = replication_order_outcome(&order, "bad-chunker.to", 4);
    assert_failure(&outcome, "SFS-VAL-003", CATEGORY_VALIDATION);
}

#[test]
fn validate_replication_order_bytes_rejects_policy_failure() {
    let mut order = replication_order();
    order.deadline_at = order.issued_at;
    let outcome = replication_order_outcome(&order, "bad-deadline.to", 5);
    assert_failure(&outcome, "SFS-POL-003", CATEGORY_POLICY);
}

#[test]
fn validate_replication_order_bytes_rejects_structural_failure() {
    let mut order = replication_order();
    order.assignments.clear();
    let outcome = replication_order_outcome(&order, "bad-assignments.to", 6);
    assert_failure(&outcome, "SFS-VAL-005", CATEGORY_VALIDATION);
}
