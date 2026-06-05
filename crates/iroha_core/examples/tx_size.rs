//! Compute the encoded size of a single Log instruction transaction with a custom payload.

use iroha_crypto::KeyPair;
use iroha_data_model::{Level, prelude::*};
use iroha_version::codec::EncodeVersioned;

fn main() -> Result<(), iroha_crypto::Error> {
    let bytes: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let payload = "x".repeat(bytes);

    let chain = ChainId::from("00000000-0000-0000-0000-000000000000");
    let key_pair = tx_size_key_pair()?;
    let authority = AccountId::new(key_pair.public_key().clone());

    let tx = TransactionBuilder::new(chain, authority)
        .with_instructions([Log::new(Level::INFO, payload)])
        .sign(key_pair.private_key());
    let encoded = tx.encode_versioned();
    println!("payload_bytes={} encoded_len={}", bytes, encoded.len());
    Ok(())
}

fn tx_size_key_pair() -> Result<KeyPair, iroha_crypto::Error> {
    KeyPair::try_random()
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Algorithm;

    use super::*;

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
}
