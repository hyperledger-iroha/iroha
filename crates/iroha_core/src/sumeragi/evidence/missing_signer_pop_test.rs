#[test]
fn v2_aggregate_verification_rejects_missing_signer_pop_without_panicking() {
    let fixture = V2EvidenceFixture::new();
    let error =
        verify_v2_aggregate_signature(&fixture.context, &fixture.proofs[..1], &[2], &[], &[])
            .expect_err("a signer without a matching proof of possession must fail closed");
    assert_eq!(error, EvidenceValidationError::V2SignerMismatch);
}
