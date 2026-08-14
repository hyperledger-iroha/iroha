//! Compute the encoded size of a single Log instruction transaction with a custom payload.
use iroha_crypto::KeyPair;
use iroha_data_model::{
    Level,
    prelude::*,
    transaction::{SignedTransaction, signed::TransactionSignatureError},
};
use iroha_version::codec::EncodeVersioned;
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let bytes: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let payload = "x".repeat(bytes);
    let key_pair = tx_size_key_pair()?;
    let tx = build_tx_size_transaction(payload, &key_pair)?;
    let encoded = tx.encode_versioned();
    println!("payload_bytes={} encoded_len={}", bytes, encoded.len());
    Ok(())
}
fn build_tx_size_transaction(
    payload: String,
    key_pair: &KeyPair,
) -> Result<SignedTransaction, TransactionSignatureError> {
    let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"tx-size-example-genesis"),
    ));
    let authority = AccountId::new(key_pair.public_key().clone());
    TransactionBuilder::new(
        network_id,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, payload)])
    .try_sign(key_pair.private_key())
}
fn tx_size_key_pair() -> Result<KeyPair, iroha_crypto::Error> {
    KeyPair::try_random()
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Algorithm;
    #[test]
    fn tx_size_key_pair_uses_checked_default_generation() {
        let key_pair = tx_size_key_pair().expect("tx-size example key pair");
        assert_eq!(
            key_pair
                .public_key()
                .try_algorithm()
                .expect("tx-size example public-key algorithm"),
            Algorithm::default()
        );
    }
    #[test]
    fn tx_size_transaction_uses_checked_signing_and_verifies() {
        let key_pair = tx_size_key_pair().expect("tx-size example key pair");
        let tx = build_tx_size_transaction("x".repeat(8), &key_pair)
            .expect("tx-size example transaction should sign");
        tx.verify_signature()
            .expect("checked tx-size example signature should verify");
    }
}
