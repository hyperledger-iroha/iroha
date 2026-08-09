    #[test]
    fn rollback_failure_classifier_accepts_rejection_or_inconclusive_confirmation() {
        assert!(is_expected_rollback_failure_text(
            "settlement leg requires 10000 units"
        ));
        assert!(is_expected_rollback_failure_text(
            "haven't got tx confirmation within 600s (configured with `transaction.status_timeout_ms`)"
        ));
        assert!(is_expected_rollback_failure_text(
            "timed out waiting for committed transaction outcome"
        ));
        assert!(!is_expected_rollback_failure_text(
            "transaction applied successfully"
        ));
    }

    #[test]
    fn render_rejection_reason_includes_debug_details_when_display_is_generic() {
        let reason = TransactionRejectionReason::LimitCheck(TransactionLimitError {
            reason: "cross-dataspace route limit exceeded".to_owned(),
        });

        let rendered = render_rejection_reason(&reason);

        assert!(rendered.contains("Failed to validate transaction limits"));
        assert!(rendered.contains("details:"));
        assert!(rendered.contains("cross-dataspace route limit exceeded"));
    }
