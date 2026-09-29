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
                "KAGEMUSHA V1 operation carrier failed canonical admission: non-canonical carrier",
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
                "KAGEMUSHA V1 pending-operation index requires recovery: reverse owner missing",
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
    fn unsupported_current_queue_admission_has_permanent_canonical_error() {
        let error = queue::Error::UnsupportedTransactionAdmission {
            reason: "current consensus does not support multi-route transaction admission"
                .to_owned(),
        };
        assert_eq!(
            Error::status_code_for_queue_error(&error),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            Error::queue_error_summary(&error).0,
            "unsupported_transaction_admission"
        );
        assert_eq!(
            queue_rejection_metadata(&error).0,
            "PRTRY:UNSUPPORTED_TRANSACTION_ADMISSION"
        );
        let envelope = Error::queue_error_envelope(&error, None);
        assert_eq!(envelope.code, "unsupported_transaction_admission");
        assert!(envelope.details.unwrap().retry_after_seconds.is_none());
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
                "KAGEMUSHA V1 operation identifier is already pending",
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
}
