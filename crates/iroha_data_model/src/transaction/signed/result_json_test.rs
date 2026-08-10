#[cfg(feature = "json")]
#[test]
fn transaction_result_json_roundtrip() {
    let ok_result = TransactionResult::new(Ok(DataTriggerSequence::default()));
    let json = norito::json::to_json(&ok_result).expect("serialize ok result");
    let decoded: TransactionResult = norito::json::from_str(&json).expect("deserialize ok result");
    assert_eq!(ok_result, decoded);

    let err_reason = error::TransactionRejectionReason::LimitCheck(error::TransactionLimitError {
        reason: "limit exceeded".into(),
    });
    let err_result = TransactionResult::new(Err(err_reason));
    let json = norito::json::to_json(&err_result).expect("serialize err result");
    let decoded: TransactionResult = norito::json::from_str(&json).expect("deserialize err result");
    assert_eq!(err_result, decoded);
}

#[cfg(feature = "json")]
#[test]
fn transaction_entrypoint_json_roundtrip() {
    let network_id = test_network_id(0x29);
    let _domain: DomainId = DomainId::try_new("default", "universal").unwrap();
    let public_key: iroha_crypto::PublicKey =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .unwrap();
    let private_key: iroha_crypto::PrivateKey =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
            .parse()
            .unwrap();
    let authority = AccountId::new(public_key);

    let tx = TransactionBuilder::new(
        network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::Instructions(Vec::new().into()))
    .sign(&private_key);
    let entry = TransactionEntrypoint::External(tx);
    let json = norito::json::to_json(&entry).expect("serialize external entrypoint");
    let decoded: TransactionEntrypoint =
        norito::json::from_str(&json).expect("deserialize external entrypoint");
    assert_eq!(entry, decoded);

    let time_entry = TimeTriggerEntrypoint {
        id: "trigger".parse().unwrap(),
        instructions: ExecutionStep(Vec::new().into()),
        authority,
    };
    let entry = TransactionEntrypoint::Time(time_entry);
    let json = norito::json::to_json(&entry).expect("serialize time entrypoint");
    let decoded: TransactionEntrypoint =
        norito::json::from_str(&json).expect("deserialize time entrypoint");
    assert_eq!(entry, decoded);
}
