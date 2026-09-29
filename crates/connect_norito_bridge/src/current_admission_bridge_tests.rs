#[test]
fn bridge_public_transaction_signing_binds_canonical_nine_field_payload() {
    use iroha_model_base::metadata::Metadata;
    use std::num::NonZeroU64;

    let keypair = fixture_key_pair(0x5A);
    let authority = AccountId::new(keypair.public_key().clone());
    let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(Hash::new(
        b"bridge-checked-signing-genesis",
    )));
    let (signed_bytes, hash_bytes) = encode_asset_transaction(
        network_id,
        authority.clone(),
        1_736_000_000_000,
        None,
        FeePaymentIntent::authority(Vec::new(), None),
        keypair.private_key().clone(),
        || {
            Executable::from(vec![InstructionBox::from(iroha_data_model::isi::Log::new(
                iroha_data_model::Level::INFO,
                "current bridge payload".to_owned(),
            ))])
        },
    )
    .expect("checked bridge transaction signing should succeed");
    let signed =
        decode_signed_transaction(&signed_bytes).expect("decode versioned signed transaction");
    assert_eq!(hash_bytes, *signed.hash().as_ref());
    assert_eq!(signed.authority(), &authority);
    assert_eq!(signed.encode_wire_v1().unwrap(), signed_bytes);
    signed.verify_signature().expect("checked bridge signature");

    let fee_payment = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(71));
    let (fee_signed_bytes, fee_hash) =
        encode_asset_transaction_with_nonce_fee_payment_and_metadata(
            network_id,
            authority.clone(),
            1_736_000_000_001,
            None,
            None,
            fee_payment.clone(),
            Metadata::default(),
            keypair.private_key().clone(),
            || {
                Executable::from(vec![InstructionBox::from(iroha_data_model::isi::Log::new(
                    iroha_data_model::Level::INFO,
                    "current bridge payload".to_owned(),
                ))])
            },
        )
        .expect("fee-aware public bridge transaction should sign");
    let fee_signed = decode_signed_transaction(&fee_signed_bytes)
        .expect("decode fee-aware versioned signed transaction");
    assert_eq!(fee_signed.authority(), &authority);
    assert_eq!(fee_signed.fee_payment_intent(), &fee_payment);
    assert_eq!(fee_hash, *fee_signed.hash().as_ref());
    assert_eq!(fee_signed.encode_wire_v1().unwrap(), fee_signed_bytes);
    fee_signed.verify_signature().expect("fee-aware signature");

    let direct = TransactionBuilder::new(network_id, authority, fee_payment)
        .with_executable(Executable::from(Vec::<InstructionBox>::new()))
        .try_sign(keypair.private_key())
        .expect("direct bridge fixture transaction should sign");
    direct.verify_signature().expect("direct current signature");
    for transaction in [&signed, &fee_signed, &direct] {
        let payload = norito::json::to_value(transaction.payload()).unwrap();
        let fields = payload
            .as_object()
            .expect("canonical transaction payload object");
        assert_eq!(fields.len(), 9);
        assert!(!fields.contains_key("admission_intent"));
        for retired in ["Ordinary", "QueuePlanSynced"] {
            let mut old_payload = payload.clone();
            old_payload.as_object_mut().unwrap().insert(
                "admission_intent".into(),
                norito::json::Value::String(retired.into()),
            );
            assert!(norito::json::from_value::<
                iroha_data_model::transaction::signed::TransactionPayload,
            >(old_payload).is_err(), "retired signed fields must not be silently downgraded");
        }
    }
}
