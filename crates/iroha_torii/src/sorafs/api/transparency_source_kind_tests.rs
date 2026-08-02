// Exact transparency source-kind parser regressions.
//
// Included by `advert_tests` to keep these assertions adjacent to the route tests.

#[test]
fn transparency_source_kind_parser_requires_exact_v1_spelling() {
    for canonical in [
        TRANSPARENCY_SOURCE_KIND_GAR_ENFORCEMENT_RECEIPT,
        TRANSPARENCY_SOURCE_KIND_MODERATION_BALLOT_GOVERNANCE_EVENT,
        TRANSPARENCY_SOURCE_KIND_APPEAL_FINANCE_REPORT,
        TRANSPARENCY_SOURCE_KIND_APPEAL_FINANCE_SETTLEMENT_RECEIPT,
        TRANSPARENCY_SOURCE_KIND_LEGAL_HOLD_NOTICE,
        TRANSPARENCY_SOURCE_KIND_REDACTION_NOTICE,
        TRANSPARENCY_SOURCE_KIND_EVIDENCE_ACCESS_SUMMARY,
    ] {
        assert_eq!(
            parse_transparency_source_kind(canonical).unwrap(),
            canonical
        );
    }
    for alias in [
        "gar",
        "gar-receipt",
        "moderation-ballot",
        "appeal-finance-settlement",
        "legal-hold",
        "redaction",
        "evidence-access",
        "evidence-viewer-access",
        "GAR-ENFORCEMENT-RECEIPT",
        "gar_enforcement_receipt",
        " gar-enforcement-receipt",
        "gar-enforcement-receipt ",
    ] {
        let response = parse_transparency_source_kind(alias)
            .expect_err("noncanonical transparency source kind must fail");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
