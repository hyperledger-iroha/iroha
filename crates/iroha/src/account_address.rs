//! Account address encoding and decoding helpers for the Rust SDK.
//!
//! This module exposes a stable surface over [`iroha_data_model::account::AccountAddress`]
//! so downstream consumers can format and parse account identifiers without depending on the
//! internal layout of the data model crate. The helpers mirror the roadmap item *ADDR-4a* and
//! keep error reporting stable via [`AccountAddressErrorCode`].

use iroha_data_model::account::AccountId;
pub use iroha_data_model::account::{
    AccountAddressSource, ParsedAccountId,
    address::{
        AccountAddress, AccountAddressError, AccountAddressErrorCode, AddressDomainKind,
        DefaultDomainLabelError, chain_discriminant, default_domain_name, set_chain_discriminant,
        set_default_domain_name,
    },
};

/// Parsed account address with domain-selector metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedAccountAddress {
    /// Canonical account address payload.
    pub address: AccountAddress,
    /// Domain selector classification embedded in the payload.
    pub domain_kind: AddressDomainKind,
}

impl ParsedAccountAddress {
    /// Canonical hexadecimal representation (`0x…`).
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if canonical bytes could not be constructed.
    pub fn canonical_hex(&self) -> Result<String, AccountAddressError> {
        self.address.canonical_hex()
    }

    /// Format the address as I105 using the provided chain discriminant.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if encoding fails.
    pub fn to_i105_with_discriminant(
        &self,
        network_prefix: u16,
    ) -> Result<String, AccountAddressError> {
        self.address.to_i105_for_discriminant(network_prefix)
    }

    /// Encode the address as canonical I105 using the configured discriminant.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if encoding fails.
    pub fn to_i105(&self) -> Result<String, AccountAddressError> {
        self.address.to_i105()
    }

    /// Domain selector classification helper.
    #[must_use]
    pub const fn domain_kind(&self) -> AddressDomainKind {
        self.domain_kind
    }
}

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
) -> Result<ParsedAccountAddress, AccountAddressError> {
    let address = AccountAddress::parse_encoded(input, expected_prefix)?;
    Ok(ParsedAccountAddress {
        domain_kind: address.domain_kind(),
        address,
    })
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    #[test]
    fn roundtrip_i105_encoding() {
        let key_pair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());

        let encoded = encode_account_id_to_i105_for_discriminant(&account, 42).expect("encode");
        let parsed = parse_account_address(&encoded, Some(42)).expect("parse i105");

        let expected = AccountAddress::from_account_id(&account).expect("address");
        assert_eq!(parsed.address, expected);
        assert_eq!(parsed.domain_kind(), AddressDomainKind::Default);
    }

    #[test]
    fn i105_encoding_matches_data_model() {
        let key_pair = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());

        let encoded = encode_account_id_to_i105(&account).expect("encode i105");
        let parsed = parse_account_address(&encoded, None).expect("parse i105");
        assert_eq!(
            parsed.address,
            AccountAddress::from_account_id(&account).expect("address")
        );
        assert_eq!(parsed.domain_kind(), AddressDomainKind::Default);
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
