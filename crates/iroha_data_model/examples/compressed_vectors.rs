//! Dump reference I105 account addresses for documentation fixtures.

use std::convert::TryFrom;

use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_data_model::account::{AccountAddress, AccountId};

fn ed25519_pk_with(byte: u8) -> PublicKey {
    let seed = vec![byte; 32];
    let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
    public_key
}

fn main() {
    let domains = [
        "default",
        "treasury",
        "wonderland",
        "iroha",
        "alpha",
        "omega",
        "governance",
        "validators",
        "explorer",
        "soranet",
        "kitsune",
        "da",
    ];
    for (index, label) in domains.iter().enumerate() {
        let index_u8 = u8::try_from(index).expect("domain index exceeds u8");
        let account = AccountId::new(ed25519_pk_with(index_u8));
        let address = AccountAddress::from_account_id(&account).expect("address encoding");
        let canonical = address
            .canonical_hex()
            .expect("canonical hex encoding must succeed");
        let i105 = address.to_i105().expect("i105 encoding must succeed");
        println!("{label}:{index} ->\n  canonical: {canonical}\n  i105: {i105}");
    }
}
