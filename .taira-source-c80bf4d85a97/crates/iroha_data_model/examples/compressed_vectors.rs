//! Dump reference I105 account addresses for documentation fixtures.

use std::convert::TryFrom;

use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_data_model::account::{AccountAddress, AccountId};

fn ed25519_pk_with(byte: u8) -> Result<PublicKey, iroha_crypto::Error> {
    let seed = vec![byte; 32];
    let (public_key, _) = KeyPair::try_from_seed(seed, Algorithm::Ed25519)?.into_parts();
    Ok(public_key)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        let account = AccountId::new(ed25519_pk_with(index_u8)?);
        let address = AccountAddress::from_account_id(&account).expect("address encoding");
        let canonical = address
            .canonical_hex()
            .expect("canonical hex encoding must succeed");
        let i105 = address.to_i105().expect("i105 encoding must succeed");
        println!("{label}:{index} ->\n  canonical: {canonical}\n  i105: {i105}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ed25519_pk_with_uses_checked_seed_derivation() {
        let public_key = ed25519_pk_with(0x42).expect("checked Ed25519 fixture derivation");

        assert_eq!(
            public_key.try_algorithm().expect("public key algorithm"),
            Algorithm::Ed25519
        );
    }
}
