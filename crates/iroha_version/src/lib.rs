//! Structures, traits and impls related to versioning.
//!
//! For usage examples see [`iroha_version_derive::declare_versioned`].
#![allow(unexpected_cfgs)]

use core::ops::Range;
use std::{format, string::String, vec::Vec};

pub mod build_line;
/// Re-export derive macros that assist with declaring versioned enums.
#[cfg(feature = "derive")]
pub use iroha_version_derive::*;
/// Re-export Norito codec helpers required by consumers of versioned types.
pub use norito::codec::{Decode, DecodeAll, Encode};

pub use crate::build_line::BuildLine;

/// JSON field name storing the version discriminator.
pub const VERSION_FIELD_NAME: &str = "version";
/// JSON field name storing the versioned payload.
pub const CONTENT_FIELD_NAME: &str = "content";
/// Internal helpers used by derive-generated JSON code. Provides a stable path
/// to JSON facilities so downstream crates do not need to depend on Norito
/// directly when using `iroha_version_derive` macros with the `json` feature.
/// JSON utilities used by derive-generated code.
#[cfg(feature = "json")]
pub mod json_helpers {
    pub use norito::json::{
        Value, from_json, from_slice, from_str, from_value, parse_value, to_json, to_string,
        to_value,
    };
}

/// Module which contains error and result for versioning
/// Error types emitted while working with versioned containers.
pub mod error {
    use std::{borrow::ToOwned, boxed::Box, fmt};

    use iroha_macro::FromVariant;

    use super::UnsupportedVersion;
    #[allow(unused_imports)] // False-positive
    use super::*;

    /// Versioning errors
    #[derive(Debug, FromVariant, thiserror::Error)]
    pub enum Error {
        /// This is not a versioned object
        NotVersioned,
        /// Cannot encode unsupported version from JSON to Norito
        UnsupportedJsonEncode,
        /// Expected JSON object
        ExpectedJson,
        /// Cannot encode unsupported version from Norito to JSON
        UnsupportedNoritoEncode,
        /// JSON (de)serialization issue
        #[cfg(feature = "json")]
        Json,
        /// Norito (de)serialization issue
        NoritoCodec(String),
        /// Problem with parsing integers
        ParseInt,
        /// Input version unsupported
        UnsupportedVersion(Box<UnsupportedVersion>),
        /// Buffer is not empty after decoding. Returned by `decode_all_versioned()`
        ExtraBytesLeft(u64),
    }

    // Map Norito JSON errors into the crate's generic JSON error variant.
    // This allows `?` on `norito::json` helpers inside derive-generated code
    // to convert into `iroha_version::error::Error` seamlessly.
    impl From<norito::json::Error> for Error {
        fn from(_: norito::json::Error) -> Self {
            Self::Json
        }
    }

    impl From<norito::Error> for Error {
        fn from(x: norito::Error) -> Self {
            use std::string::ToString as _;
            Self::NoritoCodec(x.to_string())
        }
    }

    impl From<core::num::ParseIntError> for Error {
        fn from(_: core::num::ParseIntError) -> Self {
            Self::ParseInt
        }
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let msg = match self {
                Self::NotVersioned => "Not a versioned object".to_owned(),
                Self::UnsupportedJsonEncode => {
                    "Cannot encode unsupported version from JSON to Norito".to_owned()
                }
                Self::ExpectedJson => "Expected JSON object".to_owned(),
                Self::UnsupportedNoritoEncode => {
                    "Cannot encode unsupported version from Norito to JSON".to_owned()
                }
                #[cfg(feature = "json")]
                Self::Json => "JSON (de)serialization issue".to_owned(),
                Self::NoritoCodec(x) => format!("Norito (de)serialization issue: {x}"),
                Self::ParseInt => "Issue with parsing integers".to_owned(),
                Self::UnsupportedVersion(v) => {
                    format!("Input version {} is unsupported", v.version)
                }
                Self::ExtraBytesLeft(n) => format!("Buffer contains {n} bytes after decoding"),
            };

            write!(f, "{msg}")
        }
    }

    /// Result type for versioning
    pub type Result<T, E = Error> = core::result::Result<T, E>;
}

/// General trait describing if this is a versioned container.
pub trait Version {
    /// Version of the data contained inside.
    fn version(&self) -> u8;

    /// Supported versions.
    fn supported_versions() -> Range<u8>;

    /// If the contents' version is currently supported.
    fn is_supported(&self) -> bool {
        Self::supported_versions().contains(&self.version())
    }
}

/// Structure describing a container content which version is not supported.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, norito::Encode, norito::Decode)]
#[allow(unexpected_cfgs)]
#[error(
    "Unsupported version. Expected: {}, got: {version}",
    Self::expected_version()
)]
pub struct UnsupportedVersion {
    /// Version of the content.
    pub version: u8,
    /// Raw content.
    pub raw: RawVersioned,
}

impl UnsupportedVersion {
    /// Constructs [`UnsupportedVersion`].
    #[must_use]
    #[inline]
    pub const fn new(version: u8, raw: RawVersioned) -> Self {
        Self { version, raw }
    }

    /// Expected version
    pub const fn expected_version() -> u8 {
        1
    }
}

/// Raw versioned content, serialized.
#[derive(Debug, Clone, PartialEq, Eq, norito::codec::Encode, norito::codec::Decode)]
pub enum RawVersioned {
    /// In JSON format.
    Json(String),
    /// In Norito format.
    NoritoBytes(Vec<u8>),
}

impl<'a> norito::core::DecodeFromSlice<'a> for RawVersioned {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        use norito::core::{DecodeFromSlice, Error};

        let tag = *bytes.first().ok_or(Error::LengthMismatch)?;
        let rest = &bytes[1..];
        match tag {
            0 => {
                let (value, used) = <String as DecodeFromSlice>::decode_from_slice(rest)?;
                Ok((RawVersioned::Json(value), 1 + used))
            }
            1 => {
                let (value, used) = <Vec<u8> as DecodeFromSlice>::decode_from_slice(rest)?;
                Ok((RawVersioned::NoritoBytes(value), 1 + used))
            }
            other => Err(Error::invalid_tag("decoding RawVersioned tag", other)),
        }
    }
}

/// Norito related versioned (de)serialization traits.
pub mod codec {
    use norito::codec::{DecodeAll, Encode};

    use super::{Version, error::Result};

    /// [`norito::codec::Decode`] versioned analog.
    pub trait DecodeVersioned: DecodeAll + Version {
        /// Use this function for versioned objects instead of `decode_all`.
        ///
        /// # Errors
        /// - Version is unsupported
        /// - Input won't have enough bytes for decoding
        /// - Input has extra bytes
        fn decode_all_versioned(input: &[u8]) -> Result<Self>;
    }

    /// [`norito::codec::Encode`] versioned analog.
    pub trait EncodeVersioned: Encode + Version {
        /// Use this function for versioned objects instead of `encode`.
        fn encode_versioned(&self) -> Vec<u8>;
    }
}

/// JSON related versioned (de)serialization traits.
#[cfg(feature = "json")]
pub mod json {
    use std::{
        borrow::ToOwned,
        string::{String, ToString},
    };

    use norito::json::Value;

    use super::{
        Version,
        error::{Error, Result},
    };

    /// JSON-focused versioned deserialize helper.
    pub trait DeserializeVersioned: Version {
        /// Use this function for versioned objects instead of [`norito::json::from_json`].
        ///
        /// # Errors
        ///
        /// Returns an error if the input cannot be parsed as JSON or does not contain a valid
        /// versioned payload compatible with the implementor.
        fn from_versioned_json_str(input: &str) -> Result<Self>
        where
            Self: Sized;
    }

    /// JSON-focused versioned serialize helper.
    pub trait SerializeVersioned: Version {
        /// Use this function for versioned objects instead of [`norito::json::to_json`].
        ///
        /// # Errors
        ///
        /// Returns an error if the value cannot be encoded into a versioned JSON representation.
        fn to_versioned_json_str(&self) -> Result<String>
        where
            Self: Sized;
    }

    /// Extract version and content from a versioned JSON object.
    ///
    /// # Errors
    ///
    /// Returns an error if the input value is not an object with string `version` and nested
    /// `content` fields.
    pub fn extract_version_and_content(value: Value) -> Result<(String, Value)> {
        match value {
            Value::Object(mut map) => {
                let version_val = map
                    .remove(super::VERSION_FIELD_NAME)
                    .ok_or(Error::NotVersioned)?;
                let content_val = map
                    .remove(super::CONTENT_FIELD_NAME)
                    .ok_or(Error::ExpectedJson)?;
                let version_str = match version_val {
                    Value::String(s) => s,
                    _ => return Err(Error::NotVersioned),
                };
                Ok((version_str, content_val))
            }
            _ => Err(Error::ExpectedJson),
        }
    }

    /// Construct a `Value::Object` carrying the version metadata alongside the payload.
    pub fn build_versioned_object(version: &str, content: Value) -> Value {
        let mut map = norito::json::Map::new();
        map.insert(
            super::VERSION_FIELD_NAME.to_string(),
            Value::String(version.to_owned()),
        );
        map.insert(super::CONTENT_FIELD_NAME.to_string(), content);
        Value::Object(map)
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    #[cfg(feature = "json")]
    pub use super::json::*;
    pub use super::{codec::*, *};
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct VersionedContainer(pub u8);

    impl Version for VersionedContainer {
        fn version(&self) -> u8 {
            let VersionedContainer(version) = self;
            *version
        }

        fn supported_versions() -> Range<u8> {
            1..10
        }
    }

    #[test]
    fn supported_version() {
        assert!(!VersionedContainer(0).is_supported());
        assert!(VersionedContainer(1).is_supported());
        assert!(VersionedContainer(5).is_supported());
        assert!(!VersionedContainer(10).is_supported());
        assert!(!VersionedContainer(11).is_supported());
    }

    #[test]
    fn raw_versioned_roundtrip() {
        let original = RawVersioned::Json("test".to_owned());
        let bytes = original.encode();
        let decoded = RawVersioned::decode_all(&mut &bytes[..]).expect("decode");
        assert_eq!(decoded, original);
    }

    #[test]
    fn unsupported_version_roundtrip() {
        let original = UnsupportedVersion::new(2, RawVersioned::Json("test".to_owned()));
        let bytes = original.encode();
        let decoded = UnsupportedVersion::decode_all(&mut &bytes[..]).expect("decode");
        assert_eq!(decoded.version, original.version);
        assert_eq!(decoded.encode(), bytes);
    }
}
