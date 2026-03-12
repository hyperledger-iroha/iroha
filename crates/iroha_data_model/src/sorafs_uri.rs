//! Module with [`SorafsUri`] and related impls.

use std::{str::FromStr, string::String};

use iroha_data_model_derive::model;
use iroha_primitives::conststr::ConstString;
use norito::codec::{Decode, Encode};

pub use self::model::*;
use crate::error::ParseError;

#[model]
mod model {
    use derive_more::Display;
    use iroha_schema::IntoSchema;

    use super::*;

    /// Strict `sorafs://...` URI literal used for logo links.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct SorafsUri(pub(super) ConstString);
}

impl SorafsUri {
    fn validate(value: &str) -> Result<(), ParseError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ParseError {
                reason: "SoraFS URI must not be empty",
            });
        }
        if trimmed != value {
            return Err(ParseError {
                reason: "SoraFS URI must not contain leading or trailing whitespace",
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(ParseError {
                reason: "SoraFS URI must not contain control characters",
            });
        }

        const PREFIX: &str = "sorafs://";
        let Some(rest) = trimmed.strip_prefix(PREFIX) else {
            return Err(ParseError {
                reason: "Logo URI must use `sorafs://` scheme",
            });
        };

        if rest.is_empty() {
            return Err(ParseError {
                reason: "SoraFS URI payload must not be empty",
            });
        }

        Ok(())
    }
}

impl FromStr for SorafsUri {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::validate(value)?;
        Ok(Self(ConstString::from(value)))
    }
}

impl AsRef<str> for SorafsUri {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SorafsUri {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(self.as_ref(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SorafsUri {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse()
            .map_err(|err: ParseError| norito::json::Error::Message(err.reason.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVALID: [&str; 6] = [
        "",
        " ",
        "sorafs://",
        "ipfs://bafybeigdyrztk",
        "/ipfs/bafybeigdyrztk",
        "https://gateway.sorafs/path",
    ];

    #[test]
    fn sorafs_uri_rejects_invalid_literals() {
        for raw in INVALID {
            assert!(raw.parse::<SorafsUri>().is_err(), "must fail: {raw}");
        }
    }

    #[test]
    fn sorafs_uri_accepts_valid_literal() {
        let parsed: SorafsUri = "sorafs://bafybeigdyrztk/logo.png"
            .parse()
            .expect("valid uri");
        assert_eq!(parsed.as_ref(), "sorafs://bafybeigdyrztk/logo.png");
    }
}
