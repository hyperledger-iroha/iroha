//! Print a fresh Ed25519 keypair and canonical account material as JSON.

use iroha::{
    crypto::{Algorithm, ExposedPrivateKey, KeyPair},
    data_model::prelude::AccountId,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key_pair = dev_key_pair()?;
    let (public_key, private_key) = key_pair.into_parts();
    let (algorithm, public_key_raw) = public_key
        .try_to_bytes()
        .expect("generated public key must be well-formed");
    assert_eq!(algorithm, Algorithm::Ed25519);
    let account_id = AccountId::new(public_key.clone())
        .canonical_i105()
        .expect("single-key account ids should encode as canonical i105");

    println!(
        concat!(
            "{{",
            "\"account_id\":\"{}\",",
            "\"public_key\":\"{}\",",
            "\"private_key\":\"{}\",",
            "\"public_key_raw_hex\":\"{}\"",
            "}}"
        ),
        account_id,
        public_key.to_string(),
        ExposedPrivateKey(private_key).to_string(),
        hex::encode(public_key_raw),
    );

    Ok(())
}

fn dev_key_pair() -> Result<KeyPair, iroha::crypto::Error> {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_key_pair_uses_checked_ed25519_generation() {
        let key_pair = dev_key_pair().expect("checked Ed25519 dev key generation");

        assert_eq!(
            key_pair
                .public_key()
                .try_algorithm()
                .expect("generated public key algorithm"),
            Algorithm::Ed25519
        );
    }
}
