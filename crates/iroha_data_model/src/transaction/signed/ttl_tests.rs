//! Transaction lifetime and ingress-metadata tests.
use super::*;
use crate::domain::DomainId;
fn checked_random_keypair() -> iroha_crypto::KeyPair {
    iroha_crypto::KeyPair::try_random().expect("test fixture random key generation should succeed")
}
#[test]
fn zero_ttl_is_rejected_before_signing() {
    let network_id = test_network_id(0x2A);
    let private_key: iroha_crypto::PrivateKey =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
            .parse()
            .unwrap();
    let authority = AccountId::new(iroha_crypto::PublicKey::from(private_key.clone()));
    let mut builder = TransactionBuilder::new(
        network_id,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_ttl(Duration::from_millis(0));
    assert!(matches!(
        builder.try_sign(&private_key),
        Err(TransactionSignatureError::MissingTimeToLive)
    ));
}
#[test]
fn builder_assigns_a_signature_bound_default_ttl() {
    let key_pair = checked_random_keypair();
    let authority = AccountId::new(key_pair.public_key().clone());
    let tx = TransactionBuilder::new(
        test_network_id(0x33),
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(key_pair.private_key());
    assert_eq!(tx.time_to_live(), Some(DEFAULT_TRANSACTION_TIME_TO_LIVE));
}
#[test]
fn signing_workflows_reject_payloads_without_ttl() {
    let key_pair = checked_random_keypair();
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut payload = TransactionBuilder::new(
        test_network_id(0x34),
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .into_payload()
    .expect("default payload");
    payload.time_to_live_ms = None;
    assert!(matches!(
        TransactionBuilder::from_payload(payload.clone()),
        Err(TransactionSignatureError::MissingTimeToLive)
    ));
    let bytes = norito::codec::encode_adaptive(&payload);
    let error = TransactionBuilder::decode_payload(&bytes)
        .expect_err("external signing decode must reject a payload without TTL");
    assert!(
        error.to_string().contains("time_to_live_ms is required"),
        "unexpected decode error: {error}"
    );
}
#[test]
fn ingress_metadata_accessors_read_numeric_values() {
    let network_id = test_network_id(0x2B);
    let keypair = checked_random_keypair();
    let _domain: crate::domain::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let account_id = AccountId::new(keypair.public_key().clone());
    let mut metadata = Metadata::default();
    metadata.insert(
        crate::name::Name::from_str("expires_at_height").unwrap(),
        iroha_primitives::json::Json::from(10_u64),
    );
    metadata.insert(
        crate::name::Name::from_str("tx_sequence").unwrap(),
        iroha_primitives::json::Json::from(3_u64),
    );
    let tx = TransactionBuilder::new(
        network_id,
        account_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_metadata(metadata)
    .sign(keypair.private_key());
    assert_eq!(tx.expires_at_height().expect("parse metadata"), Some(10));
    assert_eq!(tx.tx_sequence().expect("parse metadata"), Some(3));
}
#[test]
fn ingress_metadata_accessors_propagate_decode_error() {
    let network_id = test_network_id(0x2C);
    let keypair = checked_random_keypair();
    let _domain: crate::domain::DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let account_id = AccountId::new(keypair.public_key().clone());
    let mut metadata = Metadata::default();
    metadata.insert(
        crate::name::Name::from_str("expires_at_height").unwrap(),
        iroha_primitives::json::Json::new("not-a-number"),
    );
    let tx = TransactionBuilder::new(
        network_id,
        account_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_metadata(metadata)
    .sign(keypair.private_key());
    assert!(tx.expires_at_height().is_err());
}
