//! Account address encoding and decoding helpers for the Rust SDK.
//!
//! This module exposes the canonical surface over [`iroha_data_model::account::AccountAddress`]
//! so downstream consumers can format and parse account identifiers without depending on the
//! internal layout of the data model crate. I105 encoding always takes an explicit network
//! prefix, such as [`crate::client::Client::account_chain_discriminant`], without changing
//! process-global formatting state. Errors expose [`AccountAddressErrorCode`].
use iroha_data_model::account::AccountId;
pub use iroha_data_model::account::address::{
    AccountAddress, AccountAddressError, AccountAddressErrorCode,
};
/// Encode an [`AccountId`] into I105 with the supplied `network_prefix`.
///
/// # Errors
///
/// Returns [`AccountAddressError`] if the account cannot be represented or encoding fails.
pub fn encode_account_id_to_i105(
    account: &AccountId,
    network_prefix: u16,
) -> Result<String, AccountAddressError> {
    AccountAddress::from_account_id(account)?.to_i105_for_discriminant(network_prefix)
}
/// Encode an [`AccountId`] into canonical hexadecimal representation (`0x…`).
///
/// # Errors
///
/// Returns [`AccountAddressError`] if the account cannot be represented.
pub fn encode_account_id_to_canonical_hex(
    account: &AccountId,
) -> Result<String, AccountAddressError> {
    AccountAddress::from_account_id(account)?.canonical_hex()
}
/// Parse an address string in strict encoded i105 form.
///
/// # Errors
///
/// Returns [`AccountAddressError`] if decoding fails or, when `expected_prefix` is supplied, the
/// chain discriminant sentinel does not match.
pub fn parse_account_address(
    input: &str,
    expected_prefix: Option<u16>,
) -> Result<AccountAddress, AccountAddressError> {
    AccountAddress::parse_encoded(input, expected_prefix)
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    #[test]
    fn roundtrip_i105_encoding() {
        let key_pair = KeyPair::try_from_seed(vec![0xAB; 32], Algorithm::Ed25519)
            .expect("derive account-address fixture key");
        let account = AccountId::new(key_pair.public_key().clone());
        let encoded = encode_account_id_to_i105(&account, 42).expect("encode");
        let parsed = parse_account_address(&encoded, Some(42)).expect("parse i105");
        let expected = AccountAddress::from_account_id(&account).expect("address");
        assert_eq!(parsed, expected);
    }
    #[test]
    fn i105_encoding_matches_data_model() {
        let key_pair = KeyPair::try_from_seed(vec![0xCD; 32], Algorithm::Ed25519)
            .expect("derive account-address fixture key");
        let account = AccountId::new(key_pair.public_key().clone());
        let encoded = encode_account_id_to_i105(&account, 753).expect("encode i105");
        let parsed = parse_account_address(&encoded, Some(753)).expect("parse i105");
        assert_eq!(
            parsed,
            AccountAddress::from_account_id(&account).expect("address")
        );
    }
    #[test]
    fn explicit_network_prefix_is_isolated_across_concurrent_contexts() {
        let key_pair = KeyPair::try_from_seed(vec![0xEF; 32], Algorithm::Ed25519)
            .expect("derive account-address fixture key");
        let account = AccountId::new(key_pair.public_key().clone());
        let original = iroha_data_model::account::address::chain_discriminant();
        let barrier = std::sync::Barrier::new(3);
        std::thread::scope(|scope| {
            for (network_prefix, ambient_prefix) in [(369, 753), (753, 42), (42, 369)] {
                let account = &account;
                let barrier = &barrier;
                scope.spawn(move || {
                    let _ambient =
                        iroha_data_model::account::address::ChainDiscriminantGuard::enter(
                            ambient_prefix,
                        );
                    barrier.wait();
                    for _ in 0..32 {
                        let encoded = encode_account_id_to_i105(account, network_prefix)
                            .expect("encode explicit context");
                        assert_eq!(
                            parse_account_address(&encoded, Some(network_prefix))
                                .expect("bound prefix"),
                            AccountAddress::from_account_id(account).expect("canonical address")
                        );
                        assert!(parse_account_address(&encoded, Some(ambient_prefix)).is_err());
                        assert_eq!(
                            iroha_data_model::account::address::chain_discriminant(),
                            ambient_prefix
                        );
                    }
                });
            }
        });
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            original
        );
    }

    #[test]
    fn parse_reports_error_codes() {
        let err = parse_account_address("??", None).expect_err("invalid input");
        assert_eq!(
            err.code(),
            AccountAddressErrorCode::UnsupportedAddressFormat
        );
        assert_eq!(
            err.code_str(),
            AccountAddressErrorCode::UnsupportedAddressFormat.as_str()
        );
    }
}
