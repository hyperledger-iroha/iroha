    #[cfg(feature = "json")]
    #[test]
    fn transaction_result_json_roundtrip() {
        let ok_result = TransactionResult::new(Ok(DataTriggerSequence::default()));
        let json = norito::json::to_json(&ok_result).expect("serialize ok result");
        let decoded: TransactionResult =
            norito::json::from_str(&json).expect("deserialize ok result");
        assert_eq!(ok_result, decoded);

        let err_reason =
            error::TransactionRejectionReason::LimitCheck(error::TransactionLimitError {
                reason: "limit exceeded".into(),
            });
        let err_result = TransactionResult::new(Err(err_reason));
        let json = norito::json::to_json(&err_result).expect("serialize err result");
        let decoded: TransactionResult =
            norito::json::from_str(&json).expect("deserialize err result");
        assert_eq!(err_result, decoded);
    }
