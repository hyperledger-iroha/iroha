//! Structures, traits and impls related to versioning.
//!
//! For usage examples see [`iroha_version_derive::declare_versioned`].
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::ops::Range;

#[cfg(feature = "derive")]
pub use iroha_version_derive::*;
#[cfg(feature = "scale")]
pub use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "json")]
use serde::{Deserialize, Serialize};

/// Module which contains error and result for versioning
pub mod error {
    #[cfg(not(feature = "std"))]
    use alloc::{borrow::ToOwned, boxed::Box};
    use core::fmt;

    use iroha_macro::FromVariant;
    #[cfg(feature = "scale")]
    use parity_scale_codec::{Decode, Encode};

    use super::UnsupportedVersion;
    #[allow(unused_imports)] // False-positive
    use super::*;

    /// Versioning errors
    #[derive(Debug, FromVariant)]
    #[cfg_attr(feature = "std", derive(thiserror::Error))]
    #[cfg_attr(feature = "scale", derive(Encode, Decode))]
    pub enum Error {
        /// This is not a versioned object
        NotVersioned,
        /// Cannot encode unsupported version from JSON to Parity SCALE
        UnsupportedJsonEncode,
        /// Expected JSON object
        ExpectedJson,
        /// Cannot encode unsupported version from Parity SCALE to JSON
        UnsupportedScaleEncode,
        /// JSON (de)serialization issue
        #[cfg(feature = "json")]
        Serde,
        /// Parity SCALE (de)serialization issue
        #[cfg(feature = "scale")]
        ParityScale(String),
        /// Problem with parsing integers
        ParseInt,
        /// Input version unsupported
        UnsupportedVersion(Box<UnsupportedVersion>),
        /// Buffer is not empty after decoding. Returned by `decode_all_versioned()`
        ExtraBytesLeft(u64),
    }

    #[cfg(feature = "json")]
    impl From<serde_json::Error> for Error {
        fn from(_: serde_json::Error) -> Self {
            Self::Serde
        }
    }

    #[cfg(feature = "scale")]
    impl From<parity_scale_codec::Error> for Error {
        fn from(x: parity_scale_codec::Error) -> Self {
            #[cfg(not(feature = "std"))]
            use alloc::string::ToString as _;
            #[cfg(feature = "std")]
            use std::string::ToString as _;

            Self::ParityScale(x.to_string())
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
                    "Cannot encode unsupported version from JSON to SCALE".to_owned()
                }
                Self::ExpectedJson => "Expected JSON object".to_owned(),
                Self::UnsupportedScaleEncode => {
                    "Cannot encode unsupported version from SCALE to JSON".to_owned()
                }
                #[cfg(feature = "json")]
                Self::Serde => "JSON (de)serialization issue".to_owned(),
                #[cfg(feature = "scale")]
                Self::ParityScale(x) => format!("Parity SCALE (de)serialization issue: {x}"),
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
#[cfg_attr(feature = "scale", derive(Encode, Decode))]
#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "std",
    error(
        "Unsupported version. Expected: {}, got: {version}",
        Self::expected_version()
    )
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "scale", derive(Encode, Decode))]
#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]
pub enum RawVersioned {
    /// In JSON format.
    Json(String),
    /// In Parity Scale Codec format.
    ScaleBytes(Vec<u8>),
}

/// Scale related versioned (de)serialization traits.
#[cfg(feature = "scale")]
pub mod scale {
    #[cfg(not(feature = "std"))]
    use alloc::vec::Vec;

    use parity_scale_codec::{DecodeAll, Encode};

    use super::{error::Result, Version};

    /// [`parity_scale_codec::Decode`] versioned analog.
    pub trait DecodeVersioned: DecodeAll + Version {
        /// Use this function for versioned objects instead of `decode_all`.
        ///
        /// # Errors
        /// - Version is unsupported
        /// - Input won't have enough bytes for decoding
        /// - Input has extra bytes
        fn decode_all_versioned(input: &[u8]) -> Result<Self>;
    }

    /// [`parity_scale_codec::Encode`] versioned analog.
    pub trait EncodeVersioned: Encode + Version {
        /// Use this function for versioned objects instead of `encode`.
        fn encode_versioned(&self) -> Vec<u8>;
    }
}

/// JSON related versioned (de)serialization traits.
#[cfg(feature = "json")]
pub mod json {
    #[cfg(not(feature = "std"))]
    use alloc::string::String;

    use serde::{Deserialize, Serialize};

    use super::{error::Result, Version};

    /// [`Serialize`] versioned analog, specifically for JSON.
    pub trait DeserializeVersioned<'de>: Deserialize<'de> + Version {
        /// Use this function for versioned objects instead of [`serde_json::from_str`].
        ///
        /// # Errors
        /// Return error if:
        /// * serde fails to decode json
        /// * if json is not an object
        /// * if json is has no version field
        fn from_versioned_json_str(input: &str) -> Result<Self>;
    }

    /// [`Deserialize`] versioned analog, specifically for JSON.
    pub trait SerializeVersioned: Serialize + Version {
        /// Use this function for versioned objects instead of [`serde_json::to_string`].
        ///
        /// # Errors
        /// Return error if serde fails to decode json
        fn to_versioned_json_str(&self) -> Result<String>;
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    #[cfg(feature = "json")]
    pub use super::json::*;
    #[cfg(feature = "scale")]
    pub use super::scale::*;
    pub use super::*;
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
}
