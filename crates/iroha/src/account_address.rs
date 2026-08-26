//! Account address encoding and decoding helpers for the Rust SDK.
//!
//! This module exposes the canonical surface over [`iroha_data_model::account::AccountAddress`]
//! so downstream consumers can format and parse account identifiers without depending on the
//! internal layout of the data model crate. The helpers mirror the roadmap item *ADDR-4a* and
//! expose typed error reporting via [`AccountAddressErrorCode`].
use iroha_data_model::account::AccountId;
pub use iroha_data_model::account::address::{
    AccountAddress, AccountAddressError, AccountAddressErrorCode, chain_discriminant,
    set_chain_discriminant,
};
/// Encode an [`AccountId`] into I105 with the supplied `network_prefix`.
///
/// # Errors
///
/// Returns [`AccountAddressError`] if the account cannot be represented or encoding fails.
pub fn encode_account_id_to_i105_for_discriminant(
    account: &AccountId,
    network_prefix: u16,
) -> Result<String, AccountAddressError> {
    AccountAddress::from_account_id(account)?.to_i105_for_discriminant(network_prefix)
}
/// Encode an [`AccountId`] as canonical I105 using the configured discriminant.
///
/// # Errors
///
/// Returns [`AccountAddressError`] if the account cannot be represented or encoding fails.
pub fn encode_account_id_to_i105(account: &AccountId) -> Result<String, AccountAddressError> {
    AccountAddress::from_account_id(account)?.to_i105()
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
        let encoded = encode_account_id_to_i105_for_discriminant(&account, 42).expect("encode");
        let parsed = parse_account_address(&encoded, Some(42)).expect("parse i105");
        let expected = AccountAddress::from_account_id(&account).expect("address");
        assert_eq!(parsed, expected);
    }
    #[test]
    fn i105_encoding_matches_data_model() {
        let key_pair = KeyPair::try_from_seed(vec![0xCD; 32], Algorithm::Ed25519)
            .expect("derive account-address fixture key");
        let account = AccountId::new(key_pair.public_key().clone());
        let encoded = encode_account_id_to_i105(&account).expect("encode i105");
        let parsed = parse_account_address(&encoded, None).expect("parse i105");
        assert_eq!(
            parsed,
            AccountAddress::from_account_id(&account).expect("address")
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
