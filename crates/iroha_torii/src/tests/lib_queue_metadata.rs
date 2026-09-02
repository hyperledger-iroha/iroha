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
                queue::Error::KagemushaV1OperationCarrierRejected {
                    reason: "non-canonical carrier".to_owned(),
                },
                "PRTRY:KAGEMUSHA_V1_OPERATION_CARRIER_REJECTED",
                "Kagemusha V1 operation carrier failed canonical admission: non-canonical carrier",
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
            (
                queue::Error::KagemushaV1OperationIndexInconsistent {
                    reason: "reverse owner missing".to_owned(),
                },
                "PRTRY:KAGEMUSHA_V1_OPERATION_INDEX_INCONSISTENT",
                "Kagemusha V1 pending-operation index requires recovery: reverse owner missing",
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
    fn kagemusha_v1_queue_conflict_has_stable_code_and_status() {
        let existing_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::new(b"existing-kagemusha-v1-entrypoint"),
        );
        let operation_id = [0xA5; 32];
        let error = queue::Error::KagemushaV1OperationIdConflict {
            operation_id,
            existing_entrypoint_hash,
        };
        let (code, detail) = queue_rejection_metadata(&error);
        assert_eq!(code, "PRTRY:KAGEMUSHA_V1_OPERATION_ID_CONFLICT");
        assert!(detail.contains(&hex::encode(operation_id)));
        assert!(detail.contains(&existing_entrypoint_hash.to_string()));
        assert_eq!(
            super::Error::queue_error_summary(&error),
            (
                "kagemusha_v1_operation_id_conflict",
                "Kagemusha V1 operation identifier is already pending",
            )
        );
        assert_eq!(
            super::Error::status_code_for_queue_error(&error),
            StatusCode::CONFLICT
        );

        let inconsistent = queue::Error::KagemushaV1OperationIndexInconsistent {
            reason: "reverse owner missing".to_owned(),
        };
        assert_eq!(
            super::Error::status_code_for_queue_error(&inconsistent),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    #[test]
    fn queue_plan_journal_outcome_unknown_has_stable_code_and_exact_hash() {
        let entrypoint_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(b"outcome-unknown"));
        let signed_transaction_hash =
            HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::new(b"signed-transaction"));
        let error = queue::Error::PlanJournalDurabilityIndeterminate {
            entrypoint_hash,
            signed_transaction_hash: Some(signed_transaction_hash),
            reason: "cleanup sync failed".to_owned(),
        };
        assert_eq!(
            super::Error::queue_error_summary(&error),
            (
                "queue_plan_journal_outcome_unknown",
                "transaction admission outcome is unknown; reconcile by exact entrypoint hash before retrying",
            )
        );
        let (code, detail) = queue_rejection_metadata(&error);
        assert_eq!(code, "PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN");
        assert!(detail.contains(&entrypoint_hash.to_string()));
        assert!(detail.contains("reconcile that exact entrypoint before retrying"));
        assert_eq!(
            super::Error::status_code_for_queue_error(&error),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let envelope =
            super::Error::queue_error_envelope(&error, Some(queue::BackpressureState::default()));
        let details = envelope.details.expect("outcome-unknown details");
        let expected_entrypoint_hash = entrypoint_hash.to_string();
        let expected_signed_hash = signed_transaction_hash.to_string();
        assert_eq!(
            details.entrypoint_hash.as_deref(),
            Some(expected_entrypoint_hash.as_str())
        );
        assert_eq!(
            details.tx_hash.as_deref(),
            Some(expected_signed_hash.as_str())
        );
        assert!(
            details
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("byte-identical signed bytes"))
        );

        let error_without_signed_hash = queue::Error::PlanJournalDurabilityIndeterminate {
            entrypoint_hash,
            signed_transaction_hash: None,
            reason: "cleanup sync failed".to_owned(),
        };
        let envelope = super::Error::queue_error_envelope(
            &error_without_signed_hash,
            Some(queue::BackpressureState::default()),
        );
        let details = envelope.details.expect("entrypoint-only outcome details");
        assert_eq!(
            details.entrypoint_hash.as_deref(),
            Some(expected_entrypoint_hash.as_str())
        );
        assert!(details.tx_hash.is_none());
        assert!(
            details
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("exact entrypoint hash")
                    && !hint.contains("byte-identical signed bytes"))
        );
    }
}
