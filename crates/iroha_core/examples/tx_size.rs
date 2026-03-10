//! Compute the encoded size of a single Log instruction transaction with a custom payload.

use std::str::FromStr;

use iroha_crypto::KeyPair;
use iroha_data_model::{Level, domain::DomainId, prelude::*};
use iroha_version::codec::EncodeVersioned;

fn main() {
    let bytes: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let payload = "x".repeat(bytes);

    let chain = ChainId::from("00000000-0000-0000-0000-000000000000");
    let domain = DomainId::from_str("wonderland").expect("static domain id");
    let key_pair = KeyPair::random();
    let authority = AccountId::new(key_pair.public_key().clone());

    let tx = TransactionBuilder::new(chain, authority)
        .with_instructions([Log::new(Level::INFO, payload)])
        .sign(key_pair.private_key());
    let encoded = tx.encode_versioned();
    println!("payload_bytes={} encoded_len={}", bytes, encoded.len());
}
