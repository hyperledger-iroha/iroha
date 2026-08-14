//! Canonical logical paths used by durable smart-contract and native ledger state.
pub use self::model::*;
use crate::{error::ParseError, name::Name};
use iroha_data_model_derive::model;
use iroha_primitives::conststr::ConstString;
use norito::core::{DecodeFromSlice, Error as NoritoError};
use std::{borrow::Borrow, str::FromStr, string::String};
/// Maximum UTF-8 byte length of a canonical [`StatePath`].
///
/// The limit covers a canonical `StateMap` base (`Name`, at most 255 UTF-8
/// bytes), one separator, and the lowercase-hex expansion of a canonical key
/// of at most 4 KiB, with framing slack.
pub const MAX_STATE_PATH_BYTES: usize = 16 * 1024;
#[model]
mod model {
    use super::*;
    use derive_more::{Debug, Display};
    use iroha_schema::IntoSchema;
    /// Canonical logical path for durable smart-contract and native ledger state.
    ///
    /// Unlike [`Name`], this nominal type is sized for composite state paths and
    /// must not be used as an ordinary business identifier.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct StatePath(pub(super) ConstString);
}
impl StatePath {
    fn validate_str(candidate: &str) -> Result<(), ParseError> {
        const FORBIDDEN_CHARS: [char; 3] = ['@', '#', '$'];
        if candidate.is_empty() {
            return Err(ParseError::new("Empty `StatePath`"));
        }
        if candidate.len() > MAX_STATE_PATH_BYTES {
            return Err(ParseError::new(
                "`StatePath` exceeds the 16384-byte UTF-8 limit",
            ));
        }
        if candidate.chars().any(char::is_control) {
            return Err(ParseError::new(
                "Unicode control characters are not allowed in `StatePath` constructs",
            ));
        }
        if candidate.chars().any(crate::name::is_bidi_control) {
            return Err(ParseError::new(
                "Unicode bidirectional control characters are not allowed in `StatePath` constructs",
            ));
        }
        if candidate.chars().any(char::is_whitespace) {
            return Err(ParseError::new(
                "White space not allowed in `StatePath` constructs",
            ));
        }
        if candidate.chars().any(|ch| FORBIDDEN_CHARS.contains(&ch)) {
            return Err(ParseError::new(
                "The `@` character is reserved for scoped alias/public-key constructs, \
                 `#` for alias separators, and `$` for NFT identifiers",
            ));
        }
        Ok(())
    }
    fn parse(candidate: &str) -> Result<Self, ParseError> {
        Self::validate_str(candidate)?;
        let normalized = Name::normalize(candidate)?;
        Self::validate_str(normalized.as_ref())?;
        Ok(Self(ConstString::from(normalized.as_ref())))
    }
    fn decode_wire(bytes: &[u8]) -> Result<(Self, usize), NoritoError> {
        let (len, header_len) = norito::core::inspect_len_from_slice(bytes)?;
        if len > MAX_STATE_PATH_BYTES {
            return Err(NoritoError::Message(
                "`StatePath` exceeds the 16384-byte UTF-8 limit".into(),
            ));
        }
        let end = header_len
            .checked_add(len)
            .ok_or(NoritoError::LengthMismatch)?;
        let raw = bytes
            .get(header_len..end)
            .ok_or(NoritoError::LengthMismatch)?;
        let value = core::str::from_utf8(raw).map_err(|_| NoritoError::InvalidUtf8)?;
        norito::core::reserve_decode_allocation(len)?;
        let path = Self::parse(value).map_err(|error| NoritoError::Message(error.reason.into()))?;
        norito::core::note_payload_access(bytes, end);
        Ok((path, end))
    }
}
impl norito::core::NoritoSerialize for StatePath {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        <&str as norito::core::NoritoSerialize>::serialize(&self.as_ref(), writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_hint(&self.as_ref())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_exact(&self.as_ref())
    }
}
impl<'a> norito::core::NoritoDeserialize<'a> for StatePath {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("StatePath deserialization must succeed for valid archives")
    }
    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        if let Ok(payload) = norito::core::payload_slice_from_ptr(ptr) {
            return Self::decode_wire(payload).map(|(path, _)| path);
        }
        let string = norito::core::NoritoDeserialize::deserialize(archived.cast::<String>());
        Self::from_str(string.as_str())
            .map_err(|error| norito::core::Error::Message(error.reason.into()))
    }
}
impl AsRef<str> for StatePath {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl Borrow<str> for StatePath {
    fn borrow(&self) -> &str {
        self.0.as_ref()
    }
}
impl FromStr for StatePath {
    type Err = ParseError;
    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        Self::parse(candidate)
    }
}
impl TryFrom<String> for StatePath {
    type Error = ParseError;
    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::parse(&candidate)
    }
}
impl From<Name> for StatePath {
    fn from(name: Name) -> Self {
        Self::from(&name)
    }
}
impl From<&Name> for StatePath {
    fn from(name: &Name) -> Self {
        Self(ConstString::from(name.as_ref()))
    }
}
impl<'a> DecodeFromSlice<'a> for StatePath {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        Self::decode_wire(bytes)
    }
}
#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for StatePath {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.as_ref(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.as_ref(), out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for StatePath {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        Self::from_str(&value).map_err(|error| norito::json::Error::Message(error.reason.into()))
    }
}
/// Prelude exports for durable state paths.
pub mod prelude {
    pub use super::StatePath;
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::{Decode, Encode};
    use std::vec::Vec;
    #[test]
    fn accepts_long_paths_and_enforces_utf8_byte_limit() {
        let long = format!("balances/{}", "a".repeat(4 * 1024));
        let parsed = StatePath::from_str(&long).expect("path longer than Name remains valid");
        assert_eq!(parsed.as_ref(), long.as_str());
        let boundary = "a".repeat(MAX_STATE_PATH_BYTES);
        assert!(StatePath::from_str(&boundary).is_ok());
        assert!(StatePath::try_from(boundary).is_ok());
        let over_limit = "a".repeat(MAX_STATE_PATH_BYTES + 1);
        assert!(StatePath::from_str(&over_limit).is_err());
        assert!(StatePath::try_from(over_limit).is_err());
        let unicode_boundary = "é".repeat(MAX_STATE_PATH_BYTES / "é".len());
        assert_eq!(unicode_boundary.len(), MAX_STATE_PATH_BYTES);
        assert!(StatePath::from_str(&unicode_boundary).is_ok());
        assert!(StatePath::from_str(&format!("{unicode_boundary}é")).is_err());
    }
    #[test]
    fn normalizes_nfc_and_preserves_lexical_order() {
        let decomposed = StatePath::from_str("root/e\u{301}").expect("valid decomposed path");
        let composed = StatePath::from_str("root/é").expect("valid composed path");
        assert_eq!(decomposed, composed);
        let mut paths = [
            StatePath::from_str("root/10").unwrap(),
            StatePath::from_str("root/02").unwrap(),
            StatePath::from_str("root/2").unwrap(),
        ];
        paths.sort();
        assert_eq!(
            paths.map(|path| path.as_ref().to_owned()),
            ["root/02", "root/10", "root/2"]
        );
    }
    #[test]
    fn rejects_empty_whitespace_controls_bidi_and_reserved_identifier_separators() {
        for invalid in [
            "",
            "root/has space",
            "root/\0suffix",
            "root/\u{001F}",
            "root/\u{007F}",
            "root/\u{0080}",
            "root/\u{061C}",
            "root/\u{200E}",
            "root/\u{200F}",
            "root/\u{202E}",
            "root/\u{2066}",
            "root/\u{2069}",
            "root/alice@domain",
            "root/asset#domain",
            "root/nft$domain",
        ] {
            assert!(
                StatePath::from_str(invalid).is_err(),
                "unsafe state path was accepted: {invalid:?}"
            );
        }
    }
    #[test]
    fn from_name_is_infallible_and_nominal() {
        let name = Name::from_str("native_record").expect("valid Name");
        let path = StatePath::from(&name);
        assert_eq!(path.as_ref(), "native_record");
        assert_eq!(StatePath::from(name), path);
        assert_ne!(
            <StatePath as iroha_schema::TypeId>::id(),
            <Name as iroha_schema::TypeId>::id(),
            "StatePath and Name must remain nominally distinct in the public schema"
        );
        let schema = <StatePath as iroha_schema::IntoSchema>::schema();
        assert!(schema.contains_key::<StatePath>());
        assert!(!schema.contains_key::<Name>());
    }
    #[test]
    fn norito_roundtrip_and_decoders_enforce_validation() {
        let path =
            StatePath::from_str(&format!("root/{}", "ab".repeat(1024))).expect("valid long path");
        let encoded = path.encode();
        let mut cursor = encoded.as_slice();
        let decoded = StatePath::decode(&mut cursor).expect("decode StatePath");
        assert_eq!(decoded, path);
        assert!(cursor.is_empty());
        for invalid in [
            "root/\0suffix".to_owned(),
            "root/\u{202E}suffix".to_owned(),
            "x".repeat(MAX_STATE_PATH_BYTES + 1),
        ] {
            let forged = StatePath(ConstString::from(invalid.as_str()));
            let encoded = forged.encode();
            let mut cursor = encoded.as_slice();
            assert!(StatePath::decode(&mut cursor).is_err());
            assert!(StatePath::decode_from_slice(&encoded).is_err());
            let framed = norito::to_bytes(&forged).expect("encode forged fixture");
            assert!(norito::decode_from_bytes::<StatePath>(&framed).is_err());
        }
    }
    #[test]
    fn slice_decoder_rejects_declared_oversize_before_body_access() {
        let mut declared_oversize = Vec::new();
        norito::core::write_len_to_vec(
            &mut declared_oversize,
            u64::try_from(MAX_STATE_PATH_BYTES + 1).expect("path limit fits u64"),
        );
        let error = StatePath::decode_from_slice(&declared_oversize)
            .expect_err("oversized declaration must fail before reading a missing body");
        assert!(error.to_string().contains("16384-byte"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn json_roundtrip_and_decoder_enforce_validation() {
        let path =
            StatePath::from_str(&format!("root/{}", "c".repeat(1024))).expect("valid long path");
        let json = norito::json::to_json(&path).expect("serialize StatePath");
        let decoded = norito::json::from_str::<StatePath>(&json).expect("deserialize StatePath");
        assert_eq!(decoded, path);
        for invalid in [
            "\"root/\\u0000suffix\"".to_owned(),
            "\"root/\\u202Esuffix\"".to_owned(),
            format!("\"{}\"", "x".repeat(MAX_STATE_PATH_BYTES + 1)),
        ] {
            assert!(norito::json::from_str::<StatePath>(&invalid).is_err());
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn storage_json_canonicalizes_keys_and_rejects_normalization_duplicates() {
        type StateStorage = mv::storage::Storage<StatePath, Vec<u8>>;
        let decomposed = r#"{"revert":{},"blocks":{"root/e\u0301":[1]}}"#;
        let storage =
            norito::json::from_str::<StateStorage>(decomposed).expect("decode canonicalizable key");
        let canonical = norito::json::to_json(&storage).expect("encode canonical storage");
        assert!(canonical.contains("\"root/é\""));
        assert!(!canonical.contains("\\u0301"));
        let duplicate = r#"{"revert":{},"blocks":{"root/e\u0301":[1],"root/é":[2]}}"#;
        assert!(
            norito::json::from_str::<StateStorage>(duplicate).is_err(),
            "distinct JSON spellings of the same canonical StatePath must fail closed"
        );
    }
}
