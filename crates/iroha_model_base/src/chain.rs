//! Canonical exact ASCII chain labels, bounded validation and structural codecs.

use derive_more::Display;
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use norito::core::{DecodeFromSlice, Error as NoritoError};
use std::borrow::Borrow;

pub use self::model::ChainId;
use crate::error::ParseError;

/// Maximum byte length of a canonical [`ChainId`].
///
/// Chain identifiers are ASCII, so this is also the maximum character count. The bound keeps every
/// signed, configured, and peer-advertised chain identity small before any allocation is performed.
pub use iroha_primitives::chain_id::MAX_CHAIN_ID_BYTES;

#[model]
mod model {
    use super::*;
    /// Canonical, deployment-selected identifier of a blockchain.
    ///
    /// The value is exact, case-sensitive ASCII. It starts and ends with an alphanumeric byte and
    /// may otherwise contain ASCII alphanumerics plus `.`, `_`, `:`, or `-`.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::id::model::ChainId")]
    pub struct ChainId(Box<str>);
    impl ChainId {
        fn parse(value: &str) -> Result<Self, ParseError> {
            iroha_primitives::chain_id::validate_chain_id(value).map_err(ParseError::new)?;
            Ok(Self(value.into()))
        }
        pub(super) fn decode_text_wire(bytes: &[u8]) -> Result<(Self, usize), NoritoError> {
            let (len, header_len) = norito::core::inspect_len_from_slice(bytes)?;
            if len > MAX_CHAIN_ID_BYTES {
                return Err(NoritoError::Message(
                    "`ChainId` exceeds the 128-byte ASCII limit".into(),
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
            let chain =
                Self::parse(value).map_err(|error| NoritoError::Message(error.reason().into()))?;
            norito::core::note_payload_access(bytes, end);
            Ok((chain, end))
        }
        pub(super) fn decode_wire(bytes: &[u8]) -> Result<(Self, usize), NoritoError> {
            let (wire, used) = norito::core::decode_field_canonical::<ChainIdWire>(bytes)?;
            Ok((wire.0.0, used))
        }
        /// Access inner string (owned).
        pub fn into_inner(self) -> Box<str> {
            self.0
        }
        /// Borrow inner string.
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }
    impl From<&'static str> for ChainId {
        fn from(value: &'static str) -> Self {
            Self::parse(value).expect("static chain id must be canonical")
        }
    }
    impl core::str::FromStr for ChainId {
        type Err = ParseError;
        fn from_str(value: &str) -> Result<Self, Self::Err> {
            Self::parse(value)
        }
    }
    impl TryFrom<String> for ChainId {
        type Error = ParseError;
        fn try_from(value: String) -> Result<Self, Self::Error> {
            Self::parse(&value)
        }
    }
    impl TryFrom<Box<str>> for ChainId {
        type Error = ParseError;
        fn try_from(value: Box<str>) -> Result<Self, Self::Error> {
            Self::parse(&value)
        }
    }
    impl AsRef<str> for ChainId {
        fn as_ref(&self) -> &str {
            self.as_str()
        }
    }
    impl Borrow<str> for ChainId {
        fn borrow(&self) -> &str {
            self.as_str()
        }
    }

    impl norito::json::FastJsonWrite for ChainId {
        fn write_json(&self, out: &mut String) {
            norito::json::JsonSerialize::json_serialize(self.as_str(), out);
        }
        fn write_json_to(
            &self,
            out: &mut dyn norito::json::JsonWriteSink,
        ) -> Result<(), norito::json::BoundedJsonError> {
            norito::json::write_json_string_to(self.as_str(), out)
        }
    }

    impl norito::json::JsonDeserialize for ChainId {
        fn json_deserialize(
            parser: &mut norito::json::Parser<'_>,
        ) -> Result<Self, norito::json::Error> {
            let value = parser.parse_string()?;
            Self::parse(&value).map_err(|error| norito::json::Error::Message(error.reason().into()))
        }
    }
}

/// Validation-aware decoder for the text field inside the structural V1
/// `ChainId` tuple-newtype representation.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::id::ChainIdText")]
struct ChainIdText(ChainId);

impl norito::core::SerializePayload for ChainIdText {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        <&str as norito::core::SerializePayload>::serialize(&self.0.as_str(), writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        <&str as norito::core::SerializePayload>::encoded_len_hint(&self.0.as_str())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        <&str as norito::core::SerializePayload>::encoded_len_exact(&self.0.as_str())
    }
}

impl<'a> norito::core::DeserializePayload<'a> for ChainIdText {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ChainId text deserialization must succeed for valid archives")
    }
    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let payload = norito::core::payload_slice_from_ptr(ptr)?;
        let (chain_id, used) = ChainId::decode_text_wire(payload)?;
        if used != payload.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        Ok(Self(chain_id))
    }
}
/// Mirrors the single-field structural layout originally assigned to
/// `ChainId`, while delegating its inner field to the validating decoder.
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::id::ChainIdWire")]
struct ChainIdWire(ChainIdText);

impl<'a> norito::core::DeserializePayload<'a> for ChainId {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ChainId deserialization must succeed for valid archives")
    }
    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        if let Ok(payload) = norito::core::payload_slice_from_ptr(ptr) {
            return ChainId::decode_wire(payload).map(|(chain, _)| chain);
        }
        let string = norito::core::DeserializePayload::deserialize(archived.cast::<String>());
        string
            .parse()
            .map_err(|error: ParseError| norito::core::Error::Message(error.reason().into()))
    }
}
impl<'a> DecodeFromSlice<'a> for ChainId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        Self::decode_wire(bytes)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_identity_tests;
