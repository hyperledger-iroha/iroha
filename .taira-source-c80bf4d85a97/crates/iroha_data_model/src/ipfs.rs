//! Module with [`IpfsPath`] and related impls.

use std::{format, str::FromStr, string::String, vec::Vec};

use iroha_data_model_derive::model;
use iroha_primitives::conststr::ConstString;
use norito::{Decode, codec::Encode};

pub use self::model::*;
use crate::error::ParseError;

#[model]
mod model {
    use derive_more::Display;
    use iroha_schema::IntoSchema;

    use super::*;

    /// Represents path in IPFS. Performs checks to ensure path validity.
    /// Construct using [`FromStr::from_str`] method.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct IpfsPath(pub(super) ConstString);
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for IpfsPath {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.as_ref(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for IpfsPath {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse()
            .map_err(|err: ParseError| norito::json::Error::Message(err.reason.into()))
    }
}

impl IpfsPath {
    /// Superficially checks IPFS `cid` (Content Identifier)
    #[inline]
    const fn check_cid(cid: &str) -> Result<(), ParseError> {
        if cid.len() < 2 {
            return Err(ParseError {
                reason: "IPFS cid is too short",
            });
        }
        Ok(())
    }
}

impl FromStr for IpfsPath {
    type Err = ParseError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let mut subpath = string.split('/');
        let path_segment = subpath
            .next()
            .expect("First value of str::split() always has value");

        if path_segment.is_empty() {
            let root_type = subpath.next().ok_or(ParseError {
                reason: "Expected root type, but nothing found",
            })?;
            let key = subpath.next().ok_or(ParseError {
                reason: "Expected at least one content id",
            })?;

            match root_type {
                "ipfs" | "ipld" => Self::check_cid(key)?,
                "ipns" => (),
                _ => {
                    return Err(ParseError {
                        reason: "Unexpected root type, expected `ipfs`, `ipld` or `ipns`",
                    });
                }
            }
        } else {
            // by default if there is no prefix it's an ipfs or ipld path
            Self::check_cid(path_segment)?;
        }

        for path in subpath {
            Self::check_cid(path)?;
        }

        Ok(IpfsPath(ConstString::from(string)))
    }
}

impl AsRef<str> for IpfsPath {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Norito deserialization is derived via `Decode` above.
// DecodeFromSlice is provided via a crate-level shim in `norito_slice_decode.rs`.

#[cfg(test)]
mod tests {
    use std::string::ToString as _;

    use super::*;
    // Trait import not needed; tests use header-framed norito helpers directly.

    const INVALID_IPFS: [&str; 4] = [
        "",
        "/ipld",
        "/ipfs/a",
        "/ipfsssss/QmQqzMTavQgT4f4T5v6PWBp7XNKtoPmC9jvn12WPT3gkSE",
    ];

    #[test]
    fn test_invalid_ipfs_path() {
        assert!(matches!(
            INVALID_IPFS[0].parse::<IpfsPath>(),
            Err(err) if err.to_string() == "Expected root type, but nothing found"
        ));
        assert!(matches!(
            INVALID_IPFS[1].parse::<IpfsPath>(),
            Err(err) if err.to_string() == "Expected at least one content id"
        ));
        assert!(matches!(
            INVALID_IPFS[2].parse::<IpfsPath>(),
            Err(err) if err.to_string() == "IPFS cid is too short"
        ));
        assert!(matches!(
            INVALID_IPFS[3].parse::<IpfsPath>(),
            Err(err) if err.to_string() == "Unexpected root type, expected `ipfs`, `ipld` or `ipns`"
        ));
    }

    #[test]
    fn test_valid_ipfs_path() {
        // Valid paths
        "QmQqzMTavQgT4f4T5v6PWBp7XNKtoPmC9jvn12WPT3gkSE"
            .parse::<IpfsPath>()
            .expect("Path without root should be valid");
        "/ipfs/QmQqzMTavQgT4f4T5v6PWBp7XNKtoPmC9jvn12WPT3gkSE"
            .parse::<IpfsPath>()
            .expect("Path with ipfs root should be valid");
        "/ipld/QmQqzMTavQgT4f4T5v6PWBp7XNKtoPmC9jvn12WPT3gkSE"
            .parse::<IpfsPath>()
            .expect("Path with ipld root should be valid");
        "/ipns/QmSrPmbaUKA3ZodhzPWZnpFgcPMFWF4QsxXbkWfEptTBJd"
            .parse::<IpfsPath>()
            .expect("Path with ipns root should be valid");
        "/ipfs/SomeFolder/SomeImage"
            .parse::<IpfsPath>()
            .expect("Path with folders should be valid");
    }

    #[cfg(feature = "json")]
    #[test]
    fn ipfs_json_roundtrip() {
        let valid = "/ipfs/QmQqzMTavQgT4f4T5v6PWBp7XNKtoPmC9jvn12WPT3gkSE";
        let path = valid.parse::<IpfsPath>().expect("valid ipfs path");
        let json = norito::json::to_json(&path).expect("serialize ipfs path");
        assert_eq!(json, format!("\"{valid}\""));

        let decoded: IpfsPath = norito::json::from_json(&json).expect("deserialize ipfs path");
        assert_eq!(decoded.as_ref(), path.as_ref());
    }

    #[cfg(feature = "json")]
    #[test]
    fn ipfs_json_rejects_invalid_strings() {
        for invalid_ipfs in INVALID_IPFS {
            let invalid_ipfs = IpfsPath(invalid_ipfs.into());
            let serialized = norito::json::to_json(&invalid_ipfs).expect("Valid");
            let ipfs = norito::json::from_str::<IpfsPath>(serialized.as_str());

            assert!(ipfs.is_err());
        }
    }

    #[test]
    fn decode_ipfs() {
        // Roundtrip only valid paths via codec
        let valid = [
            "QmQqzMTavQgT4f4T5v6PWBp7XNKtoPmC9jvn12WPT3gkSE",
            "/ipfs/QmQqzMTavQgT4f4T5v6PWBp7XNKtoPmC9jvn12WPT3gkSE",
            "/ipld/QmQqzMTavQgT4f4T5v6PWBp7XNKtoPmC9jvn12WPT3gkSE",
            "/ipns/QmSrPmbaUKA3ZodhzPWZnpFgcPMFWF4QsxXbkWfEptTBJd",
            "/ipfs/SomeFolder/SomeImage",
        ];
        for s in valid {
            let path = s.parse::<IpfsPath>().expect("valid");
            // Use stable header-framed Norito over String, then parse back to IpfsPath
            let bytes = norito::to_bytes(&s.to_string()).expect("encode str");
            let archived = norito::from_bytes::<String>(&bytes).expect("archived str");
            let decoded_s = norito::core::NoritoDeserialize::deserialize(archived);
            assert_eq!(decoded_s, s);
            let reparsed = decoded_s.parse::<IpfsPath>().expect("parse back");
            assert_eq!(reparsed.as_ref(), path.as_ref());
        }
    }
}

// Ensure codec-level decode validates the path as well
