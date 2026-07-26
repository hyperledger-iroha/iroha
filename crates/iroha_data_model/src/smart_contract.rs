//! This module contains data and structures related only to smart contract execution

use std::{format, io::Write, str::FromStr, string::String, vec::Vec};

use bech32::{Bech32m, Hrp};
use iroha_data_model_derive::model;
use iroha_primitives::conststr::ConstString;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    account::{AccountAddressError, AccountId, rekey::AccountAliasDomain},
    error::ParseError,
    name::Name,
    nexus::{DataSpaceCatalog, DataSpaceId},
};

pub mod payloads {
    //! Contexts with function arguments for different entrypoints

    use norito::{
        codec::{Decode, Encode},
        core::DecodeFromSlice,
    };

    use crate::{block::BlockHeader, prelude::*};

    /// Context for smart contract entrypoint
    #[derive(Debug, Clone, Encode, Decode)]
    #[norito(decode_from_slice)]
    pub struct SmartContractContext {
        /// Account that submitted the transaction containing the smart contract
        pub authority: AccountId,
        /// Block currently being processed
        pub curr_block: BlockHeader,
    }

    /// Context for trigger entrypoint
    #[derive(Encode, Decode)]
    #[norito(decode_from_slice)]
    #[cfg_attr(not(feature = "fast_dsl"), derive(Debug, Clone))]
    pub struct TriggerContext {
        /// Id of this trigger
        pub id: TriggerId,
        /// Account that registered the trigger
        pub authority: AccountId,
        /// Block currently being processed
        pub curr_block: BlockHeader,
        /// Event which triggered the execution
        pub event: EventBox,
    }

    /// Context for migrate entrypoint
    #[derive(Debug, Clone, Encode, Decode)]
    #[norito(decode_from_slice)]
    pub struct ExecutorContext {
        /// Account that is executing the operation
        pub authority: AccountId,
        /// Block currently being processed (or latest block hash for queries)
        pub curr_block: BlockHeader,
    }

    /// Generic payload for `validate_*()` entrypoints of executor.
    #[derive(Debug, Clone, Encode, Decode)]
    pub struct Validate<T> {
        /// Context of the executor
        pub context: ExecutorContext,
        /// Operation to be validated
        pub target: T,
    }

    impl<'a, T> DecodeFromSlice<'a> for Validate<T>
    where
        T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
    {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
            norito::core::decode_field_canonical::<Self>(bytes)
        }
    }

    #[cfg(test)]
    mod payloads_tests {
        use core::num::NonZeroU64;

        use iroha_crypto::KeyPair;
        use norito::core::DecodeFromSlice;

        use super::*;

        fn checked_random_account_id() -> AccountId {
            AccountId::new(
                KeyPair::try_random()
                    .expect("test fixture random key generation should succeed")
                    .public_key()
                    .clone(),
            )
        }

        #[test]
        fn validate_decode_from_slice_roundtrips_any_query() {
            let authority = checked_random_account_id();
            let header = BlockHeader {
                height: NonZeroU64::new(1).expect("nonzero height"),
                prev_block_hash: None,
                merkle_root: None,
                result_merkle_root: None,
                da_proof_policies_hash: None,
                da_commitments_hash: None,
                da_pin_intents_hash: None,
                prev_roster_evidence_hash: None,
                npos_effects_hash: None,
                execution_context_hash: None,
                sccp_commitment_root: None,
                creation_time_ms: 0,
                view_change_index: 0,
                confidential_features: None,
            };
            let context = ExecutorContext {
                authority: authority.clone(),
                curr_block: header,
            };
            let target = crate::query::AnyQueryBox::Singular(
                crate::query::SingularQueryBox::FindExecutorDataModel(
                    crate::query::executor::prelude::FindExecutorDataModel,
                ),
            );
            let validate = Validate { context, target };
            let bytes = validate.encode();

            let (decoded, used) = Validate::<crate::query::AnyQueryBox>::decode_from_slice(&bytes)
                .expect("decode validate");
            assert_eq!(used, bytes.len());
            assert_eq!(decoded.context.authority, authority);
            assert_eq!(decoded.context.curr_block, header);
            assert!(matches!(
                decoded.target,
                crate::query::AnyQueryBox::Singular(
                    crate::query::SingularQueryBox::FindExecutorDataModel(_)
                )
            ));
        }
    }
}

/// Metadata key tracking the next public contract deploy nonce for an account.
pub const CONTRACT_DEPLOY_NONCE_METADATA_KEY: &str = "contract_deploy_nonce";

/// Runtime permission marker required for a contract's `hajimari`/`始まり` lifecycle entrypoint.
///
/// The host materializes this marker as an exact `CanInvokeContractEntrypoint`
/// token bound to the deployed address and lifecycle selector.
pub const CONTRACT_HAJIMARI_PERMISSION_NAME: &str = "CanInvokeContractEntrypoint";

/// Runtime permission required to invoke a contract's `kaizen`/`改善` lifecycle entrypoint.
///
/// ABI V1 uses the same address-and-selector scoped invocation permission for
/// lifecycle operations as it does for permissioned public entrypoints.
pub const CONTRACT_KAIZEN_PERMISSION_NAME: &str = "CanInvokeContractEntrypoint";

/// Default mainnet contract HRP used for Bech32m-encoded contract addresses.
pub const CONTRACT_ADDRESS_HRP_MAINNET: &str = "sorac";
/// Default Taira/testnet contract HRP used for Bech32m-encoded contract addresses.
pub const CONTRACT_ADDRESS_HRP_TAIRA: &str = "tairac";
/// Mainnet chain discriminant used by Sora Nexus address encoding.
pub const CHAIN_DISCRIMINANT_MAINNET: u16 = 753;
/// Taira/testnet chain discriminant used by Sora Nexus address encoding.
pub const CHAIN_DISCRIMINANT_TAIRA: u16 = 369;

const CONTRACT_ADDRESS_VERSION_V1: u8 = 1;
const CONTRACT_ADDRESS_TAG_V1: &[u8] = b"iroha:contract-address:v1";
const CONTRACT_SUBJECT_HASH_TO_POINT_TAG_V1: &[u8] = b"iroha:contract-subject:hash-to-point:v1:";
const CONTRACT_ADDRESS_HASH_LEN: usize = 20;
const CONTRACT_ADDRESS_PAYLOAD_LEN_V1: usize = 1 + 8 + CONTRACT_ADDRESS_HASH_LEN;

pub use self::model::*;

#[model]
mod model {
    use derive_more::Display;

    use super::*;

    /// Canonical Bech32m-encoded public contract address.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct ContractAlias(pub(super) ConstString);

    /// Canonical Bech32m-encoded public contract address.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct ContractAddress(pub(super) ConstString);

    /// Active smart-contract instance binding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct ContractInstance {
        /// Canonical deployed contract address.
        pub contract_address: ContractAddress,
        /// Optional stable alias bound to the instance.
        pub contract_alias: Option<ContractAlias>,
        /// Code hash currently activated for this address.
        pub code_hash: iroha_crypto::Hash,
    }
}

struct ContractAliasSegments<'a> {
    name: &'a str,
    domain: Option<&'a str>,
    dataspace: &'a str,
}

impl ContractAlias {
    /// Build a contract alias from validated components.
    ///
    /// # Errors
    /// Returns [`ParseError`] when any component is invalid.
    pub fn from_components(
        name: &str,
        domain_alias: Option<&str>,
        dataspace_alias: &str,
    ) -> Result<Self, ParseError> {
        let name = normalize_contract_alias_segment(name, "contract alias name")?;
        let domain_alias = domain_alias
            .map(|value| normalize_contract_alias_segment(value, "contract alias domain"))
            .transpose()?;
        let dataspace_alias =
            normalize_contract_alias_segment(dataspace_alias, "contract alias dataspace")?;
        let literal = domain_alias.map_or_else(
            || format!("{name}::{dataspace_alias}"),
            |domain_alias| format!("{name}::{domain_alias}.{dataspace_alias}"),
        );
        literal.parse()
    }

    /// Contract alias name segment (`<name>`).
    #[must_use]
    pub fn name_segment(&self) -> &str {
        let segments = split_contract_alias_segments(self.as_ref())
            .expect("contract alias must remain valid after construction");
        segments.name
    }

    /// Optional alias-domain segment (`<domain>`).
    #[must_use]
    pub fn domain_segment(&self) -> Option<&str> {
        let segments = split_contract_alias_segments(self.as_ref())
            .expect("contract alias must remain valid after construction");
        segments.domain
    }

    /// Dataspace segment (`<dataspace>`).
    #[must_use]
    pub fn dataspace_segment(&self) -> &str {
        let segments = split_contract_alias_segments(self.as_ref())
            .expect("contract alias must remain valid after construction");
        segments.dataspace
    }

    /// Resolve the alias components against the dataspace catalog.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the dataspace alias is unknown.
    pub fn resolve_components(
        &self,
        catalog: &DataSpaceCatalog,
    ) -> Result<(Name, Option<AccountAliasDomain>, DataSpaceId), ParseError> {
        let name = self
            .name_segment()
            .parse()
            .map_err(|_| ParseError::new("contract alias name segment is invalid"))?;
        let domain = self
            .domain_segment()
            .map(str::parse::<AccountAliasDomain>)
            .map(|result| {
                result.map_err(|_| ParseError::new("contract alias domain segment is invalid"))
            })
            .transpose()?;
        let dataspace = catalog
            .by_alias(self.dataspace_segment())
            .map(|entry| entry.id)
            .ok_or_else(|| ParseError::new("unknown dataspace alias in contract alias"))?;
        Ok((name, domain, dataspace))
    }
}

fn split_contract_alias_segments(input: &str) -> Result<ContractAliasSegments<'_>, ParseError> {
    let (name, right) = input.split_once("::").ok_or_else(|| {
        ParseError::new(
            "contract alias must use `<name>::<domain>.<dataspace>` or `<name>::<dataspace>` format",
        )
    })?;
    if right.contains("::") {
        return Err(ParseError::new(
            "contract alias must contain exactly one `::` separator",
        ));
    }
    if right.contains('@') {
        return Err(ParseError::new(
            "contract alias must use `.` instead of `@` between domain and dataspace",
        ));
    }
    let dot_count = right.bytes().filter(|byte| *byte == b'.').count();
    if dot_count == 1 {
        let (domain, dataspace) = right.split_once('.').expect("counted dot");
        return Ok(ContractAliasSegments {
            name,
            domain: Some(domain),
            dataspace,
        });
    }
    if dot_count > 1 {
        return Err(ParseError::new(
            "contract alias must contain at most one `.` after `::`",
        ));
    }
    Ok(ContractAliasSegments {
        name,
        domain: None,
        dataspace: right,
    })
}

fn normalize_contract_alias_segment(
    value: &str,
    segment: &'static str,
) -> Result<String, ParseError> {
    if value.is_empty() {
        return Err(ParseError::new("contract alias segments must not be empty"));
    }
    if value.contains(':') {
        return Err(ParseError::new(match segment {
            "contract alias name" => "contract alias name segment must not contain `:`",
            "contract alias domain" => "contract alias domain segment must not contain `:`",
            "contract alias dataspace" => "contract alias dataspace segment must not contain `:`",
            _ => "contract alias segment must not contain `:`",
        }));
    }
    if matches!(
        segment,
        "contract alias domain" | "contract alias dataspace"
    ) && value.contains('.')
    {
        return Err(ParseError::new(match segment {
            "contract alias domain" => "contract alias domain segment must not contain `.`",
            "contract alias dataspace" => "contract alias dataspace segment must not contain `.`",
            _ => "contract alias segment must not contain `.`",
        }));
    }
    let normalized = Name::from_str(value).map_err(|_| {
        ParseError::new(match segment {
            "contract alias name" => "contract alias name segment is invalid",
            "contract alias domain" => "contract alias domain segment is invalid",
            "contract alias dataspace" => "contract alias dataspace segment is invalid",
            _ => "contract alias segment is invalid",
        })
    })?;
    Ok(normalized.as_ref().to_owned())
}

impl FromStr for ContractAlias {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new("contract alias must not be empty"));
        }
        if trimmed != value {
            return Err(ParseError::new(
                "contract alias must not contain leading or trailing whitespace",
            ));
        }
        if trimmed.chars().any(char::is_control) {
            return Err(ParseError::new(
                "contract alias must not contain control characters",
            ));
        }

        let segments = split_contract_alias_segments(trimmed)?;
        let name = normalize_contract_alias_segment(segments.name, "contract alias name")?;
        let domain = segments
            .domain
            .map(|value| normalize_contract_alias_segment(value, "contract alias domain"))
            .transpose()?;
        let dataspace =
            normalize_contract_alias_segment(segments.dataspace, "contract alias dataspace")?;
        let canonical = domain.map_or_else(
            || format!("{name}::{dataspace}"),
            |domain| format!("{name}::{domain}.{dataspace}"),
        );
        Ok(Self(ConstString::from(&*canonical)))
    }
}

impl AsRef<str> for ContractAlias {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl norito::core::NoritoSerialize for ContractAlias {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        <&str as norito::core::NoritoSerialize>::serialize(&self.as_ref(), writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_hint(&self.as_ref())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_exact(&self.as_ref())
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for ContractAlias {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ContractAlias deserialization must succeed for valid archives")
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let value = <String as norito::core::NoritoDeserialize>::try_deserialize(archived.cast())?;
        ContractAlias::from_str(&value)
            .map_err(|err| norito::core::Error::Message(err.reason.into()))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ContractAlias {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.as_ref(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ContractAlias {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse()
            .map_err(|err: ParseError| norito::json::Error::Message(err.reason.into()))
    }
}

/// Errors returned when deriving or parsing a [`ContractAddress`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContractAddressError {
    /// The supplied literal was empty or malformed.
    #[error("invalid contract address: {0}")]
    InvalidLiteral(String),
    /// Bech32m HRP parsing failed.
    #[error("invalid contract address hrp: {0}")]
    InvalidHrp(String),
    /// The payload version is not recognized.
    #[error("unsupported contract address version {0}")]
    UnsupportedVersion(u8),
    /// The payload length does not match the expected version layout.
    #[error("invalid contract address payload length {found}; expected {expected}")]
    InvalidPayloadLength {
        /// Bytes actually decoded from the payload.
        found: usize,
        /// Bytes expected for the active address format version.
        expected: usize,
    },
    /// Deployer account canonicalization failed during address derivation.
    #[error("failed to derive contract address from deployer account: {0}")]
    InvalidDeployer(String),
}

impl ContractAddress {
    /// Derive a deterministic contract address from deployer identity, nonce, and dataspace.
    ///
    /// The address payload is versioned and encoded as:
    /// `version || dataspace_id_be || blake3(preimage)[..20]`.
    ///
    /// The preimage is domain-separated and includes the chain discriminant so the resulting
    /// address is network-specific.
    ///
    /// # Errors
    /// Returns an error when the derived HRP is invalid, the deployer account cannot be
    /// canonicalized into an address, or the final Bech32m literal cannot be encoded.
    pub fn derive(
        chain_discriminant: u16,
        deployer: &AccountId,
        deploy_nonce: u64,
        dataspace_id: DataSpaceId,
    ) -> Result<Self, ContractAddressError> {
        let hrp = contract_hrp_for_chain_discriminant(chain_discriminant);
        let hrp =
            Hrp::parse(&hrp).map_err(|err| ContractAddressError::InvalidHrp(err.to_string()))?;

        let deployer_bytes = deployer
            .to_account_address()
            .and_then(|address| address.canonical_bytes())
            .map_err(|err: AccountAddressError| {
                ContractAddressError::InvalidDeployer(err.to_string())
            })?;

        let mut preimage =
            Vec::with_capacity(CONTRACT_ADDRESS_TAG_V1.len() + 2 + 8 + 8 + deployer_bytes.len());
        preimage.extend_from_slice(CONTRACT_ADDRESS_TAG_V1);
        preimage.extend_from_slice(&chain_discriminant.to_be_bytes());
        preimage.extend_from_slice(&dataspace_id.as_u64().to_be_bytes());
        preimage.extend_from_slice(&deploy_nonce.to_be_bytes());
        preimage.extend_from_slice(&deployer_bytes);

        let digest = blake3::hash(&preimage);
        let mut payload = Vec::with_capacity(CONTRACT_ADDRESS_PAYLOAD_LEN_V1);
        payload.push(CONTRACT_ADDRESS_VERSION_V1);
        payload.extend_from_slice(&dataspace_id.as_u64().to_be_bytes());
        payload.extend_from_slice(&digest.as_bytes()[..CONTRACT_ADDRESS_HASH_LEN]);

        let encoded = bech32::encode::<Bech32m>(hrp, &payload)
            .map_err(|err| ContractAddressError::InvalidLiteral(err.to_string()))?;
        encoded.parse()
    }

    /// Decode the dataspace identifier embedded in the address payload.
    ///
    /// # Errors
    /// Returns an error when the address literal cannot be decoded or when it uses an unsupported
    /// payload version.
    pub fn dataspace_id(&self) -> Result<DataSpaceId, ContractAddressError> {
        let (_, payload) = decode_contract_address(self.as_ref())?;
        let version = payload[0];
        if version != CONTRACT_ADDRESS_VERSION_V1 {
            return Err(ContractAddressError::UnsupportedVersion(version));
        }
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(&payload[1..9]);
        Ok(DataSpaceId::new(u64::from_be_bytes(bytes)))
    }

    /// Derive the canonical, non-signable contract subject identifier used for contract-owned
    /// authority.
    ///
    /// The first-release algorithm hashes the canonical contract address directly into a valid
    /// Ed25519 public point. It deliberately does not derive a scalar/private key: knowing the
    /// public contract address therefore does not reveal signing material for the subject account.
    /// The domain and retry counter encoding are consensus-critical ABI V1 constants.
    #[must_use]
    pub fn subject_id(&self) -> AccountId {
        let mut counter = 0_u32;
        loop {
            let counter_bytes = counter.to_be_bytes();
            let candidate = iroha_crypto::Hash::new_from_chunks(&[
                CONTRACT_SUBJECT_HASH_TO_POINT_TAG_V1,
                self.as_ref().as_bytes(),
                &counter_bytes,
            ]);
            if let Ok(public_key) = iroha_crypto::PublicKey::from_bytes(
                iroha_crypto::Algorithm::Ed25519,
                candidate.as_ref(),
            ) {
                return AccountId::new(public_key);
            }
            counter = counter
                .checked_add(1)
                .expect("contract subject hash-to-point retry counter exhausted");
        }
    }

    /// Borrow the canonical encoded literal.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl AsRef<str> for ContractAddress {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl norito::core::NoritoSerialize for ContractAddress {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        <&str as norito::core::NoritoSerialize>::serialize(&self.as_ref(), writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_hint(&self.as_ref())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_exact(&self.as_ref())
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for ContractAddress {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ContractAddress deserialization must succeed for valid archives")
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let value = <String as norito::core::NoritoDeserialize>::try_deserialize(archived.cast())?;
        ContractAddress::from_str(&value)
            .map_err(|err| norito::core::Error::Message(err.to_string()))
    }
}

impl FromStr for ContractAddress {
    type Err = ContractAddressError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        decode_contract_address(value)?;
        Ok(Self(ConstString::from(value)))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ContractAddress {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.as_ref(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ContractAddress {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse()
            .map_err(|err: ContractAddressError| norito::json::Error::Message(err.to_string()))
    }
}

/// Resolve the default contract-address HRP for the provided chain discriminant.
#[must_use]
pub fn contract_hrp_for_chain_discriminant(chain_discriminant: u16) -> String {
    match chain_discriminant {
        CHAIN_DISCRIMINANT_MAINNET => CONTRACT_ADDRESS_HRP_MAINNET.to_owned(),
        CHAIN_DISCRIMINANT_TAIRA => CONTRACT_ADDRESS_HRP_TAIRA.to_owned(),
        other => format!("c{other:x}"),
    }
}

fn decode_contract_address(value: &str) -> Result<(Hrp, Vec<u8>), ContractAddressError> {
    if value.trim().is_empty() {
        return Err(ContractAddressError::InvalidLiteral(
            "contract address must not be empty".to_owned(),
        ));
    }
    if value.trim() != value {
        return Err(ContractAddressError::InvalidLiteral(
            "contract address must not contain leading or trailing whitespace".to_owned(),
        ));
    }

    let (hrp, payload) = bech32::decode(value)
        .map_err(|err| ContractAddressError::InvalidLiteral(err.to_string()))?;
    if payload.is_empty() {
        return Err(ContractAddressError::InvalidPayloadLength {
            found: 0,
            expected: CONTRACT_ADDRESS_PAYLOAD_LEN_V1,
        });
    }

    match payload[0] {
        CONTRACT_ADDRESS_VERSION_V1 => {
            if payload.len() != CONTRACT_ADDRESS_PAYLOAD_LEN_V1 {
                return Err(ContractAddressError::InvalidPayloadLength {
                    found: payload.len(),
                    expected: CONTRACT_ADDRESS_PAYLOAD_LEN_V1,
                });
            }
        }
        version => return Err(ContractAddressError::UnsupportedVersion(version)),
    }

    if hrp.as_str().is_empty() {
        return Err(ContractAddressError::InvalidLiteral(
            "contract address hrp must not be empty".to_owned(),
        ));
    }

    Ok((hrp, payload))
}

/// Re-export commonly used smart-contract types.
pub mod prelude {
    pub use super::{
        CONTRACT_DEPLOY_NONCE_METADATA_KEY, ContractAddress, ContractAlias, ContractInstance,
    };
}

#[cfg(test)]
mod contract_address_tests {
    use iroha_crypto::KeyPair;

    use super::*;

    fn checked_random_account_id() -> AccountId {
        AccountId::new(
            KeyPair::try_random()
                .expect("test fixture random key generation should succeed")
                .public_key()
                .clone(),
        )
    }

    #[test]
    fn contract_address_derivation_is_deterministic() {
        let authority = checked_random_account_id();
        let first = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            7,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let second = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            7,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        assert_eq!(first, second);
        assert_eq!(
            first.dataspace_id().expect("dataspace"),
            DataSpaceId::UNIVERSAL
        );
        assert!(first.as_str().starts_with(CONTRACT_ADDRESS_HRP_MAINNET));
    }

    #[test]
    fn contract_address_derivation_changes_with_nonce_and_network() {
        let authority = checked_random_account_id();
        let mainnet = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("mainnet address");
        let next_nonce = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            1,
            DataSpaceId::UNIVERSAL,
        )
        .expect("nonce+1 address");
        let taira = ContractAddress::derive(
            CHAIN_DISCRIMINANT_TAIRA,
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("taira address");

        assert_ne!(mainnet, next_nonce);
        assert_ne!(mainnet, taira);
        assert!(taira.as_str().starts_with(CONTRACT_ADDRESS_HRP_TAIRA));
    }

    #[test]
    fn contract_address_subject_is_deterministic_and_unique_per_address() {
        let authority = checked_random_account_id();
        let first = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("first contract address");
        let second = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            1,
            DataSpaceId::UNIVERSAL,
        )
        .expect("second contract address");

        assert_eq!(first.subject_id(), first.subject_id());
        assert_ne!(first.subject_id(), second.subject_id());
    }

    #[test]
    fn contract_address_subject_consensus_vector() {
        let address: ContractAddress =
            "sorac1qyqqqqqqqqqqqqpze5aq5vfxha4qlvu4q80e0ff4yesw50cuwzvx4"
                .parse()
                .expect("pinned contract address");

        assert_eq!(
            hex::encode(address.subject_id().signatory().to_bytes().1),
            "927d662b25886a1bc1f24e4fb974fb4b1e0bcba63728f807885ab757092611d3"
        );
    }

    #[test]
    fn contract_address_parser_rejects_invalid_literals() {
        let err = "not-an-address"
            .parse::<ContractAddress>()
            .expect_err("invalid address must fail");
        assert!(
            matches!(
                err,
                ContractAddressError::InvalidLiteral(_) | ContractAddressError::InvalidHrp(_)
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn contract_alias_parses_long_literal() {
        let alias: ContractAlias = "router::dex.universal".parse().expect("valid alias");
        assert_eq!(alias.name_segment(), "router");
        assert_eq!(alias.domain_segment(), Some("dex"));
        assert_eq!(alias.dataspace_segment(), "universal");
    }

    #[test]
    fn contract_alias_parses_short_literal() {
        let alias: ContractAlias = "router::universal".parse().expect("valid alias");
        assert_eq!(alias.name_segment(), "router");
        assert_eq!(alias.domain_segment(), None);
        assert_eq!(alias.dataspace_segment(), "universal");
    }

    #[test]
    fn contract_alias_resolves_alias_domain_segment() {
        let alias: ContractAlias = "router::dex.centralbank".parse().expect("valid alias");
        let catalog = DataSpaceCatalog::new(vec![
            crate::nexus::DataSpaceMetadata::default(),
            crate::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(9),
                alias: "centralbank".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("catalog");

        let (name, domain, dataspace) = alias.resolve_components(&catalog).expect("resolve");

        assert_eq!(name, "router".parse::<Name>().expect("name"));
        assert_eq!(
            domain,
            Some(
                "dex"
                    .parse::<AccountAliasDomain>()
                    .expect("alias-domain segment")
            )
        );
        assert_eq!(dataspace, DataSpaceId::new(9));
    }

    #[test]
    fn contract_alias_rejects_invalid_literals() {
        for raw in [
            "",
            " ",
            "router",
            "router@universal",
            "router#universal",
            "router:::universal",
            "router::dex.universal.extra",
            "router::",
            "::universal",
        ] {
            assert!(raw.parse::<ContractAlias>().is_err(), "must fail: {raw}");
        }
    }

    #[test]
    fn contract_alias_norito_wire_is_validated_string_literal() {
        let alias: ContractAlias = "router::dex.universal".parse().expect("valid alias");
        let alias_bytes = norito::codec::Encode::encode(&alias);
        let string_bytes = norito::codec::Encode::encode(&alias.as_ref().to_owned());
        assert_eq!(alias_bytes, string_bytes);

        let decoded = <ContractAlias as norito::codec::Decode>::decode(&mut alias_bytes.as_slice())
            .expect("decode contract alias");
        assert_eq!(decoded, alias);

        let invalid_bytes = norito::codec::Encode::encode(&"router".to_owned());
        let err = <ContractAlias as norito::codec::Decode>::decode(&mut invalid_bytes.as_slice())
            .expect_err("invalid alias literal must fail");
        assert!(err.to_string().contains("contract alias"));
    }

    #[test]
    fn contract_address_norito_wire_is_validated_string_literal() {
        let authority = checked_random_account_id();
        let address = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            12,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let address_bytes = norito::codec::Encode::encode(&address);
        let string_bytes = norito::codec::Encode::encode(&address.as_ref().to_owned());
        assert_eq!(address_bytes, string_bytes);

        let decoded =
            <ContractAddress as norito::codec::Decode>::decode(&mut address_bytes.as_slice())
                .expect("decode contract address");
        assert_eq!(decoded, address);

        let invalid_bytes = norito::codec::Encode::encode(&"not-an-address".to_owned());
        let err = <ContractAddress as norito::codec::Decode>::decode(&mut invalid_bytes.as_slice())
            .expect_err("invalid address literal must fail");
        assert!(err.to_string().contains("invalid contract address"));
    }
}
/// Exact recursive schemas for public Kotodama entrypoint boundaries.
pub mod entrypoint;

// Smart contract manifest types and helpers.
pub mod manifest {
    //! Manifest metadata for IVM smart contracts.
    //! It can be attached to a transaction's `metadata` under a well-known
    //! key for admission-time checks. When attached or registered, a V1
    //! manifest must carry both consensus-binding hashes.

    use iroha_crypto::{Error as CryptoError, Hash, KeyPair, PublicKey, Signature};
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};
    #[cfg(feature = "json")]
    use norito::json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize};

    use crate::{
        account::AccountId,
        events::EventFilterBox,
        metadata::Metadata,
        smart_contract::entrypoint::{EntrypointArgumentSchemaV1, EntrypointValueTypeV1},
        trigger::{TriggerId, action::Repeats},
    };

    /// Well-known metadata key used to attach a contract manifest.
    pub const MANIFEST_METADATA_KEY: &str = "contract_manifest";

    /// Smart contract manifest used for admission-time validation.
    ///
    /// `code_hash` and `abi_hash` remain represented as options so malformed
    /// external payloads can be decoded into a stable, structured admission
    /// error. Every V1 registration and every admission path that observes a
    /// manifest rejects either field when absent; the remaining fields are
    /// optional metadata.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[norito(reuse_archived)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct ContractManifest {
        /// Canonical source-level seiyaku name embedded by the compiler.
        #[norito(default)]
        pub seiyaku_name: Option<String>,
        /// Content-addressed hash of the compiled `.to` bytecode.
        /// Required in V1 and compared with the complete submitted artifact.
        pub code_hash: Option<Hash>,
        /// ABI hash computed by the node for the `abi_version` policy.
        /// Required in V1 and must match the artifact's authenticated CNTR
        /// binding and the node's canonical ABI descriptor.
        pub abi_hash: Option<Hash>,
        /// Optional compiler fingerprint (e.g., rustc/LLVM versions).
        pub compiler_fingerprint: Option<String>,
        /// Compiler-derived, hash-covered execution capability bitmap.
        ///
        /// V1 mirrors the artifact's ZK and VECTOR execution-mode bits. This is
        /// not source-selectable metadata and never describes host SIMD, Metal,
        /// or CUDA availability.
        pub features_bitmap: Option<u64>,
        /// Optional advisory access-set hints for scheduler.
        ///
        /// When present, the scheduler may use these read/write keys for conflict
        /// detection without requiring a dynamic VM prepass. Keys are canonical
        /// strings of the form `account:…`, `domain:…`, `asset_def:…`, `asset:…`,
        /// `nft:…`, or their `*.detail:…` variants, matching the internal
        /// pipeline access-key format.
        #[norito(default)]
        pub access_set_hints: Option<AccessSetHints>,
        /// Optional entrypoint descriptors (name, kind, permission) advertised by the compiler.
        #[norito(default)]
        pub entrypoints: Option<Vec<EntrypointDescriptor>>,
        /// Optional durable state schema advertised by the compiler.
        #[norito(default)]
        pub states: Option<Vec<StateDescriptor>>,
        /// Stable application error codes advertised by the compiler.
        #[norito(default)]
        pub error_codes: Option<Vec<ContractErrorCodeDescriptor>>,
        /// Optional localization tables extracted from `kotoba { ... }` blocks.
        #[norito(default)]
        pub kotoba: Option<Vec<KotobaTranslationEntry>>,
        /// Provenance metadata for the manifest, including signer and signature.
        #[norito(default)]
        pub provenance: Option<ManifestProvenance>,
    }

    /// Bounded dynamic state access advertised by a compiler.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    pub struct DynamicAccessHint {
        /// Canonical state-map base key, for example `state:Balances`.
        pub base_key: String,
        /// Canonical Kotodama key type name for the map.
        pub key_type: String,
        /// Human-readable bound source, for example `take` or `range`.
        pub bound_kind: String,
        /// Maximum number of state keys touched by this dynamic access.
        pub max_keys: u32,
    }

    /// Advisory read/write keys used by the scheduler when present in a manifest.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    pub struct AccessSetHints {
        /// Keys that the contract expects to read for a given entrypoint.
        pub read_keys: Vec<String>,
        /// Keys that the contract expects to write for a given entrypoint.
        pub write_keys: Vec<String>,
        /// Bounded dynamic state-map reads generated by the compiler.
        #[norito(default)]
        pub dynamic_reads: Vec<DynamicAccessHint>,
        /// Bounded dynamic state-map writes generated by the compiler.
        #[norito(default)]
        pub dynamic_writes: Vec<DynamicAccessHint>,
    }

    #[cfg(feature = "json")]
    impl FastJsonWrite for AccessSetHints {
        fn write_json(&self, out: &mut String) {
            out.push('{');
            json::write_json_string("read_keys", out);
            out.push(':');
            JsonSerialize::json_serialize(&self.read_keys, out);
            out.push(',');
            json::write_json_string("write_keys", out);
            out.push(':');
            JsonSerialize::json_serialize(&self.write_keys, out);
            out.push(',');
            json::write_json_string("dynamic_reads", out);
            out.push(':');
            JsonSerialize::json_serialize(&self.dynamic_reads, out);
            out.push(',');
            json::write_json_string("dynamic_writes", out);
            out.push(':');
            JsonSerialize::json_serialize(&self.dynamic_writes, out);
            out.push('}');
        }
    }

    #[cfg(feature = "json")]
    impl JsonDeserialize for AccessSetHints {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            parser.skip_ws();
            parser.consume_char(b'{')?;

            let mut read_keys: Option<Vec<String>> = None;
            let mut write_keys: Option<Vec<String>> = None;
            let mut dynamic_reads: Option<Vec<DynamicAccessHint>> = None;
            let mut dynamic_writes: Option<Vec<DynamicAccessHint>> = None;

            loop {
                parser.skip_ws();
                if parser.try_consume_char(b'}')? {
                    break;
                }

                let key = parser.parse_key()?;
                match key.as_str() {
                    "read_keys" => {
                        if read_keys.is_some() {
                            return Err(json::Error::duplicate_field("read_keys"));
                        }
                        read_keys = Some(Vec::<String>::json_deserialize(parser)?);
                    }
                    "write_keys" => {
                        if write_keys.is_some() {
                            return Err(json::Error::duplicate_field("write_keys"));
                        }
                        write_keys = Some(Vec::<String>::json_deserialize(parser)?);
                    }
                    "dynamic_reads" => {
                        if dynamic_reads.is_some() {
                            return Err(json::Error::duplicate_field("dynamic_reads"));
                        }
                        dynamic_reads = Some(Vec::<DynamicAccessHint>::json_deserialize(parser)?);
                    }
                    "dynamic_writes" => {
                        if dynamic_writes.is_some() {
                            return Err(json::Error::duplicate_field("dynamic_writes"));
                        }
                        dynamic_writes = Some(Vec::<DynamicAccessHint>::json_deserialize(parser)?);
                    }
                    other => {
                        return Err(json::Error::unknown_field(other));
                    }
                }

                if parser.consume_comma_if_present()? {
                    continue;
                }
                parser.skip_ws();
                parser.consume_char(b'}')?;
                break;
            }

            let read_keys = read_keys.ok_or_else(|| json::Error::missing_field("read_keys"))?;
            let write_keys = write_keys.ok_or_else(|| json::Error::missing_field("write_keys"))?;

            Ok(AccessSetHints {
                read_keys,
                write_keys,
                dynamic_reads: dynamic_reads.unwrap_or_default(),
                dynamic_writes: dynamic_writes.unwrap_or_default(),
            })
        }
    }

    /// Signature metadata binding a manifest to an approved signer.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct ManifestProvenance {
        /// Public key that signed the manifest payload.
        pub signer: PublicKey,
        /// Signature over the manifest payload (see [`ContractManifestSignaturePayload`]).
        pub signature: Signature,
    }

    /// Declarative metadata for a compiled entrypoint.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct EntrypointDescriptor {
        /// Symbol name as declared in the Kotodama source file.
        pub name: String,
        /// Logical kind: `kotoage`/`言挙げ`, view, `hajimari`/`始まり`, or
        /// `kaizen`/`改善`. Trigger declarations attach to one of these
        /// descriptors through [`EntrypointDescriptor::triggers`].
        pub kind: EntryPointKind,
        /// Ordered public parameters advertised by the compiler.
        #[norito(default)]
        pub params: Vec<EntrypointParamDescriptor>,
        /// Exact recursive schema for the complete public argument record.
        /// Zero-parameter entrypoints have no argument schema.
        #[norito(default)]
        pub argument_schema: Option<EntrypointArgumentSchemaV1>,
        /// Declared return type for this entrypoint, when present.
        #[norito(default)]
        pub return_type: Option<String>,
        /// Exact recursive schema for a non-unit public return value.
        #[norito(default)]
        pub return_schema: Option<EntrypointValueTypeV1>,
        /// Permission required by the dispatcher before invoking this entrypoint.
        #[norito(default)]
        pub permission: Option<String>,
        /// Advisory read keys for this entrypoint (flattened `state:...` strings).
        #[norito(default)]
        pub read_keys: Vec<String>,
        /// Advisory write keys for this entrypoint.
        #[norito(default)]
        pub write_keys: Vec<String>,
        /// Whether access-set hints are complete or explicitly provided.
        #[norito(default)]
        pub access_hints_complete: Option<bool>,
        /// Reasons access hints were skipped for this entrypoint.
        #[norito(default)]
        pub access_hints_skipped: Vec<String>,
        /// Trigger declarations that call this entrypoint.
        #[norito(default)]
        pub triggers: Vec<TriggerDescriptor>,
    }

    /// Declarative parameter metadata for a public or view entrypoint.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct EntrypointParamDescriptor {
        /// Stable parameter name as declared in the Kotodama source file.
        pub name: String,
        /// Canonical type name advertised to clients.
        pub type_name: String,
    }

    /// Declarative durable state schema advertised by a compiled contract.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct StateDescriptor {
        /// Stable state key as declared in Kotodama source.
        pub name: String,
        /// Canonical durable value type stored under this key.
        pub type_name: String,
    }

    /// Stable application error code exposed by a compiled contract.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct ContractErrorCodeDescriptor {
        /// Error enum namespace declared in Kotodama source.
        pub namespace: String,
        /// Variant name within the namespace.
        pub name: String,
        /// Explicit non-zero numeric code returned on abort.
        pub code: u32,
    }

    /// Localized message text for a specific language tag.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct KotobaTranslation {
        /// Language tag, e.g. "en", "ja".
        pub lang: String,
        /// Localized message text.
        pub text: String,
    }

    /// Translation entry keyed by a stable message id.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct KotobaTranslationEntry {
        /// Stable message identifier.
        pub msg_id: String,
        /// Localized translations for this message.
        pub translations: Vec<KotobaTranslation>,
    }

    /// Entrypoint callback target referenced by a trigger declaration.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct TriggerCallback {
        /// Optional contract namespace for cross-contract callbacks.
        #[norito(default)]
        pub namespace: Option<String>,
        /// Entrypoint name to invoke.
        pub entrypoint: String,
    }

    /// Declarative trigger metadata attached to an entrypoint.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct TriggerDescriptor {
        /// Trigger identifier.
        pub id: TriggerId,
        /// Repeat policy for the trigger action.
        pub repeats: Repeats,
        /// Event filter that drives execution.
        pub filter: EventFilterBox,
        /// Optional explicit authority override.
        #[norito(default)]
        pub authority: Option<AccountId>,
        /// Trigger metadata payload (JSON map).
        #[norito(default)]
        pub metadata: Metadata,
        /// Callback target for this trigger.
        pub callback: TriggerCallback,
    }

    /// Entry point category advertised by Kotodama.
    #[derive(Debug, Clone, Copy, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[norito(tag = "kind", content = "value")]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub enum EntryPointKind {
        /// Transaction dispatcher entrypoint (`kotoage`/`言挙げ fn`).
        Kotoage,
        /// Read-only query entrypoint (`view fn`).
        View,
        /// Deployment `hajimari`/`始まり` declaration.
        Hajimari,
        /// `kaizen`/`改善` lifecycle declaration.
        Kaizen,
    }

    /// Canonical payload signed to attest a manifest.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        derive(iroha_ffi::FfiType)
    )]
    #[cfg_attr(
        all(feature = "ffi_export", not(feature = "ffi_import")),
        ffi_type(opaque)
    )]
    pub struct ContractManifestSignaturePayload {
        /// Canonical source-level seiyaku name.
        #[norito(default)]
        pub seiyaku_name: Option<String>,
        /// Content-addressed hash of the compiled `.to` bytecode.
        pub code_hash: Option<Hash>,
        /// ABI hash computed by the node for the `abi_version` policy.
        pub abi_hash: Option<Hash>,
        /// Optional compiler fingerprint (e.g., rustc/LLVM versions).
        pub compiler_fingerprint: Option<String>,
        /// Compiler-derived, hash-covered ZK/VECTOR execution capability bitmap.
        ///
        /// This mirrors the signed manifest field and is unrelated to host
        /// hardware acceleration availability.
        pub features_bitmap: Option<u64>,
        /// Optional advisory access-set hints for scheduler.
        #[norito(default)]
        pub access_set_hints: Option<AccessSetHints>,
        /// Optional entrypoint descriptors (name, kind, permission) advertised by the compiler.
        #[norito(default)]
        pub entrypoints: Option<Vec<EntrypointDescriptor>>,
        /// Optional durable state schema advertised by the compiler.
        #[norito(default)]
        pub states: Option<Vec<StateDescriptor>>,
        /// Stable application error codes advertised by the compiler.
        #[norito(default)]
        pub error_codes: Option<Vec<ContractErrorCodeDescriptor>>,
        /// Optional localization tables extracted from `kotoba { ... }` blocks.
        #[norito(default)]
        pub kotoba: Option<Vec<KotobaTranslationEntry>>,
    }

    impl From<&ContractManifest> for ContractManifestSignaturePayload {
        fn from(manifest: &ContractManifest) -> Self {
            Self {
                seiyaku_name: manifest.seiyaku_name.clone(),
                code_hash: manifest.code_hash,
                abi_hash: manifest.abi_hash,
                compiler_fingerprint: manifest.compiler_fingerprint.clone(),
                features_bitmap: manifest.features_bitmap,
                access_set_hints: manifest.access_set_hints.clone(),
                entrypoints: manifest.entrypoints.clone(),
                states: manifest.states.clone(),
                error_codes: manifest.error_codes.clone(),
                kotoba: manifest.kotoba.clone(),
            }
        }
    }

    impl ContractManifest {
        /// Build the canonical payload that must be signed for provenance checks.
        #[must_use]
        pub fn signature_payload(&self) -> ContractManifestSignaturePayload {
            ContractManifestSignaturePayload::from(self)
        }

        /// Encode the canonical signing payload into Norito bytes.
        #[must_use]
        pub fn signature_payload_bytes(&self) -> Vec<u8> {
            norito::to_bytes(&self.signature_payload())
                .expect("manifest signature payload encoding must succeed")
        }

        /// Attach provenance by signing the canonical payload with the provided key pair.
        ///
        /// # Errors
        ///
        /// Returns a crypto error if the selected signing backend rejects the
        /// key material or payload.
        pub fn try_signed(mut self, key_pair: &KeyPair) -> Result<Self, CryptoError> {
            let payload = self.signature_payload_bytes();
            let signature = Signature::try_new(key_pair.private_key(), &payload)?;
            self.provenance = Some(ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            });
            Ok(self)
        }

        /// Attach provenance by signing the canonical payload with the provided key pair.
        #[must_use]
        pub fn signed(self, key_pair: &KeyPair) -> Self {
            self.try_signed(key_pair)
                .expect("contract manifest signing should succeed")
        }
    }

    #[cfg(all(test, feature = "json"))]
    mod tests {
        use super::*;

        #[test]
        fn access_set_hints_roundtrip() {
            let hints = AccessSetHints {
                read_keys: vec!["account:satoshi".to_owned()],
                write_keys: vec!["asset:btc#iroha".to_owned()],
                dynamic_reads: Vec::new(),
                dynamic_writes: Vec::new(),
            };

            let json = norito::json::to_json(&hints).expect("serialize access hints");
            assert_eq!(
                json,
                "{\"read_keys\":[\"account:satoshi\"],\"write_keys\":[\"asset:btc#iroha\"],\"dynamic_reads\":[],\"dynamic_writes\":[]}"
            );

            let decoded: AccessSetHints = norito::json::from_str(&json).expect("deserialize hints");
            assert_eq!(decoded.read_keys, hints.read_keys);
            assert_eq!(decoded.write_keys, hints.write_keys);
        }

        #[test]
        fn entrypoint_kind_json_uses_only_branded_v1_names() {
            for (kind, name) in [
                (EntryPointKind::Kotoage, "Kotoage"),
                (EntryPointKind::View, "View"),
                (EntryPointKind::Hajimari, "Hajimari"),
                (EntryPointKind::Kaizen, "Kaizen"),
            ] {
                let json = norito::json::to_json(&kind).expect("serialize entrypoint kind");
                assert_eq!(json, format!(r#"{{"kind":"{name}","value":null}}"#));
                let decoded: EntryPointKind =
                    norito::json::from_str(&json).expect("deserialize branded entrypoint kind");
                assert_eq!(decoded, kind);
            }

            for retired in ["Public", "public", "Init", "init", "Upgrade", "upgrade"] {
                let json = format!(r#"{{"kind":"{retired}","value":null}}"#);
                norito::json::from_str::<EntryPointKind>(&json)
                    .expect_err("retired English entrypoint kind must be rejected");
            }
        }

        #[test]
        fn entrypoint_descriptor_includes_triggers() {
            use crate::{events::EventFilterBox, trigger::action::Repeats};

            let trigger = TriggerDescriptor {
                id: "wake".parse().expect("trigger id"),
                repeats: Repeats::Indefinitely,
                filter: EventFilterBox::Time(crate::events::time::TimeEventFilter(
                    crate::events::time::ExecutionTime::PreCommit,
                )),
                authority: None,
                metadata: Metadata::default(),
                callback: TriggerCallback {
                    namespace: None,
                    entrypoint: "run".to_string(),
                },
            };
            let entrypoint = EntrypointDescriptor {
                name: "run".to_string(),
                kind: EntryPointKind::Kotoage,
                params: vec![EntrypointParamDescriptor {
                    name: "amount".to_string(),
                    type_name: "quantity".to_string(),
                }],
                argument_schema: Some(EntrypointArgumentSchemaV1 {
                    fields: vec![
                        crate::smart_contract::entrypoint::EntrypointArgumentFieldV1 {
                            name: "amount".to_string(),
                            ty: EntrypointValueTypeV1 {
                                nodes: vec![crate::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Leaf(
                                    crate::smart_contract::entrypoint::EntrypointValueKindV1::Quantity,
                                )],
                            },
                        },
                    ],
                }),
                return_type: Some("int".to_string()),
                return_schema: Some(EntrypointValueTypeV1 {
                    nodes: vec![crate::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Leaf(
                        crate::smart_contract::entrypoint::EntrypointValueKindV1::Int,
                    )],
                }),
                permission: Some("ExecuteContract".to_owned()),
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: vec![trigger],
            };
            let json = norito::json::to_json(&entrypoint).expect("serialize entrypoint");
            assert!(json.contains("\"triggers\""));
            let decoded: EntrypointDescriptor =
                norito::json::from_str(&json).expect("deserialize entrypoint");
            assert_eq!(decoded.triggers.len(), 1);
            assert_eq!(decoded.triggers[0].callback.entrypoint, "run");
        }

        #[test]
        fn access_set_hints_missing_fields_fail() {
            let err = norito::json::from_str::<AccessSetHints>("{}")
                .expect_err("missing fields must fail");
            match err {
                norito::json::Error::MissingField { field } => {
                    assert_eq!(field, "read_keys", "unexpected field: {field}");
                }
                norito::json::Error::Message(msg) => {
                    assert!(msg.contains("read_keys"), "unexpected error: {msg}");
                }
                other => panic!("unexpected error: {other}"),
            }
        }
    }

    #[cfg(test)]
    mod manifest_signing_tests {
        use iroha_crypto::KeyPair;

        use super::*;

        fn checked_random_keypair() -> KeyPair {
            KeyPair::try_random().expect("test fixture random key generation should succeed")
        }

        #[test]
        fn signature_payload_excludes_provenance_and_verifies() {
            let kp = checked_random_keypair();
            let mut manifest = ContractManifest {
                seiyaku_name: None,
                code_hash: Some(Hash::new(b"code-bytes")),
                abi_hash: Some(Hash::new(b"abi-bytes")),
                compiler_fingerprint: Some("rustc-1.78".to_owned()),
                features_bitmap: Some(0xAA),
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: Some(vec![ContractErrorCodeDescriptor {
                    namespace: "PaymentError".to_owned(),
                    name: "Unauthorized".to_owned(),
                    code: 1001,
                }]),
                provenance: None,
            };

            let payload = manifest.signature_payload_bytes();
            let signature = Signature::try_new(kp.private_key(), &payload)
                .expect("checked contract manifest fixture signature");
            manifest.provenance = Some(ManifestProvenance {
                signer: kp.public_key().clone(),
                signature: signature.clone(),
            });

            // Provenance should not affect the payload bytes.
            assert_eq!(payload, manifest.signature_payload_bytes());
            signature
                .verify(kp.public_key(), &payload)
                .expect("signature must verify");

            manifest.error_codes.as_mut().expect("error codes")[0].code = 1002;
            assert!(
                signature
                    .verify(kp.public_key(), &manifest.signature_payload_bytes())
                    .is_err(),
                "manifest provenance must bind stable error codes"
            );
        }

        #[test]
        fn try_signed_attaches_verifiable_provenance() {
            let kp = checked_random_keypair();
            let manifest = ContractManifest {
                seiyaku_name: None,
                code_hash: Some(Hash::new(b"contract-code")),
                abi_hash: Some(Hash::new(b"contract-abi")),
                compiler_fingerprint: Some("kotodama-test".to_owned()),
                features_bitmap: Some(0x55),
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            };

            let signed = manifest.try_signed(&kp).expect("sign manifest");
            let provenance = signed.provenance.as_ref().expect("manifest provenance");
            let payload = signed.signature_payload_bytes();

            assert_eq!(provenance.signer, kp.public_key().clone());
            provenance
                .signature
                .verify(kp.public_key(), &payload)
                .expect("signature must verify");
        }
    }
}
