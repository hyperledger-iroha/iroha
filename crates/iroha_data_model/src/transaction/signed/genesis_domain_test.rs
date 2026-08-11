#[test]
fn genesis_transaction_builder_cannot_be_reconstructed_as_an_ordinary_draft() {
    let key_pair = checked_random_keypair_with_algorithm(Algorithm::Ed25519);
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut genesis_builder = TransactionBuilder::new_genesis(
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "genesis-only".into())]);
    genesis_builder.set_creation_time(Duration::from_millis(42));

    let payload_bytes = genesis_builder.encode_payload();
    let payload = genesis_builder
        .clone()
        .into_payload()
        .expect("the explicit genesis constructor retains its separate domain");
    assert_eq!(payload.domain, TransactionDomain::Genesis);
    assert!(matches!(
        TransactionBuilder::from_payload(payload.clone()),
        Err(TransactionSignatureError::GenesisDomainNotAllowed)
    ));
    assert_eq!(
        TransactionBuilder::from_genesis_payload(payload.clone())
            .expect("explicit genesis reconstruction accepts the genesis-only domain")
            .encode_payload(),
        payload_bytes
    );
    assert!(
        TransactionBuilder::decode_payload(&payload_bytes)
            .expect_err("ordinary payload decoding must reject the genesis-only domain")
            .to_string()
            .contains("explicit genesis construction")
    );

    let signed = TransactionBuilder::decode_genesis_payload(&payload_bytes)
        .expect("explicit genesis decoding accepts the canonical genesis payload")
        .try_sign(key_pair.private_key())
        .expect("the explicit genesis construction path remains signable");
    signed
        .verify_signature()
        .expect("the explicit genesis transaction signature verifies");
    let signed_wire = signed.encode_versioned();
    let decoded = SignedTransaction::decode_all_versioned(&signed_wire)
        .expect("genesis transactions remain structurally decodable on the fixed V1 wire");
    assert_eq!(decoded, signed);
    assert_eq!(decoded.encode_versioned(), signed_wire);

    let mut hostile = TransactionBuilder::new(
        test_network_id(0x1F),
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    );
    assert!(matches!(
        TransactionBuilder::from_genesis_payload(hostile.payload.clone()),
        Err(TransactionSignatureError::GenesisDomainRequired)
    ));
    assert!(
        TransactionBuilder::decode_genesis_payload(&hostile.encode_payload())
            .expect_err("genesis decoding must reject an ordinary NetworkId domain")
            .to_string()
            .contains("requires the genesis transaction domain")
    );
    hostile.payload.domain = TransactionDomain::Genesis;
    assert!(matches!(
        hostile.clone().into_payload(),
        Err(TransactionSignatureError::GenesisDomainNotAllowed)
    ));
    assert!(matches!(
        hostile.clone().try_sign(key_pair.private_key()),
        Err(TransactionSignatureError::GenesisDomainNotAllowed)
    ));
    assert!(matches!(
        hostile.try_sign_multisig([key_pair.private_key()]),
        Err(TransactionSignatureError::GenesisDomainNotAllowed)
    ));
}
