//! Native transaction proxy identity boundaries.
use super::*;
fn submission() -> (TransactionEntrypoint, ToriiProxyHttpResponseV1) {
    let signer = KeyPair::from_seed(vec![0x41; 32], iroha_crypto::Algorithm::Ed25519);
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"native-torii-admission",
    )));
    let transaction = TransactionEntrypoint::External(
        iroha_data_model::transaction::TransactionBuilder::new(
            network,
            AccountId::new(signer.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(signer.private_key()),
    );
    let response = ToriiProxyHttpResponseV1 {
        status_code: 202,
        headers: vec![
            iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                name: "x-iroha-entrypoint-hash".to_owned(),
                value: transaction.hash().to_string().into_bytes(),
            },
            iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                name: "x-iroha-signed-transaction-hash".to_owned(),
                value: signed_transaction_hash_for_entrypoint(&transaction)
                    .unwrap()
                    .to_string()
                    .into_bytes(),
            },
        ],
        body: Vec::new(),
    };
    (transaction, response)
}
#[test]
fn accepted_proxy_response_binds_both_original_transaction_hashes() {
    let (transaction, mut response) = submission();
    validate_native_transaction_submission_identity(&response, &transaction).unwrap();
    response.headers[1].value = Hash::new(b"another signed transaction")
        .to_string()
        .into_bytes();
    assert!(validate_native_transaction_submission_identity(&response, &transaction).is_err());
}
#[test]
fn accepted_proxy_response_rejects_missing_and_duplicate_identity() {
    let (transaction, mut response) = submission();
    response.headers.pop();
    assert!(validate_native_transaction_submission_identity(&response, &transaction).is_err());
    let (transaction, mut response) = submission();
    response.headers.push(response.headers[0].clone());
    assert!(validate_native_transaction_submission_identity(&response, &transaction).is_err());
}
#[test]
fn queue_admission_invariant_is_unavailable_without_a_durability_claim() {
    let error = queue::Error::AdmissionInvariant {
        reason: "index drift".to_owned(),
    };
    assert_eq!(
        Error::status_code_for_queue_error(&error),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        Error::queue_error_summary(&error).0,
        "queue_admission_invariant"
    );
    assert_eq!(
        queue_rejection_metadata(&error).0,
        "PRTRY:QUEUE_ADMISSION_INVARIANT"
    );
}
