// Exact appeal-verdict CLI parser regressions.
//
// Included by the binary's unit-test module so private parser behavior remains covered.
#[test]
fn appeal_cli_verdict_parser_requires_exact_v1_spelling() {
    assert_eq!(
        parse_appeal_verdict("withdrawn_after_panel").unwrap(),
        AppealVerdict::WithdrawnAfterPanel
    );
    for alias in [
        "withdrawn-after-panel",
        "withdrawn_post",
        "pending",
        "ESCALATED",
        " escalated",
    ] {
        assert!(
            parse_appeal_verdict(alias).is_err(),
            "noncanonical CLI verdict {alias:?} must be rejected"
        );
    }
}
