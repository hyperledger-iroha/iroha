#[test]
fn bridge_public_transaction_signing_binds_queue_plan_and_direct_builder_stays_ordinary() {
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
        || Executable::from(Vec::<InstructionBox>::new()),
    )
    .expect("checked bridge transaction signing should succeed");
    let signed =
        decode_signed_transaction(&signed_bytes).expect("decode versioned signed transaction");
    assert_eq!(hash_bytes, *signed.hash().as_ref());
    assert_eq!(signed.authority(), &authority);
    assert_eq!(
        signed.admission_intent(),
        TransactionAdmissionIntent::QueuePlanSynced
    );
    signed
        .verify_signature()
        .expect("checked bridge transaction signature should verify");

    let (fee_signed_bytes, _) = encode_asset_transaction_with_nonce_fee_payment_and_metadata(
        network_id,
        authority.clone(),
        1_736_000_000_001,
        None,
        None,
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
        keypair.private_key().clone(),
        || Executable::from(Vec::<InstructionBox>::new()),
    )
    .expect("fee-aware public bridge transaction should sign");
    let fee_signed = decode_signed_transaction(&fee_signed_bytes)
        .expect("decode fee-aware versioned signed transaction");
    assert_eq!(
        fee_signed.admission_intent(),
        TransactionAdmissionIntent::QueuePlanSynced
    );

    let direct = TransactionBuilder::new(
        network_id,
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::from(Vec::<InstructionBox>::new()))
    .try_sign(keypair.private_key())
    .expect("direct bridge fixture transaction should sign");
    assert_eq!(
        direct.admission_intent(),
        TransactionAdmissionIntent::Ordinary
    );
}
