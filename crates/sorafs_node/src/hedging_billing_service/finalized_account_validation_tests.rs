// Finalized billing-account validation regressions.
#[test]
fn finalized_events_require_exact_canonical_utf8_i105_account_bytes() {
    let root = tempfile::tempdir().expect("state root");
    let (service, _feed_policy, _reference, _verifier, _publisher, _ack_authority) =
        ready_service(root.path());
    let mut legacy = event(1, "storage:event:legacy-account", "1");
    legacy.account_id = b"account-1".to_vec();
    assert!(matches!(
        service.ingest_finalized_page(&page(vec![legacy])),
        Err(HedgingBillingServiceError::InvalidFinalizedEvent)
    ));
    let mut invalid_utf8 = event(1, "storage:event:invalid-utf8", "1");
    invalid_utf8.account_id = vec![0xFF, 0xFE];
    assert!(matches!(
        service.ingest_finalized_page(&page(vec![invalid_utf8])),
        Err(HedgingBillingServiceError::InvalidFinalizedEvent)
    ));
    let canonical = event(1, "storage:event:canonical-account", "1");
    service
        .ingest_finalized_page(&page(vec![canonical]))
        .expect("exact canonical I105 account bytes");
}
