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
        AccountAddress, AccountAddressError, AccountAddressErrorCode, AccountAddressFormat,
        AddressDomainKind, DefaultDomainLabelError, chain_discriminant, default_domain_name,
        set_chain_discriminant, set_default_domain_name,
    },
};

/// Parsed account address together with the input format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedAccountAddress {
    /// Canonical account address payload.
    pub address: AccountAddress,
    /// Detected format of the supplied string.
    pub format: AccountAddressFormat,
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

    /// Format the address as IH58 using the provided `network_prefix`.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if encoding fails.
    pub fn to_ih58_with_prefix(&self, network_prefix: u16) -> Result<String, AccountAddressError> {
        self.address.to_ih58(network_prefix)
    }

    /// Encode the address using the Sora compressed alphabet (`sora…`).
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if encoding fails.
    pub fn to_compressed_sora(&self) -> Result<String, AccountAddressError> {
        self.address.to_compressed_sora()
    }

    /// Domain selector classification helper.
    #[must_use]
    pub const fn domain_kind(&self) -> AddressDomainKind {
        self.domain_kind
    }
}

/// Encode an [`AccountId`] into IH58 with the supplied `network_prefix`.
///
/// # Errors
///
/// Returns [`AccountAddressError`] if the account cannot be represented or encoding fails.
pub fn encode_account_id_to_ih58(
    account: &AccountId,
    network_prefix: u16,
) -> Result<String, AccountAddressError> {
    AccountAddress::from_account_id(account)?.to_ih58(network_prefix)
}

/// Encode an [`AccountId`] using the Sora compressed alphabet (`sora…`).
///
/// # Errors
///
/// Returns [`AccountAddressError`] if the account cannot be represented or encoding fails.
pub fn encode_account_id_to_compressed(account: &AccountId) -> Result<String, AccountAddressError> {
    AccountAddress::from_account_id(account)?.to_compressed_sora()
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

/// Parse an address string in any supported format, returning both the canonical payload and the detected format.
///
/// # Errors
///
/// Returns [`AccountAddressError`] if decoding fails or, when `expected_prefix` is supplied, the IH58
/// prefix does not match.
pub fn parse_account_address(
    input: &str,
    expected_prefix: Option<u16>,
) -> Result<ParsedAccountAddress, AccountAddressError> {
    let (address, format) = AccountAddress::parse_any(input, expected_prefix)?;
    Ok(ParsedAccountAddress {
        domain_kind: address.domain_kind(),
        address,
        format,
    })
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    #[test]
    fn roundtrip_ih58_encoding() {
        let domain = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain, key_pair.public_key().clone());

        let encoded = encode_account_id_to_ih58(&account, 42).expect("encode");
        let parsed = parse_account_address(&encoded, Some(42)).expect("parse ih58");

        let expected = AccountAddress::from_account_id(&account).expect("address");
        assert_eq!(parsed.address, expected);
        assert!(
            matches!(parsed.format, AccountAddressFormat::IH58 { network_prefix } if network_prefix == 42)
        );
        assert_eq!(parsed.domain_kind(), AddressDomainKind::Default);
    }

    #[test]
    fn compressed_encoding_matches_data_model() {
        let domain = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain, key_pair.public_key().clone());

        let encoded = encode_account_id_to_compressed(&account).expect("encode compressed");
        let parsed = parse_account_address(&encoded, None).expect("parse compressed");
        assert_eq!(
            parsed.address,
            AccountAddress::from_account_id(&account).expect("address")
        );
        assert_eq!(parsed.format, AccountAddressFormat::Compressed);
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
