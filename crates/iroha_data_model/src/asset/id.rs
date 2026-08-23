//! Asset identifiers.
pub use self::model::*;
use crate::{Name, account::prelude::*, domain::prelude::*, error::ParseError, nexus::DataSpaceId};
use getset::{CopyGetters, Getters};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
};
use std::{array, fmt, format, str::FromStr, string::String};
#[model]
mod model {
    use super::*;
    /// Canonical public asset identifier.
    ///
    /// Textual form is an unprefixed Base58 address over canonical `UUIDv4` bytes plus a version
    /// byte and checksum. On-chain asset aliases resolve to this identifier only.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, CopyGetters, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetDefinitionId {
        /// Canonical `UUIDv4` bytes.
        #[getset(get_copy = "pub")]
        pub aid_bytes: [u8; 16],
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
    /// Internal balance-bucket identifier for a concrete owner/scope bucket.
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
#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for AssetDefinitionId {
    fn write_json(&self, out: &mut String) {
        out.push('"');
        // Writing to `String` is infallible. The only allocation is the caller-owned JSON output;
        // `AssetDefinitionId::fmt` keeps its Base58 workspace on the stack.
        let _ = fmt::write(out, format_args!("{self}"));
        out.push('"');
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_display_to(self, out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AssetDefinitionId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        Self::parse_address_literal(&value)
            .map_err(|err| asset_definition_id_json_error(err.reason()))
    }
    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let candidate = value.as_str().ok_or_else(|| {
            asset_definition_id_json_error("Asset Definition ID must be a JSON string")
        })?;
        Self::parse_address_literal(candidate)
            .map_err(|err| asset_definition_id_json_error(err.reason()))
    }
}
const ASSET_DEFINITION_ADDRESS_VERSION: u8 = 1;
const ASSET_DEFINITION_ADDRESS_LEN: usize = 1 + 16 + 4;
// `bs58` documents this as its allocation-free output bound: ceil(input_len * 1.5).
const ASSET_DEFINITION_ADDRESS_TEXT_MAX_LEN: usize =
    ASSET_DEFINITION_ADDRESS_LEN + ASSET_DEFINITION_ADDRESS_LEN.div_ceil(2);
#[cfg(feature = "json")]
fn asset_definition_id_json_error(message: &'static str) -> norito::json::Error {
    norito::json::Error::WithPos {
        msg: message,
        byte: 0,
        line: 1,
        col: 1,
    }
}
impl NoritoSerialize for AssetDefinitionId {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
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
        let literal = self.canonical_literal();
        norito::json::JsonSerialize::json_serialize(&literal, out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(&self.canonical_literal(), out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AssetId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        AssetId::parse_literal(&value).map_err(|err| norito::json::Error::Message(err.to_string()))
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
    /// Render this identifier in the canonical internal balance-bucket literal form.
    ///
    /// Public asset ids remain bare Base58 asset-definition ids. Global balance
    /// buckets use `<base58-asset-definition-id>#<i105-account-id>`.
    /// Dataspace-scoped balances append `#dataspace:<id>`.
    #[must_use]
    pub fn canonical_literal(&self) -> String {
        let base = format!("{}#{}", self.definition, self.account);
        match self.scope {
            AssetBalanceScope::Global => base,
            AssetBalanceScope::Dataspace(dataspace) => {
                format!("{base}#dataspace:{}", dataspace.as_u64())
            }
        }
    }
    /// Parse the canonical internal balance-bucket literal.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the literal is empty, not in internal
    /// `<base58-asset-definition-id>#<i105-account-id>` form, or uses an invalid
    /// dataspace scope suffix.
    pub fn parse_literal(input: &str) -> Result<Self, ParseError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new(
                "Asset balance bucket literal must not be empty",
            ));
        }
        let mut parts = trimmed.split('#');
        let definition_literal = parts.next().ok_or_else(|| {
            ParseError::new("Asset balance bucket literal must include an asset definition id")
        })?;
        let account_literal = parts.next().ok_or_else(|| {
            ParseError::new("Asset balance bucket literal must include an account id")
        })?;
        let scope_literal = parts.next();
        if parts.next().is_some() {
            return Err(ParseError::new(
                "Asset balance bucket literal must use `<base58-asset-definition-id>#<i105-account-id>` with optional `#dataspace:<id>` suffix; canonical asset-definition ids are Base58",
            ));
        }
        let definition = AssetDefinitionId::parse_address_literal(definition_literal)?;
        let account = AccountId::parse_encoded(account_literal)
            .map(ParsedAccountId::into_account_id)
            .map_err(|_| ParseError::new("Asset ID account is invalid"))?;
        let scope = match scope_literal {
            None => AssetBalanceScope::Global,
            Some(raw) => {
                let Some(dataspace) = raw.strip_prefix("dataspace:") else {
                    return Err(ParseError::new(
                        "Asset ID scope must use `dataspace:<id>` when present",
                    ));
                };
                let dataspace = dataspace
                    .parse::<u64>()
                    .map(DataSpaceId::new)
                    .map_err(|_| ParseError::new("Asset ID dataspace scope must be a u64"))?;
                AssetBalanceScope::Dataspace(dataspace)
            }
        };
        Ok(Self::with_scope(definition, account, scope))
    }
}
impl AssetDefinitionId {
    /// Construct an identifier from canonical `UUIDv4` bytes.
    ///
    /// # Errors
    /// Returns [`ParseError`] when `aid_bytes` do not satisfy `UUIDv4` version/variant constraints.
    pub fn from_uuid_bytes(aid_bytes: [u8; 16]) -> Result<Self, ParseError> {
        if !is_uuid_v4_bytes(&aid_bytes) {
            return Err(ParseError::new(
                "Asset Definition ID must encode UUIDv4 bytes",
            ));
        }
        Ok(Self { aid_bytes })
    }
    /// Construct from trusted UUID bytes already validated by the decoder.
    fn from_uuid_bytes_unchecked(aid_bytes: [u8; 16]) -> Self {
        Self { aid_bytes }
    }
    /// Deterministically derive canonical UUID bytes from component labels.
    ///
    /// The labels are only a deterministic seed. They are not retained and confer no ownership,
    /// routing, or display-name semantics. Those properties belong to the stored asset
    /// definition. Public textual identifiers remain the Base58 address returned by
    /// [`Self::canonical_address`].
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the public first-release constructor accepts owned typed components at its API boundary"
    )]
    pub fn derive_from_components(domain: DomainId, name: Name) -> Self {
        let literal = format!("{name}#{domain}");
        let digest = blake3::hash(literal.as_bytes());
        let mut aid_bytes = [0u8; 16];
        aid_bytes.copy_from_slice(&digest.as_bytes()[..16]);
        // Force UUIDv4 version and RFC4122 variant bits.
        aid_bytes[6] = (aid_bytes[6] & 0x0f) | 0x40;
        aid_bytes[8] = (aid_bytes[8] & 0x3f) | 0x80;
        Self { aid_bytes }
    }
    /// Canonical textual address (unprefixed Base58 with version and checksum).
    #[must_use]
    pub fn canonical_address(&self) -> String {
        self.to_string()
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
        let mut payload = [0_u8; ASSET_DEFINITION_ADDRESS_LEN];
        let decoded_len = bs58::decode(trimmed)
            .onto(&mut payload)
            .map_err(|_| ParseError::new("Asset Definition ID must be valid Base58"))?;
        if decoded_len != ASSET_DEFINITION_ADDRESS_LEN {
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
impl fmt::Display for AssetDefinitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut encoded = [0_u8; ASSET_DEFINITION_ADDRESS_TEXT_MAX_LEN];
        let encoded_len = bs58::encode(self.address_payload())
            .onto(&mut encoded[..])
            .map_err(|_| fmt::Error)?;
        let literal = core::str::from_utf8(&encoded[..encoded_len]).map_err(|_| fmt::Error)?;
        f.write_str(literal)
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
        f.write_str(&self.canonical_literal())
    }
}
impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_literal())
    }
}
impl FromStr for AssetId {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_literal(s)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountId;
    use iroha_crypto::KeyPair;
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked asset id fixture keypair")
    }
    #[test]
    fn debug_formats_without_recursion() {
        let kp = checked_random_keypair();
        let domain: DomainId = DomainId::try_new("domain", "universal").unwrap();
        let name: Name = "xor".parse().unwrap();
        let account: AccountId = AccountId::new(kp.public_key().clone());
        let def = AssetDefinitionId::derive_from_components(domain, name);
        let id = AssetId::new(def, account);
        let s = format!("{id:?}");
        assert_eq!(s, id.canonical_literal());
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
    fn asset_definition_id_formats_into_stack_sink_with_zero_decode_heap() {
        struct StackText {
            bytes: [u8; ASSET_DEFINITION_ADDRESS_TEXT_MAX_LEN],
            len: usize,
        }
        impl fmt::Write for StackText {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                let end = self.len.checked_add(value.len()).ok_or(fmt::Error)?;
                let destination = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
                destination.copy_from_slice(value.as_bytes());
                self.len = end;
                Ok(())
            }
        }

        let expected = AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .expect("uuid v4 bytes");
        let canonical = expected.canonical_address();
        let mut output = StackText {
            bytes: [0; ASSET_DEFINITION_ADDRESS_TEXT_MAX_LEN],
            len: 0,
        };
        let zero_allocation_limits =
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
        let (formatted, usage) =
            norito::core::with_decode_limits_measured(zero_allocation_limits, || {
                fmt::write(&mut output, format_args!("{expected}"))
            });
        formatted.expect("stack formatter");
        assert_eq!(&output.bytes[..output.len], canonical.as_bytes());
        assert_eq!(usage.total_allocated_bytes(), 0);
    }
    #[test]
    fn asset_definition_id_base58_parse_uses_no_decode_heap() {
        fn zero_allocation_limits() -> norito::DecodeLimits {
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX)
        }

        let expected = AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x2f,
        ])
        .expect("uuid v4 bytes");
        let literal = expected.to_string();
        let (parsed, usage) =
            norito::core::with_decode_limits_measured(zero_allocation_limits(), || {
                AssetDefinitionId::parse_address_literal(&literal)
            });
        assert_eq!(parsed.expect("stack Base58 parse"), expected);
        assert_eq!(usage.total_allocated_bytes(), 0);

        let oversized = "1".repeat(4_096);
        let (rejected, usage) =
            norito::core::with_decode_limits_measured(zero_allocation_limits(), || {
                AssetDefinitionId::parse_address_literal(&oversized)
            });
        assert!(rejected.is_err());
        assert_eq!(usage.total_allocated_bytes(), 0);

        #[cfg(feature = "json")]
        {
            let value = norito::json::Value::String(literal);
            let (parsed, usage) =
                norito::core::with_decode_limits_measured(zero_allocation_limits(), || {
                    <AssetDefinitionId as norito::json::JsonDeserialize>::json_from_value(&value)
                });
            assert_eq!(parsed.expect("borrowed stack Base58 parse"), expected);
            assert_eq!(usage.total_allocated_bytes(), 0);

            for invalid in [
                norito::json::Value::Bool(true),
                norito::json::Value::String("not:base58".into()),
            ] {
                let (rejected, usage) =
                    norito::core::with_decode_limits_measured(zero_allocation_limits(), || {
                        <AssetDefinitionId as norito::json::JsonDeserialize>::json_from_value(
                            &invalid,
                        )
                    });
                assert!(rejected.is_err());
                assert_eq!(usage.total_allocated_bytes(), 0);
            }
        }
    }
    #[test]
    fn asset_definition_id_rejects_non_v4_uuid_bytes() {
        assert!(AssetDefinitionId::from_uuid_bytes([0_u8; 16]).is_err());
        let mut wrong_variant = [0_u8; 16];
        wrong_variant[6] = 0x40;
        wrong_variant[8] = 0x40;
        assert!(AssetDefinitionId::from_uuid_bytes(wrong_variant).is_err());
    }
    #[test]
    fn unchecked_asset_definition_constructor_is_not_public() {
        let source = include_str!("id.rs");
        let forbidden = ["pub fn ", "from_uuid_bytes_unchecked"].concat();
        assert!(!source.contains(&forbidden));
    }
    #[test]
    fn component_derivation_produces_canonical_opaque_id() {
        let derived = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("name"),
        );
        let reparsed = AssetDefinitionId::parse_address_literal(&derived.canonical_address())
            .expect("derived address parses");
        assert_eq!(reparsed, derived);
    }
    #[test]
    fn asset_id_parse_literal_roundtrips_global() {
        let kp = checked_random_keypair();
        let account = AccountId::new(kp.public_key().clone());
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("name"),
        );
        let literal = format!("{definition}#{account}");
        let parsed = AssetId::parse_literal(&literal).expect("text literal should parse");
        assert_eq!(parsed, AssetId::new(definition, account));
        assert_eq!(parsed.to_string(), literal);
    }
    #[test]
    fn asset_id_parse_literal_roundtrips_scoped() {
        let kp = checked_random_keypair();
        let account = AccountId::new(kp.public_key().clone());
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("name"),
        );
        let literal = format!("{definition}#{account}#dataspace:7");
        let parsed = AssetId::parse_literal(&literal).expect("scoped literal should parse");
        assert_eq!(
            parsed,
            AssetId::with_scope(
                definition,
                account,
                AssetBalanceScope::Dataspace(DataSpaceId::new(7))
            )
        );
        assert_eq!(parsed.to_string(), literal);
    }
    #[test]
    fn asset_balance_scope_dataspace_encoding_shape() {
        let encoded =
            norito::codec::encode_adaptive(&AssetBalanceScope::Dataspace(DataSpaceId::new(42)));
        assert_eq!(hex::encode(encoded), "0100000009082a00000000000000");
    }
    #[test]
    fn asset_id_parse_literal_rejects_malformed_colon_literal() {
        let err =
            AssetId::parse_literal("not:an-asset").expect_err("malformed asset literal must fail");
        assert!(
            err.reason().contains("account id"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn asset_definition_id_parse_address_rejects_non_canonical_literals() {
        assert!(AssetDefinitionId::parse_address_literal("usd#wonderland").is_err());
        assert!(
            AssetDefinitionId::parse_address_literal("prefix:2f17c72466f84a4bb8a8e24884fdcd2f")
                .is_err()
        );
    }
    #[test]
    fn asset_definition_id_from_str_rejects_textual_seed_literal() {
        let err = "usd#wonderland"
            .parse::<AssetDefinitionId>()
            .expect_err("textual seed literal must be rejected");
        assert!(
            err.to_string().contains("Base58"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn asset_definition_id_rejects_invalid_checksum() {
        let mut literal = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
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
    fn asset_definition_id_rejects_prefixed_literal() {
        let err =
            AssetDefinitionId::parse_address_literal("prefix:2f17c72466f84a4bb8a8e24884fdcd2f")
                .expect_err("prefixed format must fail");
        assert!(
            err.to_string().contains("Base58"),
            "unexpected error: {err}"
        );
    }
}
