// Missing-signature mutation coverage for persisted equivocation evidence.

#[test]
fn persist_record_rejects_missing_signature_mutation() {
    let ctx = test_context();
    let context = ctx.validation_context();
    let evidence = double_vote_with_unchecked(&ctx, |v1, v2| {
        v1.bls_sig.clear();
        v2.bls_sig.clear();
    });
    assert_invalid_evidence_rejected(
        &context,
        &evidence,
        EvidenceValidationError::SignatureMissing,
    );
}
