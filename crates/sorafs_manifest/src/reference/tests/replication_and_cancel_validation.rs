// Replication-order and CancelAssetLock validation regressions.
#[test]
fn validate_appeal_finance_cancel_asset_lock_bytes_rejects_trailing_bytes() {
    let mut bytes = fs::read(workspace_fixture(
        "fixtures/sorafs_manifest/appeal_finance/cancel_asset_lock_v1.to",
    ))
    .expect("read canonical CancelAssetLock fixture");
    bytes.push(0);
    let outcome = validate_appeal_finance_cancel_asset_lock_bytes(&bytes, "trailing.to", 42);
    assert!(!outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-NORITO-001");
    assert_eq!(outcome.category, CATEGORY_NORITO);
}
#[test]
fn validate_appeal_finance_cancel_asset_lock_bytes_rejects_zero_quantity() {
    let bytes = fs::read(workspace_fixture(
        "fixtures/sorafs_manifest/appeal_finance/negative/cancel_asset_lock_zero_expected_v1.to",
    ))
    .expect("read zero-quantity CancelAssetLock fixture");
    let outcome = validate_appeal_finance_cancel_asset_lock_bytes(&bytes, "zero-quantity.to", 43);
    assert!(!outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-VAL-001");
    assert_eq!(outcome.category, CATEGORY_VALIDATION);
}
#[test]
fn validate_signed_replication_order_bytes_accepts_signed_order() {
    let envelope = signed_replication_order();
    let bytes = to_bytes(&envelope).expect("encode signed order");
    let outcome = validate_signed_replication_order_bytes(&bytes, "signed-order.to", 7);
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-OK-000");
    assert!(
        outcome
            .context
            .iter()
            .any(|field| field.key == "signature_algorithm" && field.value == "ed25519"),
        "{outcome:?}"
    );
}
#[test]
fn validate_signed_replication_order_bytes_rejects_bad_signature() {
    let mut envelope = signed_replication_order();
    envelope.order.deadline_at += 1;
    let bytes = to_bytes(&envelope).expect("encode tampered signed order");
    let outcome = validate_signed_replication_order_bytes(&bytes, "bad-signed-order.to", 8);
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-SIG-006", "{outcome:?}");
    assert_eq!(outcome.category, CATEGORY_SIGNATURE);
}
#[test]
fn validate_replication_order_bytes_rejects_malformed_norito() {
    let outcome = validate_replication_order_bytes(b"not norito", "bad.to", 2);
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-NORITO-001");
    assert_eq!(outcome.category, CATEGORY_NORITO);
}
#[test]
fn validate_replication_order_bytes_rejects_manifest_digest_failure() {
    let mut order = replication_order();
    order.manifest_digest = [0; 32];
    let bytes = to_bytes(&order).expect("encode order");
    let outcome = validate_replication_order_bytes(&bytes, "bad-digest.to", 3);
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-VAL-001");
    assert_eq!(outcome.category, CATEGORY_VALIDATION);
}
#[test]
fn validate_replication_order_bytes_rejects_chunker_failure() {
    let mut order = replication_order();
    order.chunking_profile = "sorafs-sf1".to_owned();
    let bytes = to_bytes(&order).expect("encode order");
    let outcome = validate_replication_order_bytes(&bytes, "bad-chunker.to", 4);
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-VAL-003");
    assert_eq!(outcome.category, CATEGORY_VALIDATION);
}
#[test]
fn validate_replication_order_bytes_rejects_policy_failure() {
    let mut order = replication_order();
    order.deadline_at = order.issued_at;
    let bytes = to_bytes(&order).expect("encode order");
    let outcome = validate_replication_order_bytes(&bytes, "bad-deadline.to", 5);
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-POL-003");
    assert_eq!(outcome.category, CATEGORY_POLICY);
}
#[test]
fn validate_replication_order_bytes_rejects_structural_failure() {
    let mut order = replication_order();
    order.assignments.clear();
    let bytes = to_bytes(&order).expect("encode order");
    let outcome = validate_replication_order_bytes(&bytes, "bad-assignments.to", 6);
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-VAL-005");
    assert_eq!(outcome.category, CATEGORY_VALIDATION);
}
