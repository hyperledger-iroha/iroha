#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_crypto::KeyPair;
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_data_model::account::AccountId;

/// Create new account from a random keypair in the given domain
pub fn gen_account_in(domain: impl core::fmt::Display) -> (AccountId, KeyPair) {
    let key_pair = KeyPair::random();
    let domain: iroha_data_model::domain::DomainId = domain
        .to_string()
        .parse()
        .expect("domain name should be valid");

    let account_id = AccountId::new(key_pair.public_key().clone());

    (account_id, key_pair)
}
