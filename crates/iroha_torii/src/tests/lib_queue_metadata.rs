#[cfg(test)]
mod tests_queue_metadata {
    use super::*;

    #[test]
    fn queue_errors_map_to_reason_codes() {
        let cases = [
            (
                queue::Error::Full,
                "PRTRY:QUEUE_FULL",
                "transaction queue is at capacity",
            ),
            (
                queue::Error::MaximumTransactionsPerUser,
                "PRTRY:QUEUE_RATE",
                "authority reached per-user queue capacity",
            ),
            (
                queue::Error::Expired,
                "ED07",
                "transaction expired before admission",
            ),
            (
                queue::Error::UnresolvedRoute {
                    reason: "lane 9 is unknown".to_owned(),
                },
                "PRTRY:ROUTE_UNRESOLVED",
                "transaction route could not be resolved: lane 9 is unknown",
            ),
            (
                queue::Error::InBlockchain,
                "PRTRY:ALREADY_COMMITTED",
                "transaction already committed to the blockchain",
            ),
            (
                queue::Error::IsInQueue,
                "PRTRY:ALREADY_ENQUEUED",
                "transaction already present in the queue",
            ),
        ];

        for (error, expected_code, expected_detail) in cases {
            // array copy, pattern moves
            let (code, detail) = queue_rejection_metadata(&error);
            assert_eq!(code, expected_code);
            assert_eq!(detail, expected_detail);
        }
    }

    #[test]
    fn queue_plan_journal_outcome_unknown_has_stable_code_and_exact_hash() {
        let transaction_hash =
            HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::new(b"outcome-unknown"));
        let error = queue::Error::PlanJournalDurabilityIndeterminate {
            transaction_hash,
            reason: "cleanup sync failed".to_owned(),
        };

        assert_eq!(
            super::Error::queue_error_summary(&error),
            (
                "queue_plan_journal_outcome_unknown",
                "transaction admission outcome is unknown; reconcile by exact transaction hash before retrying",
            )
        );
        let (code, detail) = queue_rejection_metadata(&error);
        assert_eq!(code, "PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN");
        assert!(detail.contains(&transaction_hash.to_string()));
        assert!(detail.contains("reconcile that exact signed hash before retrying"));
        assert_eq!(
            super::Error::status_code_for_queue_error(&error),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let envelope =
            super::Error::queue_error_envelope(&error, queue::BackpressureState::default());
        let details = envelope.details.expect("outcome-unknown details");
        let expected_hash = transaction_hash.to_string();
        assert_eq!(details.tx_hash.as_deref(), Some(expected_hash.as_str()));
        assert!(
            details
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("byte-identical signed bytes"))
        );
    }
}
