#[test]
fn roadmap_invalid_evidence_roundtrip_cases() {
    let ctx = test_context();
    let context = ctx.validation_context();
    let cases: &[EvidenceRoundtripCase] = &[
        (
            "duplicate signer",
            EvidenceValidationError::SignerMismatch,
            roundtrip_case_duplicate_signer,
        ),
        (
            "conflicting height",
            EvidenceValidationError::HeightMismatch,
            roundtrip_case_conflicting_height,
        ),
        (
            "conflicting view",
            EvidenceValidationError::ViewMismatch,
            roundtrip_case_conflicting_view,
        ),
        (
            "forged signature length",
            EvidenceValidationError::SignatureTruncated,
            roundtrip_case_signature_truncated,
        ),
        (
            "mixed manifest payload",
            EvidenceValidationError::KindPayloadMismatch,
            roundtrip_case_mixed_manifest_payload,
        ),
    ];
    for (label, expected, build) in cases {
        let evidence = build(&ctx);
        assert_invalid_evidence_rejected(&context, &evidence, *expected);
        assert!(
            validate_evidence(&evidence, &context).is_err(),
            "{label}: expected structural validation to fail"
        );
    }
}
