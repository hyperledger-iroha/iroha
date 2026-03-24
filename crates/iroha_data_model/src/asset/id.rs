//! Asset identifiers.

use std::{
    array, fmt, format,
    hash::{Hash, Hasher},
    str::FromStr,
    string::String,
};

use getset::Getters;
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
    to_bytes,
};

pub use self::model::*;
use crate::{Name, account::prelude::*, domain::prelude::*, error::ParseError, nexus::DataSpaceId};

#[model]
mod model {
    use super::*;

    /// Canonical asset definition identifier.
    ///
    /// Textual form is an unprefixed Base58 address over canonical UUIDv4 bytes
    /// plus a version byte and checksum.
    #[derive(Debug, Clone, Getters, IntoSchema)]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetDefinitionId {
        /// Canonical `UUIDv4` bytes.
        #[getset(get_copy = "pub")]
        pub aid_bytes: [u8; 16],
        /// Deterministic domain component derived from canonical bytes.
        pub domain: DomainId,
        /// Deterministic name component derived from canonical bytes.
        pub name: Name,
    }

    /// Balance partition used for a concrete asset ownership bucket.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Decode,
        Encode,
        IntoSchema,
        Default,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum AssetBalanceScope {
        /// Unrestricted balance bucket shared across all dataspaces.
        #[default]
        Global,
        /// Dataspace-restricted bucket keyed by a specific dataspace identifier.
        Dataspace(DataSpaceId),
    }

    /// Identification of an asset combines the entity identifier ([`AssetId`]) with the owner [`AccountId`].
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Getters, Decode, Encode, IntoSchema)]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetId {
        /// Account Identification.
        pub account: AccountId,
        /// Entity Identification.
        pub definition: AssetDefinitionId,
        /// Balance partition scope for this ownership bucket.
        #[norito(default)]
        pub scope: AssetBalanceScope,
    }
}

string_id!(AssetDefinitionId);

const ASSET_DEFINITION_ADDRESS_VERSION: u8 = 1;
const ASSET_DEFINITION_ADDRESS_LEN: usize = 1 + 16 + 4;

impl PartialEq for AssetDefinitionId {
    fn eq(&self, other: &Self) -> bool {
        self.aid_bytes == other.aid_bytes
    }
}

impl Eq for AssetDefinitionId {}

impl PartialOrd for AssetDefinitionId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AssetDefinitionId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.aid_bytes.cmp(&other.aid_bytes)
    }
}

impl Hash for AssetDefinitionId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.aid_bytes.hash(state);
    }
}

impl NoritoSerialize for AssetDefinitionId {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        <[u8; 16] as NoritoSerialize>::serialize(&self.aid_bytes, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        <[u8; 16] as NoritoSerialize>::encoded_len_hint(&self.aid_bytes)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        <[u8; 16] as NoritoSerialize>::encoded_len_exact(&self.aid_bytes)
    }
}

impl<'de> NoritoDeserialize<'de> for AssetDefinitionId {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        let aid_bytes = <[u8; 16] as NoritoDeserialize>::deserialize(archived.cast());
        Self::from_uuid_bytes_unchecked(aid_bytes)
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let aid_bytes = <[u8; 16] as NoritoDeserialize>::deserialize(archived.cast());
        Self::from_uuid_bytes(aid_bytes)
            .map_err(|err| norito::core::Error::Message(err.to_string()))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for AssetId {
    fn write_json(&self, out: &mut String) {
        let literal = self.canonical_encoded();
        norito::json::JsonSerialize::json_serialize(&literal, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AssetId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        AssetId::parse_encoded(&value).map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

impl AssetId {
    /// Create a new [`AssetId`]
    pub fn new(definition: AssetDefinitionId, account: AccountId) -> Self {
        Self {
            account,
            definition,
            scope: AssetBalanceScope::Global,
        }
    }

    /// Convenience alias for [`Self::new`]
    pub fn of(definition: AssetDefinitionId, account: AccountId) -> Self {
        Self::new(definition, account)
    }

    /// Create an [`AssetId`] with an explicit balance scope.
    pub fn with_scope(
        definition: AssetDefinitionId,
        account: AccountId,
        scope: AssetBalanceScope,
    ) -> Self {
        Self {
            account,
            definition,
            scope,
        }
    }

    /// Parse an encoded asset identifier from `norito:<hex>`.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the value is empty, not prefixed with
    /// `norito:`, contains invalid hex, or does not decode into [`AssetId`].
    pub fn parse_encoded(input: &str) -> Result<Self, ParseError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ParseError {
                reason: "Asset ID must not be empty",
            });
        }
        if trimmed.contains('#') {
            return Err(ParseError {
                reason: "Asset ID textual forms are not supported; use encoded `norito:<hex>`",
            });
        }

        let prefix = "norito:";
        let Some(payload_hex) = trimmed
            .get(..prefix.len())
            .filter(|head| head.eq_ignore_ascii_case(prefix))
            .map(|_| &trimmed[prefix.len()..])
        else {
            return Err(ParseError {
                reason: "Asset ID must use encoded `norito:<hex>` format",
            });
        };
        if payload_hex.is_empty() {
            return Err(ParseError {
                reason: "Asset ID must include hex payload after `norito:`",
            });
        }

        let payload = hex::decode(payload_hex).map_err(|_| ParseError {
            reason: "Asset ID `norito:` payload must be valid hex",
        })?;
        norito::decode_from_bytes::<Self>(&payload).map_err(|_| ParseError {
            reason: "Asset ID `norito:` payload is invalid",
        })
    }

    /// Render this identifier in the canonical encoded `norito:<hex>` form.
    #[must_use]
    pub fn canonical_encoded(&self) -> String {
        // `parse_encoded` expects header-framed Norito bytes.
        let payload = to_bytes(self).expect("asset id encoding should not fail");
        format!("norito:{}", hex::encode(payload))
    }
}

impl AssetDefinitionId {
    /// Construct an identifier from canonical `UUIDv4` bytes.
    ///
    /// # Errors
    /// Returns [`ParseError`] when `aid_bytes` do not satisfy `UUIDv4`
    /// version/variant constraints.
    pub fn from_uuid_bytes(aid_bytes: [u8; 16]) -> Result<Self, ParseError> {
        if !is_uuid_v4_bytes(&aid_bytes) {
            return Err(ParseError::new(
                "Asset Definition ID must encode UUIDv4 bytes",
            ));
        }
        let (domain, name) = synthetic_components(aid_bytes);
        Ok(Self {
            aid_bytes,
            domain,
            name,
        })
    }

    /// Construct from UUID bytes without validation.
    #[must_use]
    pub fn from_uuid_bytes_unchecked(aid_bytes: [u8; 16]) -> Self {
        let (domain, name) = synthetic_components(aid_bytes);
        Self {
            aid_bytes,
            domain,
            name,
        }
    }

    /// Deterministically derive canonical UUID bytes from component labels.
    #[must_use]
    pub fn new(domain: DomainId, name: Name) -> Self {
        let literal = format!("{name}#{domain}");
        let digest = blake3::hash(literal.as_bytes());
        let mut aid_bytes = [0u8; 16];
        aid_bytes.copy_from_slice(&digest.as_bytes()[..16]);
        // Force UUIDv4 version and RFC4122 variant bits.
        aid_bytes[6] = (aid_bytes[6] & 0x0f) | 0x40;
        aid_bytes[8] = (aid_bytes[8] & 0x3f) | 0x80;
        Self {
            aid_bytes,
            domain,
            name,
        }
    }

    /// Canonical textual address (unprefixed Base58 with version and checksum).
    #[must_use]
    pub fn canonical_address(&self) -> String {
        let payload = self.address_payload();
        bs58::encode(payload).into_string()
    }

    /// Returns `true` when this identifier is an opaque synthetic identifier
    /// literal rather than a domain-scoped asset definition synthesized from
    /// business domain/name components.
    #[must_use]
    pub fn is_opaque_canonical(&self) -> bool {
        self.domain.name.as_ref() == "aid" && self.name.as_ref() == hex::encode(self.aid_bytes)
    }

    /// Parse the canonical unprefixed Base58 address.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the textual form is not canonical, fails
    /// checksum verification, or bytes do not satisfy `UUIDv4` constraints.
    pub fn parse_address_literal(input: &str) -> Result<Self, ParseError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new("Asset Definition ID must not be empty"));
        }
        if trimmed.contains(':') {
            return Err(ParseError::new(
                "Asset Definition ID must use unprefixed Base58 format",
            ));
        }
        let payload = bs58::decode(trimmed)
            .into_vec()
            .map_err(|_| ParseError::new("Asset Definition ID must be valid Base58"))?;
        if payload.len() != ASSET_DEFINITION_ADDRESS_LEN {
            return Err(ParseError::new(
                "Asset Definition ID must contain exactly 21 decoded bytes",
            ));
        }
        if payload[0] != ASSET_DEFINITION_ADDRESS_VERSION {
            return Err(ParseError::new(
                "Asset Definition ID version is not supported",
            ));
        }
        let expected_checksum = address_checksum(&payload[..17]);
        if payload[17..] != expected_checksum {
            return Err(ParseError::new("Asset Definition ID checksum is invalid"));
        }
        let aid_bytes = array::from_fn(|index| payload[index + 1]);
        Self::from_uuid_bytes(aid_bytes)
    }

    /// Convenience alias for [`Self::new`].
    pub fn of(domain: DomainId, name: Name) -> Self {
        Self::new(domain, name)
    }

    fn address_payload(&self) -> [u8; ASSET_DEFINITION_ADDRESS_LEN] {
        let checksum = address_checksum(&[
            ASSET_DEFINITION_ADDRESS_VERSION,
            self.aid_bytes[0],
            self.aid_bytes[1],
            self.aid_bytes[2],
            self.aid_bytes[3],
            self.aid_bytes[4],
            self.aid_bytes[5],
            self.aid_bytes[6],
            self.aid_bytes[7],
            self.aid_bytes[8],
            self.aid_bytes[9],
            self.aid_bytes[10],
            self.aid_bytes[11],
            self.aid_bytes[12],
            self.aid_bytes[13],
            self.aid_bytes[14],
            self.aid_bytes[15],
        ]);
        [
            ASSET_DEFINITION_ADDRESS_VERSION,
            self.aid_bytes[0],
            self.aid_bytes[1],
            self.aid_bytes[2],
            self.aid_bytes[3],
            self.aid_bytes[4],
            self.aid_bytes[5],
            self.aid_bytes[6],
            self.aid_bytes[7],
            self.aid_bytes[8],
            self.aid_bytes[9],
            self.aid_bytes[10],
            self.aid_bytes[11],
            self.aid_bytes[12],
            self.aid_bytes[13],
            self.aid_bytes[14],
            self.aid_bytes[15],
            checksum[0],
            checksum[1],
            checksum[2],
            checksum[3],
        ]
    }
}

fn is_uuid_v4_bytes(bytes: &[u8; 16]) -> bool {
    (bytes[6] >> 4) == 0b0100 && (bytes[8] & 0b1100_0000) == 0b1000_0000
}

fn address_checksum(payload: &[u8]) -> [u8; 4] {
    let digest = blake3::hash(payload);
    let mut checksum = [0_u8; 4];
    checksum.copy_from_slice(&digest.as_bytes()[..4]);
    checksum
}

fn synthetic_components(aid_bytes: [u8; 16]) -> (DomainId, Name) {
    let domain: DomainId = "aid"
        .parse()
        .expect("static `aid` domain label must remain valid");
    let name_literal = hex::encode(aid_bytes);
    let name: Name = name_literal
        .parse()
        .expect("lowercase hex must remain a valid name literal");
    (domain, name)
}

impl fmt::Display for AssetDefinitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_address())
    }
}

/// Asset definition identifier textual representation.
impl FromStr for AssetDefinitionId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new("Asset Definition ID must not be empty"));
        }
        Self::parse_address_literal(trimmed)
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_encoded())
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_encoded())
    }
}

impl FromStr for AssetId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_encoded(s)
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;

    use super::*;
    use crate::account::AccountId;

    #[test]
    fn debug_formats_without_recursion() {
        let kp = KeyPair::random();
        let domain: DomainId = "domain".parse().unwrap();
        let name: Name = "xor".parse().unwrap();
        let account: AccountId = AccountId::new(kp.public_key().clone());
        let def = AssetDefinitionId::new(domain, name);
        let id = AssetId::new(def, account);
        let s = format!("{id:?}");
        // Should be the canonical encoded literal and not recurse.
        assert!(s.starts_with("norito:"));
    }

    #[test]
    fn asset_definition_id_parses_canonical_aid() {
        let expected = AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .expect("uuid v4 bytes");
        let literal = expected.to_string();
        let parsed: AssetDefinitionId = literal.parse().expect("address should parse");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.to_string(), literal);
    }

    #[test]
    fn asset_definition_id_distinguishes_opaque_from_domain_scoped_ids() {
        let opaque = AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .expect("opaque bytes should parse");
        assert!(opaque.is_opaque_canonical());

        let domain_scoped = AssetDefinitionId::new(
            "wonderland".parse().expect("domain"),
            "xor".parse().expect("name"),
        );
        assert!(!domain_scoped.is_opaque_canonical());
    }

    #[test]
    fn asset_id_parse_encoded_roundtrips() {
        let kp = KeyPair::random();
        let account = AccountId::new(kp.public_key().clone());
        let domain: DomainId = "wonderland".parse().expect("domain");
        let name: Name = "xor".parse().expect("name");
        let definition = AssetDefinitionId::new(domain, name);
        let id = AssetId::new(definition, account);

        let encoded = id.canonical_encoded();
        let parsed = AssetId::parse_encoded(&encoded).expect("encoded asset id parses");
        assert_eq!(parsed, id);
    }

    #[test]
    fn asset_id_with_explicit_scope_roundtrips() {
        let kp = KeyPair::random();
        let account = AccountId::new(kp.public_key().clone());
        let domain: DomainId = "wonderland".parse().expect("domain");
        let name: Name = "xor".parse().expect("name");
        let definition = AssetDefinitionId::new(domain, name);
        let id = AssetId::with_scope(
            definition,
            account,
            AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
        );

        let encoded = id.canonical_encoded();
        let parsed = AssetId::parse_encoded(&encoded).expect("encoded asset id parses");
        assert_eq!(parsed, id);
        assert_eq!(
            parsed.scope(),
            &AssetBalanceScope::Dataspace(DataSpaceId::new(7))
        );
    }

    #[test]
    fn asset_id_parse_encoded_rejects_textual_literal() {
        let kp = KeyPair::random();
        let account = AccountId::new(kp.public_key().clone());
        let literal = format!("xor#wonderland#{account}");

        let err = AssetId::parse_encoded(&literal).expect_err("textual literal must fail");
        assert!(
            err.reason().contains("textual forms are not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn asset_definition_id_parse_address_rejects_non_canonical_literals() {
        assert!(AssetDefinitionId::parse_address_literal("usd#wonderland").is_err());
        assert!(
            AssetDefinitionId::parse_address_literal("aid:2f17c724-66f8-4a4b-b8a8-e24884fdcd2f")
                .is_err()
        );
    }

    #[test]
    fn asset_definition_id_from_str_rejects_legacy_literal() {
        let err = "usd#wonderland"
            .parse::<AssetDefinitionId>()
            .expect_err("legacy literal must be rejected");
        assert!(
            err.to_string().contains("Base58"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn asset_definition_id_rejects_invalid_checksum() {
        let mut literal = AssetDefinitionId::new(
            "wonderland".parse().expect("domain"),
            "xor".parse().expect("name"),
        )
        .to_string()
        .into_bytes();
        let last = literal.len() - 1;
        literal[last] = if literal[last] == b'1' { b'2' } else { b'1' };
        let literal = String::from_utf8(literal).expect("utf8");

        let err = literal
            .parse::<AssetDefinitionId>()
            .expect_err("checksum must fail");
        assert!(
            err.to_string().contains("checksum"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn asset_definition_id_rejects_legacy_aid_prefix() {
        let err = AssetDefinitionId::parse_address_literal("aid:2f17c72466f84a4bb8a8e24884fdcd2f")
            .expect_err("legacy format must fail");
        assert!(
            err.to_string().contains("Base58"),
            "unexpected error: {err}"
        );
    }
}
