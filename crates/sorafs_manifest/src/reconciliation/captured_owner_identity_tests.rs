// Actual compiler observations for existing owners in sorafs_manifest::reconciliation.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::SorafsReconciliationReportV1>(
        "sorafs_manifest::reconciliation::SorafsReconciliationReportV1",
        "sorafs_manifest::reconciliation::SorafsReconciliationReportV1",
        "b23cf78347b28e0286e91ee0f916187b",
        "b23cf78347b28e0286e91ee0f916187b",
    );
    crate::captured_owner_identity_support::check_both::<self::AppealFinanceReconciliationSummaryV1>(
        "sorafs_manifest::reconciliation::AppealFinanceReconciliationSummaryV1",
        "sorafs_manifest::reconciliation::AppealFinanceReconciliationSummaryV1",
        "54721071dd4fa59da648cde4bc9abc82",
        "54721071dd4fa59da648cde4bc9abc82",
    );
}
